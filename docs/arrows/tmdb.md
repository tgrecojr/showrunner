# Arrow: tmdb

The adapter for the only external data source: the HTTP client, the TMDB response shapes the app reads, the reductions to providers, networks, directors, and image URLs, and the rules that keep the API key out of errors.

## Status

**AUDITED** — last audited 2026-10-10 (git SHA `4da220e`). All 17 specs are implemented and annotated in code and tests; nothing is deferred or open, though `TMDB-CLIENT-008` (no accessor for the key) is verified by inspection rather than by a test; that property is verified by review and the coherence script lists it as such.

## References

### HLD
- docs/high-level-design.md (System Design → tmdb)

### LLD
- docs/intent/tmdb/tmdb-design.md

### EARS
- docs/intent/tmdb/tmdb-specs.md (17 specs: 17 implemented, 0 deferred, 0 active gaps)

### Tests
- backend/src/datasources/tmdb.rs inline `#[cfg(test)]` module — 23 tests, wholly owned by this segment: wiremock-driven request shape and key placement (`get_show_parses_response`, `get_movie_parses_response`, `get_season_parses_episodes`, `search_multi_sends_query_key_and_adult_filter`, `new_uses_real_tmdb_base_url`, `with_base_url_overrides`), status mapping (`get_show_404_maps_to_not_found`, `get_movie_404_maps_to_not_found`, `get_show_429_maps_to_rate_limit_message`, `get_movie_429_maps_to_rate_limit_message`, `get_show_500_maps_to_unavailable_message`, `get_movie_500_maps_to_unavailable_message`, `get_season_429_and_5xx_map_to_friendly_messages`, `get_season_non_2xx_maps_to_upstream`, `get_show_401_keeps_raw_status_text`, `get_season_other_status_names_the_season`), body cap (`oversized_body_is_rejected_before_it_is_read`), key hygiene (`transport_error_does_not_leak_api_key`), and reductions (`us_providers_returns_empty_when_no_wrapper`, `us_providers_returns_empty_when_no_us_region`, `us_providers_concatenates_flatrate_free_and_ads_sorted_unique`, `network_names_keeps_order_and_drops_blanks_and_duplicates`, `network_names_is_empty_when_field_missing`)
- backend/src/models/show.rs inline tests — `poster_url_prepends_base_when_path_present`, `poster_url_treats_empty_string_as_none`, `poster_url_returns_none_for_none`, `backdrop_url_uses_w780_base`
- backend/tests/api.rs — `search_returns_mixed_results_with_already_tracked_flag`, `search_returns_502_with_unavailable_message_on_tmdb_5xx`, `search_returns_502_with_rate_limit_message_on_tmdb_429`, `search_returns_502_with_raw_status_on_other_tmdb_errors`, `add_show_returns_404_when_tmdb_missing`, `sync_returns_per_show_results`, `add_movie_fetches_from_tmdb_and_inserts`, `add_movie_returns_404_when_tmdb_missing`, `get_movie_detail_returns_cast_and_providers`, `get_movie_detail_maps_tmdb_429_to_friendly_message`

### Code
- backend/src/datasources/tmdb.rs — `json_within_cap`, `TmdbClient` (struct, `new`, `with_base_url`, private `get`, `get_show`, `get_movie`, `get_season`, `search_multi`), `map_status`, the deserialized shapes `TmdbShow`, `TmdbSeason`, `TmdbMovie`, `TmdbCredits`, and the reductions `TmdbShow::us_providers`, `TmdbShow::network_names`, `TmdbMovie::us_providers`, `TmdbMovie::directors`
- backend/src/error.rs — `impl From<reqwest::Error> for AppError` (URL stripping)
- backend/src/models/show.rs — `poster_url`, `backdrop_url`
- backend/src/models/movie.rs — `profile_url`
- backend/src/api/search.rs — `POSTER_BASE` (the search handler's own copy of the w185 image base)
- Consumed from other segments: `AppError` variants (`NotFound`, `Upstream`, `Http`) and their HTTP status mapping and `client_message()` collapse (`app`, backend/src/error.rs)
- Consumers: `search` (`search_multi`, `json_within_cap`; backend/src/api/search.rs), `shows` (`get_show`, `get_season`, `us_providers`, `network_names`, `poster_url`, `backdrop_url`; backend/src/api/shows.rs), `movies` (`get_movie`, `us_providers`, `directors`, `profile_url`; backend/src/api/movies.rs), `resync` (`get_show`, `get_season`, `network_names`; backend/src/logic/resync.rs)

## Architecture

**Purpose:** Talk to TMDB on the user's behalf without leaking the key, and hand the rest of the app small, stable shapes.

**Key Components:**
1. `TmdbClient` — four request methods (show, movie, season, search) over one private `get` that is the only place the key is used; the key is the `api_key` query parameter, the timeout is 10 s, and only the base URL is readable (and overridable only through `with_base_url`, for tests).
2. `json_within_cap` and the minimal deserialized shapes with empty-collection defaults; a `Content-Length` over 16 MiB is refused before the body is read.
3. Reductions — US providers (`flatrate` + `free` + `ads`, sorted, de-duplicated), network names (TMDB order, trimmed, no blanks or duplicates), directors (`job == "Director"`), and the w185/w780 image URL helpers.
4. Error conversion — URL stripped at the `reqwest::Error` boundary; one shared status mapper (`map_status`: fixed sentences for 429 and 5xx, raw `TMDB returned <status>` otherwise) after each method's 404 rule. Show and movie 404s become `NotFound`; a season 404 falls through to `TMDB season <n> returned 404 Not Found`.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Client | TMDB-CLIENT-001 to 008 | 8 | 0 | 0 |
| Shape | TMDB-SHAPE-001 to 004 | 4 | 0 | 0 |
| Errors | TMDB-ERR-001 to 005 | 5 | 0 | 0 |

**Summary:** 17 of 17 active specs implemented; 0 deferred. (Specs with no test citation: TMDB-CLIENT-008.)

## Key Findings

1. **`TMDB-CLIENT-008` has no test** — "no accessor for the key or the HTTP client" is a property of `TmdbClient`'s private fields (backend/src/datasources/tmdb.rs:`TmdbClient`) that no runtime test asserts; it holds by inspection today.
2. **`get_season` has no 404 mapping** (backend/src/datasources/tmdb.rs:`get_season`); a missing season is a 502 carrying `TMDB season <n> returned 404 Not Found`, as `get_season_other_status_names_the_season` pins. See LLD Open Questions & Future Decisions → Deferred #1.
3. **Chunked responses are not size-capped** (backend/src/datasources/tmdb.rs:`json_within_cap`); only the 10 s timeout bounds them. See LLD Deferred #2.
4. **No retry, backoff, User-Agent, or caching** anywhere in the client (backend/src/datasources/tmdb.rs:`TmdbClient::get`). See LLD Deferred #3.
5. **`Config` derives `Debug` with the key in a plain `String`** (backend/src/config.rs:`Config`); nothing formats it today. See LLD Deferred #4.
6. **Credential naming mismatch in docs** — README.md's prerequisites line tells users to copy the "API Read Access Token (v3 auth)"; TMDB labels the v3 credential "API Key" and the Read Access Token is the v4 bearer token, while the code sends a v3 key as `api_key` (backend/src/datasources/tmdb.rs:`TmdbClient::get`). The Configuration table in the same README correctly says "TMDB v3 API key". See LLD Deferred #5.
7. **Three copies of the w185 image base** (backend/src/models/show.rs:`POSTER_BASE`, backend/src/models/movie.rs:`PROFILE_BASE`, backend/src/api/search.rs:`POSTER_BASE`). See LLD Deferred #6.

## Work Required

### Must Fix
*(none)*

### Should Fix
*(none)*

### Nice to Have
1. Consolidate the image base constants (LLD Deferred #6).
