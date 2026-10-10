---
parent: high-level-design
prefix: APP
---

# App perimeter and shell

## Context and Design Philosophy

Showrunner has no authentication by design: anyone who can reach the port can read and change the watchlist. This segment owns everything that makes that acceptable on a home LAN and nothing more: the configuration the process boots from, the HTTP perimeter that bounds what an unauthenticated caller can do (content-type gate, concurrency ceiling, body and time limits, CORS, security headers), the mapping from internal errors to client-safe responses, the health probe, and the delivery of the single-page app (static serving with history fallback, the router, the navigation shell, the fetch wrapper). Feature segments own their routes and pages; this segment owns the frame they sit in.

## Configuration

`Config::from_env` (backend/src/config.rs) runs after `dotenvy::dotenv()` loads a `.env` from the working directory if present (`run` in backend/src/lib.rs):

| Variable | Default | Validation |
|---|---|---|
| `SERVER_HOST` | `0.0.0.0` | none here; an unparsable value falls back to `0.0.0.0` at bind (`run` in lib.rs) |
| `SERVER_PORT` | `3001` | must parse as u16, else `SERVER_PORT must be a u16` |
| `DATABASE_URL` | `sqlite:///data/showrunner.db` | parsed by sqlx at pool creation, else `Invalid DATABASE_URL: …` |
| `TMDB_API_KEY` | required | missing or whitespace-only → `TMDB_API_KEY is required` |
| `RESYNC_CRON` | `0 0 6 * * *` | validated by the scheduler at startup (`resync`) |
| `TIMEZONE` | `America/New_York` | must be an IANA zone, else `TIMEZONE '<x>' is not a valid IANA zone (…)` |
| `CORS_ALLOWED_ORIGIN` | unset | blank or whitespace reads as unset; an unparsable header value panics at startup (`build_cors_layer`) |

Two more are read outside `Config`: `DB_MAX_CONNECTIONS` (default 5; an unparsable value silently falls back) in backend/src/db/pool.rs (`create_pool`), and `STATIC_DIR` (default `./static`) in lib.rs (`run`). `RUST_LOG` drives the tracing filter with `info` as the default (`log_filter` in lib.rs). Every configuration failure is an `AppError::Config` that aborts startup before the socket is bound.

`create_pool` (backend/src/db/pool.rs) opens SQLite with create-if-missing, foreign keys on, WAL journal, `synchronous=NORMAL`, a 30 s busy timeout, and the configured pool size, then runs the migrations embedded at build time from `backend/src/db/migrations`; a migration failure is also a startup `Config` error.

## HTTP perimeter

`build_api_router` (lib.rs) mounts the seventeen `/api/v1` routes and layers, outermost last:

1. **Content-type gate** (`require_json_content_type`, with `is_json_content_type` doing the match) — `POST`, `PUT`, and `PATCH` must carry `Content-Type: application/json` or `application/<x>+json` (parameters ignored) or receive 415 before any handler runs. A cross-origin HTML form can only produce the CORS-simple content types, so this turns every state-changing request into one a browser must preflight. It does not authenticate anything; `GET` and `DELETE` are unguarded, and a non-browser client can set the header freely.
2. **Concurrency ceiling** — `ConcurrencyLimit(MAX_INFLIGHT_REQUESTS)`, 64, behind `LoadShed`, so the 65th in-flight request gets an immediate 503 instead of queueing (the `ServiceBuilder` stack in `build_api_router`). Sized against the five-connection SQLite pool, not as a throughput target.
3. **Body limit** — 1 MiB (`RequestBodyLimitLayer` in `build_api_router`).
4. **Timeout** — `API_REQUEST_TIMEOUT`, 30 s per API request, answered with 408 (`TimeoutLayer` in `build_api_router`). The handler future is dropped when it fires; long adds and manual syncs are cut off, not cancelled upstream.

`build_cors_layer` (lib.rs) has three branches that differ only in the origin they accept: unset → `http://localhost:<port>`; a named origin → that origin; `*` → any origin, hand-built rather than via the permissive constructor so it grants the same explicit method list (`GET POST PUT PATCH DELETE`), any request header, no credentials, and no exposed-header wildcard. CORS is layered on the API router only; static responses carry no CORS headers.

`with_security_headers` (lib.rs) wraps the whole app last so static HTML gets them too: a CSP that locks scripts and connections to same-origin, allows images from `https://image.tmdb.org` and `data:`, permits inline styles, denies framing; plus `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`. Each is added only if not already present.

**Static delivery** (`with_static_fallback` in lib.rs): if `STATIC_DIR/index.html` exists, every non-API path is served from that directory, and a path with no matching file gets `index.html` with status 200 so client-side routes resolve for browsers and for anything that checks the status; otherwise the process logs a warning and runs API-only.

## Error responses

`AppError` (backend/src/error.rs) maps to HTTP as: `NotFound` 404, `InvalidData` 400, `Upstream` 502, `TooManyRequests` 429, everything else 500. The body is always `{"error": <client_message>}`. User-facing variants (`NotFound`, `InvalidData`, `Upstream`, `TooManyRequests`, and `Config`) pass their text through; internal variants (`Database`, `Http`, `Io`, `Json`) collapse to `An internal error occurred` and are logged at error level, while user-facing ones are not logged. `Config` is in the pass-through set even though it maps to 500, so a handler that uses it for an impossible state (`show vanished after insert`) leaks that text.

## Health

`GET /api/v1/health` (backend/src/api/health.rs) runs `SELECT 1` with a 2 s timeout and returns `{ status: "ok" | "degraded", version, database: bool }`, with HTTP 200 when the probe succeeds and 503 when it fails or times out. The body is for humans; the status code is what a container or proxy check keys on.

The binary doubles as its own probe. Started as `showrunner-backend --healthcheck`, it reads `SERVER_HOST` and `SERVER_PORT` (defaults `0.0.0.0` and `3001`, an unspecified host becoming loopback), issues one `GET /api/v1/health` with a 3 s timeout, and exits 0 on HTTP 200 and 1 on anything else, including a refused connection or a timeout. It loads nothing else: no `.env`, no `Config`, no database, so a probe never fails for a reason unrelated to the server's health. `main.rs` dispatches on that single argument before `run()`; the image's `HEALTHCHECK` (`build-ship`) is the only intended caller.

## SPA shell

- **Entry** (frontend/src/main.tsx) mounts `App` into `#root` under `StrictMode`; effects double-invoke in development, which is why every page guards its fetch with a `cancelled` flag.
- **Router** (frontend/src/App.tsx) — `BrowserRouter`, one `Layout` parent, nine child routes: `/` Up Next, `/watchlist`, `/movies`, `/movies/:tmdbId`, `/search`, `/shows/:tmdbId`, `/calendar`, `/history`, `/settings`. There is no catch-all; an unknown path renders the shell with an empty outlet. All pages are statically imported.
- **Layout** (frontend/src/components/Layout.tsx) — a sticky top bar with the brand link to `/` and seven `NavLink`s in the order Up Next, Watchlist, Movies, Search, Calendar, History, Settings; Up Next uses `end` so it is active only at exactly `/`. No mobile navigation.
- **Fetch wrapper** (`request` and `ApiError` in frontend/src/api/client.ts) — prefixes `/api/v1`, always sends `Content-Type: application/json` (so the server's gate is satisfied on every method), never opts into cross-origin credentials (fetch's same-origin default stands), has no timeout or retry. A non-2xx response rejects with an `ApiError` (a subclass of `Error`) whose `status` is the HTTP status and whose `message` is the body's `error` field when the body is JSON and the raw text otherwise, with no prefix; 204 resolves to `undefined`; any other success is parsed as JSON. Query values are URL-encoded; numeric path parameters are interpolated.
- **Types** (frontend/src/types/index.ts) mirror the Rust response structs in snake_case with no mapping layer. `BulkWatchScope` lives in the client module because the lint rules forbid non-component exports from component files.
- **Styles** (frontend/src/index.css) — one plain stylesheet: no custom properties, no media queries, light theme only; layout responsiveness comes from auto-fill grids and `flex-wrap`.
- **Error presentation** — pages render `Error: <message>` with the message exactly as the wrapper delivered it, which is the server's own text. No page interprets statuses or rewrites messages; the wording of upstream errors is the `tmdb` client's responsibility, and the status is available on `ApiError` for any page that ever needs to branch on it.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| Authentication | None | Basic auth; token; reverse-proxy auth | Deliberate for a single-household LAN app; auth and multi-user are one future design topic, not a bolt-on (SECURITY.md "Security model"; CONTRIBUTING.md "Things that likely need discussion first"). |
| CSRF defense | Require JSON content type on `POST`/`PUT`/`PATCH` at the router | Origin/Referer checks; CSRF tokens | Makes every state change a non-simple request a browser must preflight, covering body-less handlers and future routes alike (`require_json_content_type`). |
| Flood handling | 64 in-flight, shed to 503 | Queue; per-IP rate limiting | Bounds pending work from an unauthenticated flood without a queue that grows unbounded (`MAX_INFLIGHT_REQUESTS`; the `LoadShed` stack in `build_api_router`). |
| Request bounds | 1 MiB body, 30 s timeout | Per-route values | Generous for JSON bodies; the timeout caps the slowest legitimate call (a many-season add). |
| CORS default | `http://localhost:<port>` when unset | Same-origin only (no CORS layer) | Lets a dev browser on the same host call the API directly. |
| Wildcard CORS | Hand-built, no credentials, explicit methods | tower-http's permissive constructor | The permissive constructor also wildcards methods and exposed headers, a broader grant than the named branch; credentials with `*` are forbidden by the CORS spec (the `Some("*")` branch of `build_cors_layer`). |
| CSP | Same-origin scripts and connections; images from TMDB's CDN; inline styles allowed | Nonce-based styles; report-only | The SPA loads only its own bundle and TMDB posters (`CONTENT_SECURITY_POLICY`). |
| Header placement | Security headers wrap the whole app, last | API router only | The static HTML is where CSP and anti-framing matter (`with_security_headers`). |
| SPA delivery | Backend serves `dist/` with `index.html` fallback | Separate web server | One container, one process (README.md's overview, CLAUDE.md). |
| Error bodies | `{"error": …}` with generic text for internal variants | Pass-through messages | sqlx and reqwest internals must never reach a client (`AppError::client_message`). |
| Client error shape | `ApiError` with `status` as a field and the server message as `message` | `Error` with `API <status>: <message>` as the message, stripped per page | The status is data, not part of the sentence; carrying it as a field means every page shows the server's wording without each one parsing a prefix. |
| Health status code | 503 when degraded, 200 when ok | State in the body only | A container or proxy health check keys on the status code; the body alone cannot fail a probe. |
| Health probe | A `--healthcheck` mode in the binary that GETs the endpoint over loopback | curl or wget in the image; a compose `healthcheck` command; a sidecar | The runtime image has no shell or HTTP tool, and a compose `healthcheck` also executes inside the container, so the binary is the only thing that can run there; reqwest is already linked. |
| SQLite mode | WAL, `synchronous=NORMAL`, 30 s busy timeout, foreign keys on | Default journal | Concurrent reads during the serial resync writer; FK cascades are relied on by delete. |
| Home route | Up Next at `/` | Watchlist | "What do I watch next" is the daily question. |
| Data layer | A thin `fetch` wrapper, no query or state library | React Query, SWR | Nine pages with simple load-then-mutate flows. |
| Styling | Single plain stylesheet | CSS modules; utility framework; design tokens | Small surface, one theme. |

## Open Questions & Future Decisions

### Resolved
*(none yet)*

### Deferred
1. **Configuration outside `Config`.** `STATIC_DIR` (`run` in lib.rs) and `DB_MAX_CONNECTIONS` (`create_pool` in pool.rs) bypass `Config::from_env`; `STATIC_DIR` is passed by compose but documented in neither env.example nor README.md, and `DB_MAX_CONNECTIONS` is documented in both but missing from CLAUDE.md and not passed by compose.
2. **CORS origin validation.** An unparsable `CORS_ALLOWED_ORIGIN` panics (`build_cors_layer`) instead of returning a `Config` error like every other setting; the panic is pinned by a test.
3. **Silent host fallback.** A bad `SERVER_HOST` binds `0.0.0.0` without a warning (`run` in lib.rs).
4. **Timeout versus long handlers.** The 30 s timeout drops the add and manual-sync futures mid-work; the resync gate stays consumed.
5. **`Config` error text reaches clients** as a 500 body; handlers use the variant for impossible states.
6. **`DELETE` and the content-type gate.** HTML forms cannot issue `DELETE`, so the gap is theoretical, but worth stating as a rule.
7. **No catch-all route.** An unknown SPA path renders an empty shell rather than a not-found page.
8. **Responsive layout.** No breakpoints, no mobile navigation, light theme only.
9. **Duplicated page scaffolding.** The fetch-with-`cancelled` skeleton is hand-copied into eight pages with inconsistent loading, error, and empty structures.
10. **`lib.rs` has grown past the 300-line limit** set by the repository's file-size guideline.

## References

- backend/src/lib.rs; backend/src/main.rs (`--healthcheck` dispatch); backend/src/config.rs; backend/src/db/pool.rs; backend/src/error.rs; backend/src/api/health.rs (handler and probe)
- env.example; README.md "Configuration" and "Homelab / reverse proxy"; SECURITY.md "Security model" and "What the app does protect"
- frontend/src/main.tsx; frontend/src/App.tsx; frontend/src/components/Layout.tsx; frontend/src/api/client.ts; frontend/src/types/index.ts; frontend/src/index.css; frontend/index.html; frontend/vite.config.ts (dev proxy to 3001)
- Tests: backend/src/lib.rs tests (log filter, CORS branches incl. the panic, security headers; content-type matching is covered in backend/tests/api.rs), backend/src/config.rs tests, backend/src/db/pool.rs tests, backend/src/error.rs tests; backend/tests/api.rs `health_returns_ok_when_db_responsive` (health), `form_encoded_post_to_sync_is_rejected` through `get_requests_are_unaffected_by_the_content_type_gate` (content-type gate), `wildcard_branch_grants_no_more_than_the_named_origin_branch` through `named_origin_branch_still_denies_other_origins` (CORS); frontend/src/App.test.tsx, components/Layout.test.tsx, api/client.test.ts
- Consumers: every feature segment (routes, pages, `AppState`, `Config`, error mapping)
