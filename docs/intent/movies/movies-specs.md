# Movies — EARS specs

Prefix `MOVIES`. Facets: `API` (endpoints and persistence) and `UI` (Movies and MovieDetail pages). "On the list" means a row exists in `movies`.

## API

- [x] **MOVIES-API-001**: When `POST /api/v1/movies` is called with a `tmdb_id` not on the list, the system shall fetch the movie from TMDB, insert a row using TMDB `title` as `name`, storing empty strings as NULL and `added_at` as the current UTC time, and respond 201 with the movie item.
- [x] **MOVIES-API-002**: If `POST /api/v1/movies` names a `tmdb_id` already on the list, then the system shall respond 400 with the error `movie <id> is already on the watchlist` and make no TMDB request.
- [x] **MOVIES-API-003**: If TMDB reports that the requested movie does not exist, then `POST /api/v1/movies` shall respond 404 and persist nothing.
- [x] **MOVIES-API-004**: The system shall persist only `tmdb_id`, `name`, `overview`, `poster_path`, `backdrop_path`, `release_date`, `runtime`, and `added_at` for a movie; cast, directors, and watch providers are never stored.
- [x] **MOVIES-API-005**: `GET /api/v1/movies` shall return at most 500 movies ordered by `added_at` descending, then by name case-insensitively.
- [x] **MOVIES-API-006**: When `GET /api/v1/movies/{id}` is called for a movie on the list, the system shall fetch the movie's credits and providers live from TMDB and respond with the stored fields plus `watch_providers` (US flatrate, free, and ads providers), `directors` (crew members whose job is `Director`), and `cast` sorted by TMDB order with missing order last, capped at 12 entries, with empty `character` as null and `profile_url` on the w185 base.
- [x] **MOVIES-API-007**: If `GET /api/v1/movies/{id}` names a movie not on the list, then the system shall respond 404 with the error `movie <id> not on watchlist` without contacting TMDB.
- [x] **MOVIES-API-008**: If TMDB responds 429 while fetching movie detail, then the system shall respond 502 with the error `TMDB is rate-limiting requests right now. Please try again in a moment.` (the mapping is performed by the TMDB client).
- [x] **MOVIES-API-009**: When `POST /api/v1/movies/{id}/watched` is called for a movie on the list, the system shall, in one transaction, delete the row and write exactly one watch-log row with media type `movie`, action `watched`, scope `movie`, and the movie's name and poster snapshotted, then respond 204; a second call for the same id shall respond 404 and write nothing.
- [x] **MOVIES-API-010**: When `DELETE /api/v1/movies/{id}` is called for a movie on the list, the system shall delete the row, write no watch-log row, and respond 204; if the movie is not on the list it shall respond 404.
- [x] **MOVIES-API-011**: The system shall not include movies in resync, in the calendar, or in Up Next.

## UI

- [x] **MOVIES-UI-001**: The Movies page shall render each movie as a card whose poster (or "No poster" placeholder) links to `/movies/{tmdb_id}` with the accessible label `View details for <name>`, showing the name followed by `(<year>)` from the first four characters of `release_date` when present, and **Mark Watched** and **Remove** buttons.
- [x] **MOVIES-UI-002**: The Movies page shall show "Loading…" until the list arrives, `Error: <message>` if loading fails ("Load failed" when the rejection carries no message), and, when the list is empty, "No movies yet" with a link to Search.
- [x] **MOVIES-UI-003**: When the user clicks Mark Watched or Remove on the Movies page, the page shall ask for confirmation naming the movie; if cancelled it shall make no request, and if confirmed it shall call the matching endpoint and on success remove that card from the list without refetching.
- [x] **MOVIES-UI-004**: While a Mark Watched or Remove request is pending for a movie, the Movies page shall disable both buttons on that card only.
- [x] **MOVIES-UI-005**: When the movie-detail route id is not a finite number, the MovieDetail page shall show `Error: Invalid movie id` without requesting the movie.
- [x] **MOVIES-UI-006**: The MovieDetail page shall show the backdrop when present, the poster (or placeholder), name, release year when present, `<runtime> min` when runtime is present and non-zero, `Director:` or `Directors:` followed by the comma-joined names when any exist, `Watch on:` followed by the comma-joined providers when any exist, and the overview when present.
- [x] **MOVIES-UI-007**: The MovieDetail page shall render a cast grid with each member's photo (or "No photo"), name, and `as <character>` when a character is present, or the text "No cast information available." when the cast is empty.
- [x] **MOVIES-UI-008**: When the user clicks Mark Watched or Remove on the MovieDetail page, the page shall ask for confirmation, and if confirmed call the matching endpoint and navigate to `/movies` on success; if the request fails it shall show `Error: <message>` above the header and keep the page visible.
- [x] **MOVIES-UI-009**: The MovieDetail page shall display errors after stripping any leading `API <status>:` prefix, replacing a rate-limit message with "We couldn't load this movie's details right now — TMDB is rate-limiting requests. Please try again in a moment." and a `TMDB returned 5xx` or upstream message with "We couldn't load this movie's details from TMDB right now. Please try again shortly."
- [ ] **MOVIES-UI-010**: If a Mark Watched or Remove request fails on the Movies page, then the page shall show `Error: <message>` as a banner above the list, keep the list visible, and re-enable that card's buttons.
