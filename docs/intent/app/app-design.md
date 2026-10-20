---
parent: high-level-design
prefix: APP
---

# App perimeter and shell

## Context and Design Philosophy

Showrunner has no authentication by design: anyone who can reach the port can read and change the watchlist. This segment owns everything that makes that acceptable on a home LAN and nothing more: the configuration the process boots from, the HTTP perimeter that bounds what an unauthenticated caller can do (content-type gate, concurrency ceiling, body and time limits, CORS, security headers), the mapping from internal errors to client-safe responses, the health probe, and the delivery of the single-page app (static serving with history fallback, the router, the navigation shell, the fetch wrapper, the shared stylesheet with its one narrow breakpoint). Feature segments own their routes and pages; this segment owns the frame they sit in, on a desktop browser and on a phone alike.

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
- **Layout** (frontend/src/components/Layout.tsx) — a sticky dark top bar with the brand link to `/` and seven `NavLink`s in the order Up Next, Watchlist, Movies, Search, Calendar, History, Settings; Up Next uses `end` so it is active only at exactly `/`. The links sit in a strip (`nav`) that the stylesheet lets scroll sideways inside the bar at narrow widths. `Layout` reads `useNarrow`; while narrow, whenever the pathname changes (including the first render of a deep link) or the viewport crosses into narrow, it scrolls the strip's active link into view instantly (`scrollIntoView` with `inline: "center"`, `block: "nearest"`, `behavior: "auto"`), so the current page is visible even when it is the seventh link. With no active link (a show or movie detail page, an unknown path) the strip keeps its position; a browser without the method is skipped. See *Narrow-width layout*.
- **Fetch wrapper** (`request` and `ApiError` in frontend/src/api/client.ts) — prefixes `/api/v1`, always sends `Content-Type: application/json` (so the server's gate is satisfied on every method), never opts into cross-origin credentials (fetch's same-origin default stands), has no timeout or retry. A non-2xx response rejects with an `ApiError` (a subclass of `Error`) whose `status` is the HTTP status and whose `message` is the body's `error` field when the body is JSON and the raw text otherwise, with no prefix; 204 resolves to `undefined`; any other success is parsed as JSON. Query values are URL-encoded; numeric path parameters are interpolated.
- **Types** (frontend/src/types/index.ts) mirror the Rust response structs in snake_case with no mapping layer. `BulkWatchScope` lives in the client module because the lint rules forbid non-component exports from component files.
- **Styles** (frontend/src/index.css) — one plain stylesheet, light theme only, no custom properties. Wide layouts come from auto-fill grids and `flex-wrap`; everything phone-specific lives in a single `@media (max-width: 900px)` block at the end of the file (the **narrow breakpoint**; the file has exactly one `max-width` query). Feature queries are separate and allowed: `@media (hover: hover)` wraps hover styles so touch screens never show a stuck hover state, and `@media (hover: none)` hides the link strip's scrollbar on touch devices only. Page segments add rules inside the one width block for their own markup; nobody adds a second width breakpoint.
- **Document head** (frontend/index.html) — the viewport meta carries `width=device-width, initial-scale=1, viewport-fit=cover`; `theme-color` matches the bar (`#1a1a1a`); a web app manifest (frontend/public/manifest.webmanifest: name, `display: standalone`, bar-coloured theme, 192 and 512 px icons) and an `apple-touch-icon` (frontend/public/apple-touch-icon.png, 180 px) let "Add to Home Screen" launch the app full-screen with a proper icon. All are same-origin static files, so the CSP needs no change.
- **Error presentation** — pages render `Error: <message>` with the message exactly as the wrapper delivered it, which is the server's own text. No page interprets statuses or rewrites messages; the wording of upstream errors is the `tmdb` client's responsibility, and the status is available on `ApiError` for any page that ever needs to branch on it.

## Narrow-width layout

The breakpoint is `max-width: 900px`, inclusive: a 900 px viewport is narrow. Below it the desktop top bar (brand, gaps, seven links, and the bar's own padding come to roughly 860 px) cannot fit, so the layout changes shape; above it the desktop layout is used unchanged. The value puts every phone in portrait and every tablet in portrait on the narrow layout and keeps a side-by-side desktop window on the dense one; the largest phones in landscape (above 900 CSS px) and a 13-inch iPad in portrait take the desktop layout, where the bar fits. The breakpoint appears twice and must agree: the stylesheet constant and the identical query string in `useNarrow` (frontend/src/hooks/useNarrow.ts), the one hook a component uses when it must change markup or behaviour rather than arrangement (`Layout` for the strip, the calendar in `up-next-calendar` for its agenda list). `useNarrow` reads `matchMedia(...).matches` synchronously for its first value, subscribes to the query's `change` event for resizes and rotation, and returns false when `matchMedia` is missing, so a page never renders one shape and then flips on mount. The stylesheet test pins both occurrences to the same value.

**Top bar.** At narrow widths the bar becomes two rows that keep the dark background: the brand alone on the first row; the seven links on a second row that scrolls horizontally inside the bar (`overflow-x: auto`, `min-width: 0`, `-webkit-overflow-scrolling: touch`). The bar never exceeds the viewport by construction, because the strip is the only thing that can be wider than the bar and it scrolls inside it; no `overflow-x: hidden` is placed on `html` or `body`, which would clip content silently and interfere with sticky positioning in Safari. A mask fades the strip's right edge as the affordance that more links follow; being a mask on the strip itself it intercepts no taps, and a trailing spacer inside the strip keeps the last link clear of the fade when scrolled to the end. The scrollbar is hidden only where `(hover: none)`, so a mouse user in a narrow window still sees one; links remain keyboard-focusable with a visible `:focus-visible` ring, and focusing a link scrolls it into view natively. Each link's own box is at least 44 px tall. The bar stays sticky.

```
┌──────────────────────────────────────┐
│ Showrunner                           │ ← brand row, dark
│ Up Next  Watchlist  Movies  Search ▸ │ ← link strip scrolls sideways, fades at edge
├──────────────────────────────────────┤
│ Page content, 1rem gutters           │
```

**Shared patterns.** The stylesheet owns these class families and their narrow arrangement; page segments use them without redefining them:

| Pattern | Classes | Desktop | Narrow |
|---|---|---|---|
| Detail header (show and movie pages) | `.show-header`, `.poster-lg`, `.show-meta`, `.detail-overview`, `.action-row` | 200×300 poster beside the metadata | 100×150 poster beside the title, pills, and metadata lines; the `.detail-overview` paragraph and the `.action-row` span the full width beneath; each action button stretches to the full row |
| Card grid (Watchlist, Movies, Search) | `.results-grid`, `.result-card` | auto-fill columns at 280 px minimum | one column; cards fill the width |
| Buttons and small controls | `.btn`, `.btn-sm`, `.ep-through-btn`, `.season-toggle` | compact | at least 44 px tall and 44 px wide, the Apple touch-target minimum, keeping their desktop font size |
| Page frame | `.layout`, `.main`, `.topnav` | `min-height: 100vh` then `min-height: 100dvh` as a base rule at every width (the second wins where supported, so the Safari toolbar never leaves a scroll gap); 1.5rem padding | 1rem side gutters widened to the side safe-area insets where a notch reports them (`max(1rem, env(safe-area-inset-left))` and the right-hand twin, on the bar and on `.main`); the bar's top padding adds `env(safe-area-inset-top)` and `.main`'s bottom padding adds `env(safe-area-inset-bottom)`, which are zero in an ordinary browser tab and non-zero in standalone mode |
| Inputs | `.search-input` | 1rem | unchanged: 16 px is the size below which iOS Safari zooms the page on focus |

Flex children that hold text carry `min-width: 0` so a long title shrinks instead of widening its row; the calendar grid already uses `minmax(0, 1fr)` columns. Nothing in the layout clips: a page element that cannot fit shrinks or wraps.

Arrangements specific to one page (the Up Next row's action button, the episode rows of a season, the calendar's agenda list) belong to that page's segment and are described in its design; their rules still live in the same media block of this stylesheet.

**Verification.** The unit test environment (happy-dom) does not lay out or apply stylesheets, so narrow-width behaviour is pinned two ways: `useNarrow` is tested with `matchMedia` mocked (initial value, change subscription, absence of the API), `Layout` is tested with `scrollIntoView` spied (called on the active link only while narrow, not called with no active link), and frontend/src/styles.test.ts reads index.css and index.html as text and asserts that exactly one `max-width` media query exists with the 900 px value, that `useNarrow` carries the same query, that the required rules appear inside the block, and that the head carries the viewport, theme-color, manifest, and touch-icon tags. A look at a real phone (or a 390 px device emulation) is part of review for any change inside the media block.

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
| Narrow navigation | Two-row bar with a sideways-scrolling link strip | Bottom tab bar; hamburger or "More" menu; wrapping the links onto several rows | Seven destinations overflow a tab bar at phone width and a menu hides them (tenet 5); wrapped rows cost up to three rows of bar on every page. A strip keeps every link one swipe away inside the bar, with the active one scrolled into view. |
| Breakpoint | One, `max-width: 900px`, shared by the stylesheet and `useNarrow` | 840 px; two breakpoints (phone and tablet); container queries per component | One shape change is enough for a nine-page app and keeps the stylesheet readable. The desktop bar needs about 860 px, so 840 would leave a band where the desktop layout overflows; tablets in portrait overflow it anyway. |
| Phone layouts | Rearrange the desktop markup in one media block; switch markup only via `useNarrow` where no arrangement fits | A separate mobile shell or page variants; a CSS framework | One set of markup and tests to keep correct; the calendar is the only view whose desktop shape has no readable narrow arrangement. |
| Touch targets | 44 px minimum height for controls only under the breakpoint | 44 px everywhere; leave as is | Desktop keeps its density; phones get Apple's recommended minimum. |
| Hover styles | Wrapped in `@media (hover: hover)` | Unconditional hover rules | A tapped control on iOS otherwise keeps its hover colour until the next tap. |
| Horizontal overflow | Prevented by construction (the strip scrolls inside the bar; text children carry `min-width: 0`) | `overflow-x: hidden` on `html, body` | Hiding overflow clips silently and can make `body` the scroll container, breaking the sticky bar in Safari. |
| Strip affordance | A right-edge mask on the strip with a trailing spacer; no left-edge fade | A fade overlay element; fades on both edges; arrows | A mask intercepts no taps and needs no script; the strip starts at Up Next and centring the active link reveals its neighbours, so a left cue adds little. |
| Strip scrollbar | Hidden only where `(hover: none)` | Hidden everywhere; shown everywhere | Touch users scroll by swipe and the fade shows there is more; a mouse user in a narrow window needs the bar. |
| Home-screen install | Manifest, `theme-color`, and an `apple-touch-icon`; no service worker | Full PWA with offline cache; nothing | The install affordance and a proper icon cost three static files; offline caching is pointless for a LAN app whose every view is live data. |
| Narrow-layout verification | Component tests with mocked `matchMedia` plus a stylesheet-text test | A Playwright or browser-mode CI job; review only | A browser job is a `build-ship` change out of proportion to one media block; a text test at least pins that the rules exist at the agreed breakpoint. |

## Open Questions & Future Decisions

### Resolved
1. ✅ **Responsive layout.** One narrow breakpoint at 900 px; the top bar becomes a brand row over a sideways-scrolling link strip; shared patterns rearrange in a single media block; the calendar alone switches markup (see *Narrow-width layout*).

### Deferred
1. **Configuration outside `Config`.** `STATIC_DIR` (`run` in lib.rs) and `DB_MAX_CONNECTIONS` (`create_pool` in pool.rs) bypass `Config::from_env`; `STATIC_DIR` is passed by compose but documented in neither env.example nor README.md, and `DB_MAX_CONNECTIONS` is documented in both but missing from CLAUDE.md and not passed by compose.
2. **CORS origin validation.** An unparsable `CORS_ALLOWED_ORIGIN` panics (`build_cors_layer`) instead of returning a `Config` error like every other setting; the panic is pinned by a test.
3. **Silent host fallback.** A bad `SERVER_HOST` binds `0.0.0.0` without a warning (`run` in lib.rs).
4. **Timeout versus long handlers.** The 30 s timeout drops the add and manual-sync futures mid-work; the resync gate stays consumed.
5. **`Config` error text reaches clients** as a 500 body; handlers use the variant for impossible states.
6. **`DELETE` and the content-type gate.** HTML forms cannot issue `DELETE`, so the gap is theoretical, but worth stating as a rule.
7. **No catch-all route.** An unknown SPA path renders an empty shell rather than a not-found page.
8. **Light theme only.** No dark mode or `prefers-color-scheme` handling.
11. **No scroll restoration.** A route change keeps the previous scroll position (React Router's default), so landing on a long page mid-scroll is possible.
12. **No in-app Back control in standalone mode.** Every page is one tap away in the strip and detail pages are reached from strip pages, so the gap is small; revisit if the iOS edge-swipe proves unreliable for a home-screen app.
9. **Duplicated page scaffolding.** The fetch-with-`cancelled` skeleton is hand-copied into eight pages with inconsistent loading, error, and empty structures.
10. **`lib.rs` has grown past the 300-line limit** set by the repository's file-size guideline.

## References

- backend/src/lib.rs; backend/src/main.rs (`--healthcheck` dispatch); backend/src/config.rs; backend/src/db/pool.rs; backend/src/error.rs; backend/src/api/health.rs (handler and probe)
- env.example; README.md "Configuration" and "Homelab / reverse proxy"; SECURITY.md "Security model" and "What the app does protect"
- frontend/src/main.tsx; frontend/src/App.tsx; frontend/src/components/Layout.tsx; frontend/src/hooks/useNarrow.ts; frontend/src/api/client.ts; frontend/src/types/index.ts; frontend/src/index.css; frontend/index.html; frontend/public/manifest.webmanifest, frontend/public/apple-touch-icon.png, frontend/public/icon-192.png, frontend/public/icon-512.png; frontend/vite.config.ts (dev proxy to 3001)
- Tests: backend/src/lib.rs tests (log filter, CORS branches incl. the panic, security headers; content-type matching is covered in backend/tests/api.rs), backend/src/config.rs tests, backend/src/db/pool.rs tests, backend/src/error.rs tests; backend/tests/api.rs `health_returns_ok_when_db_responsive` (health), `form_encoded_post_to_sync_is_rejected` through `get_requests_are_unaffected_by_the_content_type_gate` (content-type gate), `wildcard_branch_grants_no_more_than_the_named_origin_branch` through `named_origin_branch_still_denies_other_origins` (CORS); frontend/src/App.test.tsx, components/Layout.test.tsx, hooks/useNarrow.test.ts, styles.test.ts, api/client.test.ts
- Consumers: every feature segment (routes, pages, `AppState`, `Config`, error mapping)
