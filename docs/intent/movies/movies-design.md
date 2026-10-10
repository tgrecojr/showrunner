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

Wire shapes (backend/src/models/movie.rs): `MovieWatchlistItem` (:25) mirrors the row with `poster_url` (w185) and `backdrop_url` (w780); `MovieDetail` (:44) adds `watch_providers`, `directors`, and `cast[]` of `CastMember { name, character, profile_url }`.

## Endpoints

| Route | Behavior | Source |
|---|---|---|
| `POST /api/v1/movies` | 400 `movie N is already on the watchlist` if present; else fetch from TMDB (404 passes through), insert with empty strings as NULL and `added_at` = now, respond 201 with the item | backend/src/api/movies.rs:66-84; backend/src/db/queries.rs:294-313 |
| `GET /api/v1/movies` | Up to 500 rows, newest `added_at` first, then name case-insensitively | queries.rs:315-340 |
| `GET /api/v1/movies/{id}` | 404 if not on the list, before any TMDB call; else fetch the movie with credits and providers live, then merge: stored fields + US providers + crew with job `Director` + cast sorted by TMDB `order` (missing → last), capped at 12, empty `character` → null, `profile_url` w185 | movies.rs:26-64 |
| `POST /api/v1/movies/{id}/watched` | In one transaction: snapshot name and poster, delete the row, insert one watch-log row (`movie` / `watched` / scope `movie`, `episode_count` 1); 204. Absent → 404, no row | movies.rs:89-101; queries.rs:374-410 |
| `DELETE /api/v1/movies/{id}` | Delete the row; 204. Absent → 404. Never logs | movies.rs:104-116; queries.rs:363-369 |

Add is check-then-insert with no transaction (movies.rs:70-78); a concurrent duplicate would surface the primary-key violation as a 500.

A TMDB 429 on detail is mapped by the TMDB client to `TMDB is rate-limiting requests right now. Please try again in a moment.` with HTTP 502 (backend/src/datasources/tmdb.rs:112-117). That mapping is owned by `tmdb`; movie detail is currently the only call that has it.

## Pages

**Movies** (frontend/src/pages/Movies.tsx): a card grid. The poster is a link to `/movies/{id}` labelled `View details for <name>`; the title is plain text. Each card has **Mark Watched** and **Remove**, both routed through one `removeMovie` helper (:28-49) that asks for confirmation, calls the matching endpoint, and on success filters the card out of local state without refetching. `pending[tmdb_id]` disables both buttons on that card. A failed call sets `error`, and because the error branch returns early (:51-58), the whole list is replaced by `Error: <message>`; the intended behavior is a banner with the list kept (`MOVIES-UI-010`). States: loading, error, and an empty state linking to Search.

**MovieDetail** (frontend/src/pages/MovieDetail.tsx): backdrop hero with poster, name, year, `N min` pill when runtime is truthy, `Director:`/`Directors:` by count, `Watch on:` providers, overview, then a cast grid with photo or "No photo" and `as <character>`; "No cast information available." when the cast is empty. A non-finite route id short-circuits to `Invalid movie id`. **Mark Watched** and **Remove** confirm, call the endpoint, and navigate to `/movies`; failures render a banner with the page intact.

Every error on this page passes through `friendlyError` (:6-15): it strips the `API <status>:` prefix the fetch wrapper adds, replaces rate-limit text with a fixed sentence, replaces `TMDB returned 5xx` / `Upstream` with another fixed sentence, and otherwise shows the stripped message. No other page has this mapping.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| Watched state | Row existence; marking watched deletes the row | A `watched` column with filtering | A to-watch list only needs to know what is left (migration header, add_movies.sql:1-2). |
| Watched vs remove | Two routes; only `watched` logs | One delete with a `log` flag | The log should contain only things actually watched (movies.rs:86-88). |
| Credits and providers | Fetched live on every detail view, never stored | Persist at add; refresh on resync | `[inferred]` Keeps the table to eight columns and the data current without adding movies to resync; the cost is a TMDB dependency for every detail view. |
| Cast size | Top 12 by TMDB `order` | Full cast | `[inferred]` A detail page, not a credits database. |
| Episode-driven views | Movies excluded from resync, calendar, Up Next | Show release dates on the calendar | Those views are driven by episode air dates (CLAUDE.md). |
| List order | Newest added first, then name | Alphabetical like shows | `[inferred]` A queue reads newest-first. |
| After mutation on the list page | Filter locally, no refetch | Refetch the list | `[inferred]` One fewer request; the server response is 204 anyway. |
| After mutation on the detail page | Navigate to `/movies` | Stay with a confirmation | `[inferred]` The movie no longer exists on the list. |
| Error wording | `friendlyError` on the detail page | Raw `API <status>: …` | A TMDB outage or rate limit should read as a transient condition, not an API dump; app-wide intent, implemented here first. |

## Open Questions & Future Decisions

### Resolved
1. ✅ **A failed mark or remove keeps the list.** The Movies page shows the error as a banner and leaves the grid in place with that card re-enabled. The page currently drops the grid (Movies.tsx:51-58); tracked as `MOVIES-UI-010`.

### Deferred
1. **Live TMDB on every detail view.** A TMDB outage makes a listed movie's page a 502 even though the row exists. Should credits and providers be cached at add time, with movies joining resync?
2. **Stale metadata.** Because movies are outside resync, poster, overview, runtime, and release date are frozen at add time.
3. **`friendlyError` placement.** The mapping is app-wide intent but lives only on this page; moving it into the fetch wrapper is `app`'s cascade.
4. **`runtime` of 0** is hidden by the truthiness check (MovieDetail.tsx:93).
5. **Cast card key** is `name-character`, which collides for duplicate pairs (MovieDetail.tsx:145).
6. **Impossible-state error text** `movie vanished after insert` reaches the client as a 500 body (movies.rs:82).
7. **Near-duplicate handlers.** `mark_movie_watched` and `delete_movie` differ only in the query called (movies.rs:89-116).
8. **`#[allow(dead_code)]` on `MovieRow`** (models/movie.rs:11) looks stale; every field is read.
9. **Test gaps.** `App.test.tsx` covers neither movie route; `client.test.ts` covers none of the six movie and watch-log client methods.
10. **Check-then-insert race** on add, as with shows.

## References

- backend/src/api/movies.rs; backend/src/models/movie.rs
- backend/src/db/queries.rs:284-425 (`movie_exists`, `insert_movie`, `list_movies`, `get_movie`, `delete_movie`, `mark_movie_watched`, `tracked_movie_tmdb_ids_in`)
- backend/src/db/migrations/20260513000000_add_movies.sql
- frontend/src/pages/Movies.tsx, frontend/src/pages/MovieDetail.tsx
- backend/tests/api.rs:595-782, :1272-1302; backend/tests/db.rs:577-590, :797-822; frontend/src/pages/Movies.test.tsx, MovieDetail.test.tsx
- Consumed: `tmdb` (`get_movie`, `us_providers`, `directors`, 429 mapping), `watch-log` (`insert_entry`)
- Consumers: `search` (`tracked_movie_tmdb_ids_in`, add endpoint), `watch-log` (movie rows in History are not linked because the row is gone)
