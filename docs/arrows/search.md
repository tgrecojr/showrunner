# Arrow: search

TMDB multi-search proxied through the backend, plus the Search page with its per-result Add flow and the `already_tracked` flag.

## Status

**MAPPED** — sampled 2026-10-10 (git SHA `3b525e1`), not yet audited. Skeleton LLD and EARS specs were reverse-engineered from code; design rationale carries `[inferred]` markers until confirmed.

## References

### HLD
- docs/high-level-design.md (System Design → search)

### LLD
- docs/intent/search/search-design.md

### EARS
- docs/intent/search/search-specs.md (22 specs: 21 implemented, 1 active gap)

### Tests
- backend/tests/api.rs — `search_returns_mixed_results_with_already_tracked_flag` (:91), `search_rejects_blank_query` (:146), `search_returns_502_when_tmdb_errors` (:156)
- frontend/src/pages/Search.test.tsx — 12 tests (debounce, result rendering, add TV vs movie, per-result errors)
- frontend/src/api/client.test.ts — URL construction and query encoding for `api.search` (:29, :37)

### Code
- backend/src/api/search.rs — `search_shows` handler (the whole file)
- frontend/src/pages/Search.tsx — Search page
- frontend/src/api/client.ts — `api.search` (:36)
- Consumed from other segments: `queries::tracked_tmdb_ids_in` (backend/src/db/queries.rs:427), `queries::tracked_movie_tmdb_ids_in` (:412), `tmdb::json_within_cap` (backend/src/datasources/tmdb.rs:18), `api.addShow` / `api.addMovie` (frontend/src/api/client.ts:40, :49)

## Architecture

**Purpose:** Let the user find a TV show or movie on TMDB and add it to the right list, without the TMDB API key ever reaching the browser.

**Key Components:**
1. `search_shows` handler — validates `q`, calls TMDB `/search/multi` with `include_adult=false`, drops non-tv/movie results, normalizes TV vs movie fields, annotates each result with `already_tracked` from two IN-list lookups.
2. Search page — 350 ms debounced query, per-result add state keyed by `media_type:tmdb_id`, routes Add to `addMovie` or `addShow` by media type, shows `On watchlist` for tracked or just-added results.
3. Tracked-id lookups — owned by `shows` and `movies` (in `queries.rs`); this segment only consumes them.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| API | SEARCH-API-001 to 010 | 10 | 0 | 0 |
| UI | SEARCH-UI-001 to 012 | 12 | 0 | 0 |

**Summary:** 22 of 22 active specs implemented; no gaps.

## Key Findings

1. **Non tv/movie results are dropped silently** — `person` and any unknown `media_type` are filtered at search.rs:90-94 and :110-114, pinned by api.rs:112.
2. **Add crosses the segment boundary** — Search.tsx:66-70 calls `api.addMovie` / `api.addShow`, whose endpoints belong to `movies` and `shows`. The Search page owns only the button states.
3. **Debounce without cancellation** — Search.tsx:25-50 clears the timer and ignores late results via a `cancelled` flag, but no `AbortController`; previous results stay visible while a new search runs (:120 shows "Searching…" above them), and `loading` is only set once the 350 ms timer fires (:26).
4. **Per-result add errors render without the `Error:` prefix** every other page uses (Search.tsx:155; contrast UpNext.tsx and Watchlist.tsx).
5. **`q` is trimmed and required but has no length cap** (search.rs:58-63); it is a query parameter, so the 1 MiB body limit does not apply.
6. **Poster base URL duplicated** — search.rs defines `POSTER_BASE` separately from `models::show::poster_url` (backend/src/models/show.rs:4).

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` decisions in the LLD (why add stays on the Search page rather than navigating).

### Should Fix

### Nice to Have
2. Cap `q` length server-side.
3. Abort in-flight searches on query change.
4. Reuse `models::show::poster_url` instead of a local base constant.
