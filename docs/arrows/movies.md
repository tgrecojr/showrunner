# Arrow: movies

The flat movie to-watch list: add, list, detail with live TMDB credits, mark watched (which deletes and logs), and remove (which deletes silently).

## Status

**AUDITED** — last audited 2026-10-10 (git SHA `4da220e`). All 20 active specs are implemented, annotated in code, and cited by at least one test; what remains open is the Deferred items listed in the LLD.

## References

### HLD
- docs/high-level-design.md (System Design → movies)

### LLD
- docs/intent/movies/movies-design.md

### EARS
- docs/intent/movies/movies-specs.md (20 specs: 20 implemented, 0 deferred, 0 active gaps)

### Tests
- backend/tests/api.rs — `movies_are_absent_from_up_next_calendar_and_resync`, `list_movies_returns_empty_initially`, `add_movie_fetches_from_tmdb_and_inserts`, `add_movie_rejects_duplicate`, `add_movie_returns_404_when_tmdb_missing`, `get_movie_detail_returns_cast_and_providers`, `get_movie_detail_returns_404_when_not_on_watchlist`, `get_movie_detail_maps_tmdb_429_to_friendly_message`, `delete_movie_removes_and_returns_204`, `mark_movie_watched_returns_204_then_404`, `delete_movie_writes_no_log_entry`, `watch_log_returns_page_shape`
- backend/tests/db.rs — `insert_movie_stores_only_basic_metadata_and_nulls_empty_strings`, `list_movies_is_capped`, `mark_movie_watched_deletes_and_logs_snapshot`, `delete_movie_does_not_log`
- frontend/src/pages/Movies.test.tsx (11 tests)
- frontend/src/pages/MovieDetail.test.tsx (10 tests)
- Not covered: `App.test.tsx` mocks no movie route; `client.test.ts` exercises none of the five movie client methods

### Code
- backend/src/api/movies.rs — `list_movies`, `get_movie_detail`, `add_movie`, `mark_movie_watched`, `delete_movie`
- backend/src/db/queries.rs — `MAX_LIST_ROWS` (shared cap), the `=== Movies ===` module anchor (movies absent from resync, calendar, Up Next), `insert_movie`, `list_movies`, `delete_movie`, `mark_movie_watched`; unannotated helpers `movie_exists`, `get_movie`, `tracked_movie_tmdb_ids_in`
- backend/src/models/movie.rs — `MovieRow`, `MovieWatchlistItem`, `MovieDetail`, `CastMember`, `AddMovieRequest` (unannotated wire shapes; `profile_url` is cited under `tmdb`)
- backend/src/db/migrations/20260513000000_add_movies.sql (never annotated; the table is cited on the queries module)
- frontend/src/pages/Movies.tsx — `Movies` page (`removeMovie` helper, per-card `pending`, action-error banner)
- frontend/src/pages/MovieDetail.tsx — `MovieDetail` page
- frontend/src/api/client.ts — `listMovies`, `addMovie`, `getMovie`, `deleteMovie`, `markMovieWatched` (object-literal properties, not annotated)
- Consumed from other segments: `tmdb` (`get_movie`, `us_providers`, `directors`, `map_status` 429/5xx mapping), `watch-log` (`insert_entry`)
- Consumers: `search` (`tracked_movie_tmdb_ids_in`, and the add endpoint from the Search page), `watch-log` (movie rows in History are not linked because the row is gone)

## Architecture

**Purpose:** Keep a simple queue of movies to watch, and distinguish "watched it" from "changed my mind" so History stays honest.

**Key Components:**
1. Eight-column `movies` row captured at add time.
2. Detail merge — stored row plus a live TMDB credits/providers fetch on every view.
3. Two exits — `watched` (delete + log, one transaction) and `DELETE` (delete only).
4. Pages — Movies grid with local removal and an action-error banner over the grid, MovieDetail showing server messages as delivered.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| API | MOVIES-API-001 to 011 | 11 | 0 | 0 |
| UI | MOVIES-UI-001 to 010 (009 retired, not in the spec file) | 9 | 0 | 0 |

**Summary:** 20 of 20 active specs implemented; 0 deferred. (Specs with no test citation: none.)

## Key Findings

1. **Detail depends on TMDB at view time** — `backend/src/api/movies.rs:get_movie_detail` calls `state.tmdb.get_movie` on every request, so a listed movie's page is a 502 during a TMDB outage, and stored metadata is never refreshed because movies are outside resync. LLD Deferred 1 and 2.
2. **Check-then-insert on add** — `backend/src/api/movies.rs:add_movie` runs `movie_exists` then `insert_movie` without a transaction; a concurrent duplicate surfaces the primary-key violation as 500. LLD Deferred 8.
3. **Cast card key collides** on identical name/character pairs — `frontend/src/pages/MovieDetail.tsx:MovieDetail` keys cast cards by `${name}-${character}`. LLD Deferred 3.
4. **Impossible-state text reaches clients** — `backend/src/api/movies.rs:add_movie` returns `AppError::Config("movie vanished after insert")`, and `error.rs:client_message` passes `Config` text through verbatim. LLD Deferred 4.
5. **Routing and client tests skip movies** — `frontend/src/App.test.tsx` mocks seven pages, neither `Movies` nor `MovieDetail`; `frontend/src/api/client.test.ts` exercises 10 of 16 client methods, none of the five movie methods. LLD Deferred 7.

## Work Required

### Must Fix

### Should Fix
1. Add routing (`App.test.tsx`) and client (`client.test.ts`) tests for the movie routes and endpoints.

### Nice to Have
2. Collapse the two near-identical exit handlers (`mark_movie_watched`, `delete_movie` in backend/src/api/movies.rs).
