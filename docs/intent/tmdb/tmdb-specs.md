# TMDB access — EARS specs

Prefix `TMDB`. Facets: `CLIENT` (requests and response handling), `SHAPE` (reductions of TMDB data), `ERR` (error mapping and secret handling). "The TMDB client" means `TmdbClient` in backend/src/datasources/tmdb.rs.

## Client

- [x] **TMDB-CLIENT-001**: The TMDB client shall send every request to the base URL `https://api.themoviedb.org/3` with the operator's API key as the `api_key` query parameter and a 10-second request timeout.
- [x] **TMDB-CLIENT-002**: When asked for a show, the TMDB client shall request `GET /tv/{id}` with `append_to_response=watch/providers`.
- [x] **TMDB-CLIENT-003**: When asked for a movie, the TMDB client shall request `GET /movie/{id}` with `append_to_response=credits,watch/providers`.
- [x] **TMDB-CLIENT-004**: When asked for a season, the TMDB client shall request `GET /tv/{id}/season/{n}`.
- [x] **TMDB-CLIENT-007**: When asked to search, the TMDB client shall request `GET /search/multi` with `query` set to the given text and `include_adult=false`.
- [x] **TMDB-CLIENT-008**: The TMDB client shall expose no accessor for the API key or the underlying HTTP client; only the base URL is readable.
- [x] **TMDB-CLIENT-005**: If a TMDB response advertises a `Content-Length` greater than 16 MiB, then the TMDB client shall reject it with the upstream error `TMDB response was unexpectedly large` before reading the body.
- [x] **TMDB-CLIENT-006**: The TMDB client shall deserialize only the fields the application uses and shall treat absent `seasons`, `networks`, `episodes`, `credits`, `cast`, `crew`, and `watch/providers` as empty rather than failing.

## Shape

- [x] **TMDB-SHAPE-001**: When reducing a show's or movie's watch providers, the system shall take only the `US` region, combine the `flatrate`, `free`, and `ads` tiers, and return the provider names sorted and de-duplicated, yielding an empty list when no `US` entry exists; `rent` and `buy` tiers shall be ignored.
- [x] **TMDB-SHAPE-002**: When reducing a show's networks, the system shall return the network names in TMDB's order with surrounding whitespace trimmed and blank or duplicate names removed.
- [x] **TMDB-SHAPE-003**: When reducing a movie's directors, the system shall return the names of crew members whose job is exactly `Director`, in TMDB's order.
- [x] **TMDB-SHAPE-004**: The system shall build poster and profile image URLs on `https://image.tmdb.org/t/p/w185` and backdrop URLs on `https://image.tmdb.org/t/p/w780`, returning no URL when the TMDB path is absent or empty.

## Errors

- [x] **TMDB-ERR-001**: If TMDB responds 404 to a show or movie request, then the TMDB client shall return a not-found error naming the id (`show <id> not found on TMDB` / `movie <id> not found on TMDB`), which the API surfaces as HTTP 404.
- [x] **TMDB-ERR-002**: If TMDB responds 429 to any request made by the TMDB client, then the client shall return an upstream error whose message is the user-readable sentence `TMDB is rate-limiting requests right now. Please try again in a moment.` rather than the raw status text.
- [x] **TMDB-ERR-003**: If TMDB responds with any 5xx status to any request made by the TMDB client, then the client shall return an upstream error whose message is the user-readable sentence `TMDB is unavailable right now. Please try again shortly.` rather than the raw status text.
- [x] **TMDB-ERR-004**: If TMDB responds with a non-2xx status that is not 404 on a show or movie request, not 429, and not 5xx, then the TMDB client shall return an upstream error carrying `TMDB returned <status>` (or `TMDB season <n> returned <status>` for a season), which the API surfaces as HTTP 502.
- [x] **TMDB-ERR-005**: When an outbound TMDB request fails at the HTTP layer, the system shall store the error without its request URL so that the API key never appears in any error message, log line, or response body.
