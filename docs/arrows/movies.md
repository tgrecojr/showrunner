# Arrow: movies

The flat movie to-watch list: add, list, detail with live TMDB credits, mark watched (which deletes and logs), and remove (which deletes silently).

## Status

**MAPPED** — sampled 2026-10-10 (git SHA `3b525e1`), not yet audited. Skeleton LLD and EARS specs were reverse-engineered from code; design rationale carries `[inferred]` markers until confirmed.

## References

### HLD
- docs/high-level-design.md (System Design → movies)

### LLD
- docs/intent/movies/movies-design.md

### EARS
- docs/intent/movies/movies-specs.md (21 specs: 20 implemented, 1 active gap)

### Tests
- backend/tests/api.rs — list/add/detail/delete (:595-782), mark-watched and delete log behavior (:1272-1302)
- backend/tests/db.rs — list cap (:577), `mark_movie_watched` and `delete_movie` log behavior (:797-822)
- backend/src/models/movie.rs unit test (request shape)
- frontend/src/pages/Movies.test.tsx (8 tests), frontend/src/pages/MovieDetail.test.tsx (9 tests)
- Not covered: `App.test.tsx` has no movie routes; `client.test.ts` has none of the movie client methods

### Code
- backend/src/api/movies.rs; backend/src/models/movie.rs
- backend/src/db/queries.rs — `movie_exists` (:286), `insert_movie` (:294), `list_movies` (:315), `get_movie` (:342), `delete_movie` (:363), `mark_movie_watched` (:374), `tracked_movie_tmdb_ids_in` (:412)
- backend/src/db/migrations/20260513000000_add_movies.sql
- frontend/src/pages/Movies.tsx, frontend/src/pages/MovieDetail.tsx; frontend/src/api/client.ts (:48-57)
- Consumed from other segments: `tmdb` (`get_movie`, `us_providers`, `directors`, 429 mapping), `watch-log` (`insert_entry`)

## Architecture

**Purpose:** Keep a simple queue of movies to watch, and distinguish "watched it" from "changed my mind" so History stays honest.

**Key Components:**
1. Eight-column `movies` row captured at add time.
2. Detail merge — stored row plus a live TMDB credits/providers fetch on every view.
3. Two exits — `watched` (delete + log, one transaction) and `DELETE` (delete only).
4. Pages — Movies grid with local removal and an action-error banner over the grid, MovieDetail with the only client-side friendly-error mapping in the app.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| API | MOVIES-API-001 to 011 | 11 | 0 | 0 |
| UI | MOVIES-UI-001 to 010 | 10 | 0 | 0 |

**Summary:** 21 of 21 active specs implemented; no gaps.

## Key Findings

1. **Detail depends on TMDB at view time** — `get_movie_detail` calls TMDB on every request (backend/src/api/movies.rs:34); a listed movie's page fails with 502 during an outage, and stored metadata is never refreshed because movies are outside resync.
2. **Friendly error mapping is page-local** — `friendlyError` (MovieDetail.tsx:6-15) exists only here, although the intent is app-wide.
3. **Check-then-insert on add** (movies.rs:70-78) can surface a concurrent duplicate as 500.
4. **`runtime` of 0 is hidden** by a truthiness check (MovieDetail.tsx:93).
5. **Cast card key collides** on identical name/character pairs (MovieDetail.tsx:145).
6. **Impossible-state text reaches clients** — `AppError::Config("movie vanished after insert")` (movies.rs:82).
7. **Routing and client tests skip movies** — `App.test.tsx` mocks 7 of 9 routes; `client.test.ts` exercises 10 of 16 methods, none of them movie or watch-log.

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` decisions (live credits fetch, cast cap, newest-first order, local removal, post-mutation navigation).

### Should Fix
2. Move `friendlyError` into the shared fetch wrapper (cascade to `app`).
3. Add routing and client tests for the movie endpoints.

### Nice to Have
5. Cache credits and providers at add time, or add movies to resync.
6. Collapse the two near-identical exit handlers.
