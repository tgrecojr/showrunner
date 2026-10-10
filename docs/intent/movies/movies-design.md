---
parent: high-level-design
prefix: MOVIES
---

# Movies

## Context and Design Philosophy

Movies are a flat to-watch list, deliberately simpler than shows: a row in `movies` means "on the list and not yet watched", and there is no watched state to toggle. Marking a movie watched deletes the row and writes a watch-log entry; removing it deletes the row silently. Keeping those as two routes is what lets the History page show only things the user actually watched.

The row stores basic metadata captured at add time. Cast, directors, and watch providers are not persisted; the detail page fetches them live from TMDB on every view. Movies are excluded from resync, the calendar, and Up Next, all of which are episode-driven.

## Data model

`movies` (backend/src/db/migrations/20260513000000_add_movies.sql): `tmdb_id` (PK), `name`, `overview`, `poster_path`, `backdrop_path`, `release_date`, `runtime`, `added_at` (UTC RFC3339, set by the insert). No indexes beyond the primary key; no foreign keys.

Wire shapes (backend/src/models/movie.rs): `MovieWatchlistItem` mirrors the row with `poster_url` (w185) and `backdrop_url` (w780); `MovieDetail` adds `watch_providers`, `directors`, and `cast[]` of `CastMember { name, character, profile_url }`.

## Endpoints

| Route | Behavior | Source |
|---|---|---|
| `POST /api/v1/movies` | 400 `movie N is already on the watchlist` if present; else fetch from TMDB (404 passes through), insert with empty strings as NULL and `added_at` = now, respond 201 with the item | backend/src/api/movies.rs (`add_movie`); backend/src/db/queries.rs (`movie_exists`, `insert_movie`) |
| `GET /api/v1/movies` | Up to 500 rows, newest `added_at` first, then name case-insensitively | queries.rs (`list_movies`) |
| `GET /api/v1/movies/{id}` | 404 if not on the list, before any TMDB call; else fetch the movie with credits and providers live, then merge: stored fields + US providers + crew with job `Director` + cast sorted by TMDB `order` (missing → last), capped at 12, empty `character` → null, `profile_url` w185 | movies.rs (`get_movie_detail`) |
| `POST /api/v1/movies/{id}/watched` | In one transaction: snapshot name and poster, delete the row, insert one watch-log row (`movie` / `watched` / scope `movie`, `episode_count` 1); 204. Absent → 404, no row | movies.rs (`mark_movie_watched`); queries.rs (`mark_movie_watched`) |
| `DELETE /api/v1/movies/{id}` | Delete the row; 204. Absent → 404. Never logs | movies.rs (`delete_movie`); queries.rs (`delete_movie`) |

Add is check-then-insert with no transaction (movies.rs (`add_movie`)); a concurrent duplicate would surface the primary-key violation as a 500.

A TMDB 429 on detail is mapped by the TMDB client to `TMDB is rate-limiting requests right now. Please try again in a moment.` with HTTP 502 (backend/src/datasources/tmdb.rs (`map_status`)). That mapping is owned by `tmdb`; `map_status` is shared by every TMDB call and covers 429 and 5xx alike (TMDB-ERR-002/003).

## Pages

**Movies** (frontend/src/pages/Movies.tsx): a card grid. The poster is a link to `/movies/{id}` labelled `View details for <name>`; the title is plain text. Each card has **Mark Watched** and **Remove**, both routed through one `removeMovie` helper that asks for confirmation, calls the matching endpoint, and on success filters the card out of local state without refetching. `pending[tmdb_id]` disables both buttons on that card. A failed call sets `error`, which renders as a banner above the grid with the list intact and that card's buttons re-enabled; the banner clears when the next Mark Watched or Remove attempt begins. Only a failed initial load replaces the page with `Error: <message>`, because there is no list to keep. States: loading, load error, action-error banner over the grid, and an empty state linking to Search.

**MovieDetail** (frontend/src/pages/MovieDetail.tsx): backdrop hero with poster, name, year, `N min` pill when runtime is present and non-zero, `Director:`/`Directors:` by count, `Watch on:` providers, overview, then a cast grid with photo or "No photo" and `as <character>`; "No cast information available." when the cast is empty. A non-finite route id short-circuits to `Invalid movie id`. **Mark Watched** and **Remove** confirm, call the endpoint, and navigate to `/movies`; failures render a banner with the page intact.

Errors on this page are shown as the API client delivered them, like every other page; the wording of a TMDB rate limit or outage is the server's (`tmdb`).

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| Watched state | Row existence; marking watched deletes the row | A `watched` column with filtering | A to-watch list only needs to know what is left (migration header comment, add_movies.sql). |
| Watched vs remove | Two routes; only `watched` logs | One delete with a `log` flag | The log should contain only things actually watched (movies.rs, doc comment on `mark_movie_watched`). |
| Credits and providers | Fetched live on every detail view, never stored | Persist at add; refresh on resync | Keeps the table to eight columns and the data current without adding movies to resync; the cost is a TMDB dependency for every detail view. |
| Cast size | Top 12 by TMDB `order` | Full cast | A detail page, not a credits database. |
| Episode-driven views | Movies excluded from resync, calendar, Up Next | Show release dates on the calendar | Those views are driven by episode air dates (CLAUDE.md). |
| List order | Newest added first, then name | Alphabetical like shows | A queue reads newest-first. |
| After mutation on the list page | Filter locally, no refetch | Refetch the list | One fewer request; the server response is 204 anyway. |
| After mutation on the detail page | Navigate to `/movies` | Stay with a confirmation | The movie no longer exists on the list. |

## Open Questions & Future Decisions

### Resolved
*(none yet)*

### Deferred
1. **Live TMDB on every detail view.** A TMDB outage makes a listed movie's page a 502 even though the row exists. Should credits and providers be cached at add time, with movies joining resync?
2. **Stale metadata.** Because movies are outside resync, poster, overview, runtime, and release date are frozen at add time.
3. **Cast card key** is `name-character`, which collides for duplicate pairs (MovieDetail.tsx, the `cast-card` `key`).
4. **Impossible-state error text** `movie vanished after insert` reaches the client as a 500 body (movies.rs (`add_movie`)).
5. **Near-duplicate handlers.** `mark_movie_watched` and `delete_movie` differ only in the query called (movies.rs).
6. **`#[allow(dead_code)]` on `MovieRow`** (models/movie.rs) looks stale; every field is read.
7. **Test gaps.** `App.test.tsx` covers neither movie route; `client.test.ts` covers none of the six movie and watch-log client methods.
8. **Check-then-insert race** on add, as with shows.

## References

- backend/src/api/movies.rs; backend/src/models/movie.rs
- backend/src/db/queries.rs (`movie_exists`, `insert_movie`, `list_movies`, `get_movie`, `delete_movie`, `mark_movie_watched`, `tracked_movie_tmdb_ids_in`)
- backend/src/db/migrations/20260513000000_add_movies.sql
- frontend/src/pages/Movies.tsx, frontend/src/pages/MovieDetail.tsx
- backend/tests/api.rs (`movies_are_absent_from_up_next_calendar_and_resync`, the Movies block from `list_movies_returns_empty_initially` through `delete_movie_removes_and_returns_204`, `mark_movie_watched_returns_204_then_404`, `delete_movie_writes_no_log_entry`); backend/tests/db.rs (`insert_movie_stores_only_basic_metadata_and_nulls_empty_strings`, `list_movies_is_capped`, `mark_movie_watched_deletes_and_logs_snapshot`, `delete_movie_does_not_log`); frontend/src/pages/Movies.test.tsx, MovieDetail.test.tsx
- Consumed: `tmdb` (`get_movie`, `us_providers`, `directors`, `map_status` for 429/5xx), `watch-log` (`insert_entry`)
- Consumers: `search` (`tracked_movie_tmdb_ids_in`, add endpoint), `watch-log` (movie rows in History are not linked because the row is gone)
