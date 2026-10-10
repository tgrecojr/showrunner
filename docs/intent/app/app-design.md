---
parent: high-level-design
prefix: APP
---

# App perimeter and shell

## Context and Design Philosophy

Showrunner has no authentication by design: anyone who can reach the port can read and change the watchlist. This segment owns everything that makes that acceptable on a home LAN and nothing more: the configuration the process boots from, the HTTP perimeter that bounds what an unauthenticated caller can do (content-type gate, concurrency ceiling, body and time limits, CORS, security headers), the mapping from internal errors to client-safe responses, the health probe, and the delivery of the single-page app (static serving with history fallback, the router, the navigation shell, the fetch wrapper). Feature segments own their routes and pages; this segment owns the frame they sit in.

## Configuration

`Config::from_env` (backend/src/config.rs:29-68) runs after `dotenvy::dotenv()` loads a `.env` from the working directory if present (backend/src/lib.rs:235):

| Variable | Default | Validation |
|---|---|---|
| `SERVER_HOST` | `0.0.0.0` | none here; an unparsable value falls back to `0.0.0.0` at bind (lib.rs:269) |
| `SERVER_PORT` | `3001` | must parse as u16, else `SERVER_PORT must be a u16` |
| `DATABASE_URL` | `sqlite:///data/showrunner.db` | parsed by sqlx at pool creation, else `Invalid DATABASE_URL: …` |
| `TMDB_API_KEY` | required | missing or whitespace-only → `TMDB_API_KEY is required` |
| `RESYNC_CRON` | `0 0 6 * * *` | validated by the scheduler at startup (`resync`) |
| `TIMEZONE` | `America/New_York` | must be an IANA zone, else `TIMEZONE '<x>' is not a valid IANA zone (…)` |
| `CORS_ALLOWED_ORIGIN` | unset | blank or whitespace reads as unset; an unparsable header value panics at startup (lib.rs:217) |

Two more are read outside `Config`: `DB_MAX_CONNECTIONS` (default 5; an unparsable value silently falls back) in backend/src/db/pool.rs:8-11, and `STATIC_DIR` (default `./static`) in lib.rs:254. `RUST_LOG` drives the tracing filter with `info` as the default (lib.rs:237-239). Every configuration failure is an `AppError::Config` that aborts startup before the socket is bound.

`create_pool` (pool.rs:7-36) opens SQLite with create-if-missing, foreign keys on, WAL journal, `synchronous=NORMAL`, a 30 s busy timeout, and the configured pool size, then runs the migrations embedded at build time from `backend/src/db/migrations`; a migration failure is also a startup `Config` error.

## HTTP perimeter

`build_api_router` (lib.rs:120-182) mounts the seventeen `/api/v1` routes and layers, outermost last:

1. **Content-type gate** (lib.rs:77-116) — `POST`, `PUT`, and `PATCH` must carry `Content-Type: application/json` or `application/<x>+json` (parameters ignored) or receive 415 before any handler runs. A cross-origin HTML form can only produce the CORS-simple content types, so this turns every state-changing request into one a browser must preflight. It does not authenticate anything; `GET` and `DELETE` are unguarded, and a non-browser client can set the header freely.
2. **Concurrency ceiling** — `ConcurrencyLimit(64)` behind `LoadShed`, so the 65th in-flight request gets an immediate 503 instead of queueing (lib.rs:157-175). Sized against the five-connection SQLite pool, not as a throughput target.
3. **Body limit** — 1 MiB (lib.rs:176).
4. **Timeout** — 30 s per API request, answered with 408 (lib.rs:177-180). The handler future is dropped when it fires; long adds and manual syncs are cut off, not cancelled upstream.

`build_cors_layer` (lib.rs:184-231) has three branches that differ only in the origin they accept: unset → `http://localhost:<port>`; a named origin → that origin; `*` → any origin, hand-built rather than via the permissive constructor so it grants the same explicit method list (`GET POST PUT PATCH DELETE`), any request header, no credentials, and no exposed-header wildcard. CORS is layered on the API router only; static responses carry no CORS headers.

`with_security_headers` (lib.rs:55-75) wraps the whole app last so static HTML gets them too: a CSP that locks scripts and connections to same-origin, allows images from `https://image.tmdb.org` and `data:`, permits inline styles, denies framing; plus `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`. Each is added only if not already present.

**Static delivery** (lib.rs:254-264): if `STATIC_DIR/index.html` exists, every non-API path is served from that directory with `index.html` as the not-found fallback so client-side routes resolve; otherwise the process logs a warning and runs API-only.

## Error responses

`AppError` (backend/src/error.rs) maps to HTTP as: `NotFound` 404, `InvalidData` 400, `Upstream` 502, `TooManyRequests` 429, everything else 500. The body is always `{"error": <client_message>}`. User-facing variants (`NotFound`, `InvalidData`, `Upstream`, `TooManyRequests`, and `Config`) pass their text through; internal variants (`Database`, `Http`, `Io`, `Json`) collapse to `An internal error occurred` and are logged at error level, while user-facing ones are not logged. `Config` is in the pass-through set even though it maps to 500, so a handler that uses it for an impossible state (`show vanished after insert`) leaks that text.

## Health

`GET /api/v1/health` (backend/src/api/health.rs) runs `SELECT 1` with a 2 s timeout and returns `{ status: "ok" | "degraded", version, database: bool }`. It is the only handler that cannot fail, and today it returns HTTP 200 in both states, so a probe keyed on status code cannot detect a dead database. The intended behavior is 503 when degraded (`APP-HEALTH-002`), which is what the container health check in `build-ship` (`SHIP-IMAGE-006`) will key on.

## SPA shell

- **Entry** (frontend/src/main.tsx) mounts `App` into `#root` under `StrictMode`; effects double-invoke in development, which is why every page guards its fetch with a `cancelled` flag.
- **Router** (frontend/src/App.tsx) — `BrowserRouter`, one `Layout` parent, nine child routes: `/` Up Next, `/watchlist`, `/movies`, `/movies/:tmdbId`, `/search`, `/shows/:tmdbId`, `/calendar`, `/history`, `/settings`. There is no catch-all; an unknown path renders the shell with an empty outlet. All pages are statically imported.
- **Layout** (frontend/src/components/Layout.tsx) — a sticky top bar with the brand link to `/` and seven `NavLink`s in the order Up Next, Watchlist, Movies, Search, Calendar, History, Settings; Up Next uses `end` so it is active only at exactly `/`. No mobile navigation.
- **Fetch wrapper** (frontend/src/api/client.ts:15-33) — prefixes `/api/v1`, always sends `Content-Type: application/json` (so the server's gate is satisfied on every method), sends no credentials, has no timeout or retry. A non-2xx response rejects with an `ApiError` (a subclass of `Error`) whose `status` is the HTTP status and whose `message` is the body's `error` field when the body is JSON and the raw text otherwise, with no prefix; 204 resolves to `undefined`; any other success is parsed as JSON. Query values are URL-encoded; numeric path parameters are interpolated.
- **Types** (frontend/src/types/index.ts) mirror the Rust response structs in snake_case with no mapping layer. `BulkWatchScope` lives in the client module because the lint rules forbid non-component exports from component files.
- **Styles** (frontend/src/index.css) — one plain stylesheet: no custom properties, no media queries, light theme only; layout responsiveness comes from auto-fill grids and `flex-wrap`.
- **Error presentation** — pages render `Error: <message>` with the message exactly as the wrapper delivered it, which is the server's own text. No page interprets statuses or rewrites messages; the wording of upstream errors is the `tmdb` client's responsibility, and the status is available on `ApiError` for any page that ever needs to branch on it.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| Authentication | None | Basic auth; token; reverse-proxy auth | Deliberate for a single-household LAN app; auth and multi-user are one future design topic, not a bolt-on (SECURITY.md:13-22, CONTRIBUTING.md:30). |
| CSRF defense | Require JSON content type on `POST`/`PUT`/`PATCH` at the router | Origin/Referer checks; CSRF tokens | Makes every state change a non-simple request a browser must preflight, covering body-less handlers and future routes alike (lib.rs:77-92). |
| Flood handling | 64 in-flight, shed to 503 | Queue; per-IP rate limiting | Bounds pending work from an unauthenticated flood without a queue that grows unbounded (lib.rs:37-40, :157-163). |
| Request bounds | 1 MiB body, 30 s timeout | Per-route values | `[inferred]` Generous for JSON bodies; the timeout caps the slowest legitimate call (a many-season add). |
| CORS default | `http://localhost:<port>` when unset | Same-origin only (no CORS layer) | `[inferred]` Lets a dev browser on the same host call the API directly. |
| Wildcard CORS | Hand-built, no credentials, explicit methods | tower-http's permissive constructor | The permissive constructor also wildcards methods and exposed headers, a broader grant than the named branch; credentials with `*` are forbidden by the CORS spec (lib.rs:198-205). |
| CSP | Same-origin scripts and connections; images from TMDB's CDN; inline styles allowed | Nonce-based styles; report-only | The SPA loads only its own bundle and TMDB posters (lib.rs:42-44). |
| Header placement | Security headers wrap the whole app, last | API router only | The static HTML is where CSP and anti-framing matter (lib.rs:55-57). |
| SPA delivery | Backend serves `dist/` with `index.html` fallback | Separate web server | One container, one process (README.md:5, CLAUDE.md). |
| Error bodies | `{"error": …}` with generic text for internal variants | Pass-through messages | sqlx and reqwest internals must never reach a client (error.rs:49-54). |
| Client error shape | `ApiError` with `status` as a field and the server message as `message` | `Error` with `API <status>: <message>` as the message, stripped per page | The status is data, not part of the sentence; carrying it as a field means every page shows the server's wording without each one parsing a prefix. |
| Health status code | Intended: 503 when degraded, 200 when ok (`APP-HEALTH-002`); today always 200 | State in the body only | A container or proxy health check keys on the status code; the body alone cannot fail a probe. |
| SQLite mode | WAL, `synchronous=NORMAL`, 30 s busy timeout, foreign keys on | Default journal | `[inferred]` Concurrent reads during the serial resync writer; FK cascades are relied on by delete. |
| Home route | Up Next at `/` | Watchlist | `[inferred]` "What do I watch next" is the daily question. |
| Data layer | A thin `fetch` wrapper, no query or state library | React Query, SWR | `[inferred]` Nine pages with simple load-then-mutate flows. |
| Styling | Single plain stylesheet | CSS modules; utility framework; design tokens | `[inferred]` Small surface, one theme. |

## Open Questions & Future Decisions

### Resolved
1. ✅ **Degraded health is a 503.** `/api/v1/health` answers 503 when the database probe fails so container and proxy checks can act on it; the body keeps `status` and `database` for humans. Not yet implemented; tracked as `APP-HEALTH-002`.

### Deferred
1. **Configuration outside `Config`.** `STATIC_DIR` (lib.rs:254) and `DB_MAX_CONNECTIONS` (pool.rs:8) bypass `Config::from_env`; `STATIC_DIR` is documented nowhere and `DB_MAX_CONNECTIONS` is missing from CLAUDE.md and not passed by compose.
2. **CORS origin validation.** An unparsable `CORS_ALLOWED_ORIGIN` panics (lib.rs:217) instead of returning a `Config` error like every other setting; the panic is pinned by a test.
3. **Silent host fallback.** A bad `SERVER_HOST` binds `0.0.0.0` without a warning (lib.rs:269).
4. **Timeout versus long handlers.** The 30 s timeout drops the add and manual-sync futures mid-work; the resync gate stays consumed.
5. **`Config` error text reaches clients** as a 500 body; handlers use the variant for impossible states.
6. **`DELETE` and the content-type gate.** HTML forms cannot issue `DELETE`, so the gap is theoretical, but worth stating as a rule.
7. **No catch-all route.** An unknown SPA path renders an empty shell rather than a not-found page.
8. **Responsive layout.** No breakpoints, no mobile navigation, light theme only.
9. **Duplicated page scaffolding.** The fetch-with-`cancelled` skeleton is hand-copied into eight pages with inconsistent loading, error, and empty structures.
10. **`lib.rs` is 351 lines**, over the repository's 300-line guideline.

## References

- backend/src/lib.rs; backend/src/main.rs; backend/src/config.rs; backend/src/db/pool.rs; backend/src/error.rs; backend/src/api/health.rs
- env.example; README.md:163-211 (configuration and reverse proxy); SECURITY.md:11-30
- frontend/src/main.tsx; frontend/src/App.tsx; frontend/src/components/Layout.tsx; frontend/src/api/client.ts; frontend/src/types/index.ts; frontend/src/index.css; frontend/index.html; frontend/vite.config.ts (dev proxy to 3001)
- Tests: backend/src/lib.rs tests (content-type matching, CORS branches incl. the panic), backend/src/config.rs tests, backend/src/db/pool.rs tests, backend/src/error.rs tests; backend/tests/api.rs:76-88 (health), :878-974 (content-type gate), :976-1156 (CORS); frontend/src/App.test.tsx, components/Layout.test.tsx, api/client.test.ts
- Consumers: every feature segment (routes, pages, `AppState`, `Config`, error mapping)
