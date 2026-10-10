---
parent: high-level-design
prefix: TMDB
---

# TMDB access

## Context and Design Philosophy

TMDB is the only external data source. This segment is the adapter: one HTTP client, the subset of TMDB's response shapes the app reads, the helpers that reduce those shapes to what the app stores (US providers, network names, directors, image URLs), and the handling that keeps the API key out of anything a user or log could see. Every outbound request carries the operator's key, so the adapter also owns the rules that keep upstream trouble from turning into either a leak or an unreadable error.

## Client

`TmdbClient` (backend/src/datasources/tmdb.rs) wraps one `reqwest::Client` with a 10 s timeout (`TMDB_HTTP_TIMEOUT`), the API key, and a base URL that defaults to `https://api.themoviedb.org/3` and is overridable only through `with_base_url` (used by tests with a mock server; no environment override exists). The key is sent as the `api_key` query parameter on every request, which is TMDB's v3 authentication.

| Method | Request | Not found |
|---|---|---|
| `get_show(id)` | `GET /tv/{id}?append_to_response=watch/providers` | 404 → `NotFound("show N not found on TMDB")` |
| `get_movie(id)` | `GET /movie/{id}?append_to_response=credits,watch/providers` | 404 → `NotFound("movie N not found on TMDB")` |
| `get_season(id, n)` | `GET /tv/{id}/season/{n}` | no mapping; 404 falls through to the generic text with the `season N` context |
| `search_multi(query)` | `GET /search/multi?query=…&include_adult=false` | no mapping; 404 falls through to the generic text |

Every method sends its response through one status mapper before deserializing. After the per-method 404 rule, the mapper turns 429 into `Upstream("TMDB is rate-limiting requests right now. Please try again in a moment.")`, any 5xx into `Upstream("TMDB is unavailable right now. Please try again shortly.")`, and any other non-2xx into `Upstream("TMDB returned <status>")` (`TMDB season <n> returned <status>` for a season). The perimeter maps `NotFound` to HTTP 404 and `Upstream` to 502, so a rate limit and an outage are both 502s whose body already reads as a sentence; nothing downstream rewrites it. The `/sync` per-show results pass through the same `client_message()`, so the Settings error list carries the same sentences.

The client is the only holder of the API key and the only module that builds a TMDB URL. `base_url()` remains for tests; there is no accessor for the key or the underlying HTTP client.

## Response handling

- **Body cap** — `json_within_cap` refuses a response whose advertised `Content-Length` exceeds 16 MiB (`MAX_TMDB_BODY_BYTES`) with `Upstream("TMDB response was unexpectedly large")` before buffering. Chunked bodies with no length are not capped; the 10 s timeout bounds them.
- **Shapes** (`TmdbSearchResponse` through `TmdbProvider` in tmdb.rs) — only the fields the app uses are declared. Collections (`seasons`, `networks`, `episodes`, `cast`, `crew`, provider tiers) and the `watch/providers` and `credits` attachments default to empty when absent, so a sparse TMDB record deserializes rather than failing. Movies use `title` where shows use `name`.
- **Reductions** — `us_providers()` on shows and movies (`TmdbShow::us_providers` and `TmdbMovie::us_providers`, both delegating to `us_providers_from`) takes the `US` region only, concatenates the `flatrate`, `free`, and `ads` tiers, and returns provider names sorted and de-duplicated; `rent` and `buy` are ignored. `network_names()` (`TmdbShow::network_names`) keeps TMDB's order, trims, and drops blanks and duplicates. `directors()` (`TmdbMovie::directors`) is every crew member whose `job` is exactly `Director`.
- **Image URLs** — `poster_url` (w185) and `backdrop_url` (w780) in backend/src/models/show.rs and `profile_url` (w185) in backend/src/models/movie.rs prefix a TMDB path with the image CDN base and treat an empty path as none. The search handler carries its own copy of the w185 base.

## Keeping the key out of errors

`reqwest::Error`'s `Display` appends the request URL, which would include `api_key=…`. The `From<reqwest::Error>` impl (backend/src/error.rs) therefore stores `err.without_url()`, so the key never enters an `AppError` in the first place. Downstream, `client_message()` collapses the `Http` variant to a generic string anyway, and `IntoResponse` logs it via `Display`, which is now URL-free. `Config` holds the key as a plain `String` with `#[derive(Debug)]`; nothing formats it, but nothing prevents it either.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| Authentication | v3 API key as `api_key` query parameter | v4 bearer token in a header | `[inferred]` The v3 key is what TMDB's older endpoints document; the cost is that every URL carries a secret, hence the URL stripping. |
| URL in errors | Stripped at conversion with `without_url()` | Redact in `Display`; filter at response time | The secret must never reach the error object at all, since handlers surface error strings to clients and logs (the doc comment on `From<reqwest::Error>` in error.rs). |
| Body size | Reject when `Content-Length` > 16 MiB | Stream with a hard byte cap | Guards a misbehaving upstream without buffering; chunked bodies fall back to the timeout (the doc comment on `MAX_TMDB_BODY_BYTES`). |
| Timeout | 10 s per request | Longer for season fetches | `[inferred]` Keeps the serial add and resync paths bounded. |
| Upstream errors to users | One status mapper shared by every client method: fixed sentences for 429 and 5xx, raw `TMDB returned <status>` otherwise | Raw status text; client-side mapping per page; per-method mapping | A rate limit or outage should read as a transient condition, not a status dump, and mapping once on the server keeps every page and the Settings sync list consistent. Other statuses (a 401 from a bad key, say) are operator problems where the status is the useful part. |
| Search request | A `search_multi` client method | Assemble the request in the search handler from client accessors | Every outbound call goes through the mapper, and the key never leaves this module. |
| Provider regions and tiers | `US` only; `flatrate` + `free` + `ads` | All regions; include `rent` / `buy` | `[inferred]` Single-household app; "where can I stream it" excludes purchase. The tier choice is noted on `TmdbShow::us_providers`. |
| Deserialized fields | Only what the app reads, with defaults for collections | Full TMDB models | Resilient to sparse records and TMDB additions (the `TMDB response shapes` section of tmdb.rs). |
| Retries | None | Backoff on 429 / 5xx | `[inferred]` Serial callers and the manual-sync cooldown already pace requests. |
| Season 404 | Reported as `Upstream`, not `NotFound` | Map 404 like show and movie | `[inferred]` A season listed by the show but missing from TMDB is an upstream inconsistency rather than a user error. |

## Open Questions & Future Decisions

### Resolved
*(none yet)*

### Deferred
1. **`get_season` 404.** Should a missing season map to `NotFound` for symmetry?
2. **Chunked bodies** bypass the size cap.
3. **No retry or backoff**; a transient 5xx fails the whole add or the show's resync.
4. **Key visibility in `Config`.** `#[derive(Debug)]` on a struct holding the key invites an accidental `{:?}`.
5. **Three copies of the w185 base** (models/show.rs, models/movie.rs, api/search.rs).
6. **No base-URL override outside tests.**

## References

- backend/src/datasources/tmdb.rs (client, status mapper, shapes, reductions, tests)
- backend/src/error.rs (`From<reqwest::Error>`, `AppError::client_message`)
- backend/src/models/show.rs (`poster_url`, `backdrop_url`), backend/src/models/movie.rs (`profile_url`) (image URL helpers)
- backend/tests/api.rs (`get_movie_detail_maps_tmdb_429_to_friendly_message` for 429 on movie detail, `search_returns_502_with_unavailable_message_on_tmdb_5xx` for 502 on search)
- Consumers: `search` (`search_multi`, `json_within_cap`), `shows` (`get_show`, `get_season`, reductions), `movies` (`get_movie`, reductions), `resync` (`get_show`, `get_season`)
