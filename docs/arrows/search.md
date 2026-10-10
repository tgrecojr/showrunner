# Arrow: search

TMDB multi-search proxied through the backend, plus the Search page with its per-result Add flow and the `already_tracked` flag.

## Status

**AUDITED** — last audited 2026-10-10 (git SHA `4da220e`). All 22 specs implemented and annotated in code and tests; nothing is open beyond the LLD's deferred items.

## References

### HLD
- docs/high-level-design.md (System Design → search)

### LLD
- docs/intent/search/search-design.md

### EARS
- docs/intent/search/search-specs.md (22 specs: 22 implemented, 0 deferred, 0 active gaps)

### Tests
- backend/tests/api.rs — `search_returns_mixed_results_with_already_tracked_flag`, `search_rejects_blank_query`, `search_returns_502_with_unavailable_message_on_tmdb_5xx`, `search_returns_502_with_rate_limit_message_on_tmdb_429`, `search_returns_502_with_raw_status_on_other_tmdb_errors`
- frontend/src/pages/Search.test.tsx — 16 tests (input focus and label, debounce, blank-query clearing, in-flight and error status lines, card rendering, add TV vs movie, pending/added/error states, state keying and reset)

### Code
- backend/src/api/search.rs — `search_shows` handler (the whole file)
- backend/src/db/queries.rs — `tracked_tmdb_ids_in`, `tracked_movie_tmdb_ids_in` (the two IN-list lookups behind `already_tracked`)
- frontend/src/pages/Search.tsx — `Search` page component
- Consumed from other segments: `TmdbClient::search_multi` and its status mapping and body cap (`tmdb`); `POSTER_BASE` in search.rs is annotated under `tmdb` (TMDB-SHAPE-004); `api.search` in frontend/src/api/client.ts and its URL-encoding tests are annotated under `app` (APP-SPA-003/005); `api.addShow` / `api.addMovie` and their endpoints (`shows`, `movies`)
- Consumers: none — no other segment calls into search.

## Architecture

**Purpose:** Let the user find a TV show or movie on TMDB and add it to the right list, without the TMDB API key ever reaching the browser.

**Key Components:**
1. `search_shows` handler — validates `q`, calls TMDB `/search/multi` with `include_adult=false`, drops non-tv/movie results, normalizes TV vs movie fields, annotates each result with `already_tracked` from two IN-list lookups.
2. Search page — 350 ms debounced query, per-result add state keyed by `media_type:tmdb_id`, routes Add to `addMovie` or `addShow` by media type, shows `On watchlist` for tracked or just-added results.
3. Tracked-id lookups — `tracked_tmdb_ids_in` (shows) and `tracked_movie_tmdb_ids_in` (movies) in `queries.rs`; both short-circuit to no query on an empty id list. Only search calls them, and they are annotated under this segment (SEARCH-API-006).

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| API | SEARCH-API-001 to 010 | 10 | 0 | 0 |
| UI | SEARCH-UI-001 to 012 | 12 | 0 | 0 |

**Summary:** 22 of 22 active specs implemented; 0 deferred. (Specs with no test citation: none.)

## Key Findings

1. **Add crosses the segment boundary** — `Search.tsx:handleAdd` calls `api.addMovie` / `api.addShow`, whose endpoints belong to `movies` and `shows`. The Search page owns only the button states.
2. **Debounce without cancellation** — `Search.tsx:Search` (the query effect) clears the timer and ignores late results via a `cancelled` flag but never aborts the HTTP request. LLD Deferred 3.
3. **Per-result add errors omit the `Error:` prefix** every other page uses (`Search.tsx:Search`, the `addErrors[key]` paragraph). LLD Deferred 4.
4. **`q` is trimmed and required but has no length cap** (`search.rs:search_shows`). LLD Deferred 1.
5. **Poster base URL duplicated** — `search.rs:POSTER_BASE` alongside `models/show.rs:poster_url`. LLD Deferred 5.

## Work Required

### Must Fix

### Should Fix

### Nice to Have
1. Cap `q` length server-side (LLD Deferred 1).
2. Abort in-flight searches on query change (LLD Deferred 3).
3. Reuse `models::show::poster_url` instead of a local base constant (LLD Deferred 5).
4. Decide whether per-card add errors should carry the `Error:` prefix (LLD Deferred 4).
