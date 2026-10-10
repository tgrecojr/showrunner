# App perimeter and shell — EARS specs

Prefix `APP`. Facets: `CONFIG` (startup configuration), `HTTP` (the API perimeter and static delivery), `ERR` (error responses), `HEALTH`, `SPA` (router, navigation, fetch wrapper).

## Config

- [x] **APP-CONFIG-001**: On startup the system shall load a `.env` file from the working directory when present, then read `SERVER_HOST` (default `0.0.0.0`), `SERVER_PORT` (default `3001`), `DATABASE_URL` (default `sqlite:///data/showrunner.db`), `TMDB_API_KEY` (required), `RESYNC_CRON` (default `0 0 6 * * *`), `TIMEZONE` (default `America/New_York`), and `CORS_ALLOWED_ORIGIN` (optional; blank reads as unset).
- [x] **APP-CONFIG-002**: If `SERVER_PORT` is not a valid u16, `TMDB_API_KEY` is missing or blank, `TIMEZONE` is not a valid IANA zone, `DATABASE_URL` cannot be parsed, or `RESYNC_CRON` cannot be parsed, then startup shall fail with a configuration error naming the variable before the server socket is bound.
- [x] **APP-CONFIG-003**: On startup the system shall open the SQLite database creating the file if missing, with foreign keys enabled, WAL journaling, `synchronous=NORMAL`, a 30-second busy timeout, and a pool of `DB_MAX_CONNECTIONS` connections (default 5), and shall apply all embedded migrations before serving requests, failing startup if a migration fails.
- [x] **APP-CONFIG-004**: When the directory named by `STATIC_DIR` (default `./static`) contains an `index.html`, the system shall serve that directory for every non-API path with `index.html` as the fallback for unknown paths; otherwise it shall log a warning and serve the API only.
- [x] **APP-CONFIG-005**: The system shall configure log filtering from `RUST_LOG`, defaulting to `info`.

## HTTP

- [x] **APP-HTTP-001**: The system shall expose every API route under the `/api/v1` prefix and shall treat any other path as a static or SPA path.
- [x] **APP-HTTP-002**: If a `POST`, `PUT`, or `PATCH` request to the API does not carry a `Content-Type` of `application/json` or `application/<subtype>+json` (ignoring parameters such as `charset`), then the system shall respond 415 before invoking any handler; `GET` and `DELETE` requests shall not be subject to this check.
- [x] **APP-HTTP-003**: While 64 API requests are already in flight, the system shall respond 503 to further API requests immediately rather than queueing them.
- [x] **APP-HTTP-004**: If an API request body exceeds 1 MiB, then the system shall reject the request.
- [x] **APP-HTTP-005**: If an API request has not completed within 30 seconds, then the system shall respond 408.
- [x] **APP-HTTP-006**: The system shall allow cross-origin API requests from `http://localhost:<SERVER_PORT>` when `CORS_ALLOWED_ORIGIN` is unset, from exactly that origin when it names one, and from any origin when it is `*`; in every case it shall advertise exactly the methods `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, allow any request header, never allow credentials, and never expose response headers with a wildcard.
- [x] **APP-HTTP-007**: If `CORS_ALLOWED_ORIGIN` is set to a value that is not a valid origin header value, then startup shall fail.
- [x] **APP-HTTP-008**: The system shall add to every response, API and static alike, unless already present: a `Content-Security-Policy` restricting scripts, connections, base URI, and form actions to same-origin, allowing images from self, `https://image.tmdb.org`, and `data:`, allowing inline styles, and denying all framing; `X-Content-Type-Options: nosniff`; `X-Frame-Options: DENY`; and `Referrer-Policy: no-referrer`.

## Errors

- [x] **APP-ERR-001**: The system shall respond to a handler error with a JSON body `{"error": <message>}` and the status 404 for not-found, 400 for invalid data, 502 for upstream failures, 429 for rate limiting, and 500 for every other error.
- [x] **APP-ERR-002**: When a handler error originates from the database, an outbound HTTP failure, IO, or JSON processing, the system shall use the message `An internal error occurred` in the response and log the underlying error at error level; for not-found, invalid-data, upstream, rate-limit, and configuration errors it shall pass the error's own message through and log nothing.

## Health

- [x] **APP-HEALTH-001**: `GET /api/v1/health` shall probe the database with `SELECT 1` under a 2-second timeout and respond with `{ status: "ok", version, database: true }` when the probe succeeds and `{ status: "degraded", version, database: false }` when it fails or times out.
- [x] **APP-HEALTH-002**: When the health probe reports `degraded`, `GET /api/v1/health` shall respond with HTTP 503; when it reports `ok` it shall respond with HTTP 200.
- [x] **APP-HEALTH-003**: When the backend binary is started with the single argument `--healthcheck`, it shall request `GET /api/v1/health` on `SERVER_PORT` (default 3001) at `SERVER_HOST` (default and unspecified addresses resolving to loopback) with a 3-second timeout, exit 0 when the response is HTTP 200, and exit 1 when the response is any other status, the connection fails, or the timeout elapses, without reading `.env`, loading `Config`, or opening the database.

## SPA

- [x] **APP-SPA-001**: The SPA shall route `/` to Up Next, `/watchlist` to Watchlist, `/movies` to Movies, `/movies/:tmdbId` to MovieDetail, `/search` to Search, `/shows/:tmdbId` to ShowDetail, `/calendar` to Calendar, `/history` to History, and `/settings` to Settings, all inside the shared layout.
- [x] **APP-SPA-002**: The layout shall render a brand link to `/` and navigation links in the order Up Next, Watchlist, Movies, Search, Calendar, History, Settings, marking the current route's link active, with Up Next active only at exactly `/`.
- [x] **APP-SPA-003**: The API client shall prefix every path with `/api/v1`, send `Content-Type: application/json` on every request, send no credentials, resolve a 204 response to no value, and parse any other successful response as JSON.
- [x] **APP-SPA-004**: If an API response is not successful, then the API client shall reject with an `ApiError` (an `Error` subclass) whose `status` is the HTTP status code and whose message is the body's `error` field when the body is JSON with that field and the raw body text otherwise, with no prefix.
- [x] **APP-SPA-005**: The API client shall URL-encode query parameter values it sends.
- [x] **APP-SPA-006**: When a page displays an API error to the user, it shall show the error's message exactly as the API client delivered it, without rewriting, prefixing, or interpreting it, on every page.
