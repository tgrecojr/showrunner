# Shows — EARS specs

Prefix `SHOWS`. Facets: `API` (add, list, detail, delete endpoints and persistence), `PROGRESS` (the definition of today, aired, and progress counts), `WATCHED` (single and bulk watched-state mutations), `UI` (Watchlist and ShowDetail pages). "Aired" throughout means `air_date` is non-null and not after today in the configured `TIMEZONE`.

## API

- [x] **SHOWS-API-001**: When `POST /api/v1/shows` is called with a `tmdb_id` not on the watchlist, the system shall fetch the show from TMDB, then fetch each season with `season_number > 0` one at a time, then insert the show, its seasons, and its episodes in a single transaction, and respond 201 with the show's watchlist item.
- [x] **SHOWS-API-002**: If `POST /api/v1/shows` names a `tmdb_id` already on the watchlist, then the system shall respond 400 with the error `show <id> is already on the watchlist` and make no TMDB request.
- [x] **SHOWS-API-003**: If TMDB reports that the requested show does not exist, then `POST /api/v1/shows` shall respond 404 and persist nothing.
- [x] **SHOWS-API-004**: If any TMDB fetch fails while adding a show, then the system shall persist nothing for that show, because all fetches complete before the insert transaction begins.
- [x] **SHOWS-API-005**: The system shall never fetch or store season 0 (Specials) when adding a show.
- [x] **SHOWS-API-006**: When inserting a show, the system shall store its US watch providers and its network names as JSON arrays of names, store empty TMDB strings as NULL, and set `last_synced_at` to the current UTC time in RFC3339.
- [x] **SHOWS-API-007**: When inserting a season, the system shall set its stored `episode_count` to the number of episodes TMDB returned for that season.
- [x] **SHOWS-API-008**: `GET /api/v1/shows` shall return at most 500 shows ordered by name case-insensitively, each with `watched_count`, `aired_count`, `total_count`, `next_episode_air_date`, `status`, `in_production`, and `poster_url`.
- [x] **SHOWS-API-009**: `GET /api/v1/shows/{tmdb_id}` shall return the show with the show-level `watched_count`, `aired_count`, and `total_count` defined by SHOWS-PROGRESS-002, seasons ordered by season number and episodes ordered by episode number, each season carrying `episode_count` and a `watched_count` of all its watched episodes regardless of air date, and `watch_providers` parsed from the stored JSON (an empty list when the JSON is unparseable).
- [x] **SHOWS-API-010**: If `GET /api/v1/shows/{id}`, `DELETE /api/v1/shows/{id}`, or `POST /api/v1/shows/{id}/bulk-watch` names a show not on the watchlist, then the system shall respond 404 with the error `show <id> not on watchlist`.
- [x] **SHOWS-API-011**: When `DELETE /api/v1/shows/{id}` removes a show, the system shall delete its seasons and episodes by cascade, respond 204, and leave existing watch-log rows untouched.

## Progress

- [x] **SHOWS-PROGRESS-001**: The system shall define today as the current date in the configured `TIMEZONE`, formatted `YYYY-MM-DD`, and shall treat an episode as aired when its `air_date` is non-null and not after today.
- [x] **SHOWS-PROGRESS-002**: On the watchlist and on the show detail, the show-level `watched_count` shall count only episodes that are both watched and aired, `aired_count` shall count aired episodes, and `total_count` shall count every episode including unaired and undated ones.
- [x] **SHOWS-PROGRESS-003**: `next_episode_air_date` shall be the earliest `air_date` strictly after today, or null when the show has none.
- [x] **SHOWS-PROGRESS-004**: An episode whose `air_date` is NULL shall count toward `total_count` only and shall never be treated as aired or as the next air date.

## Watched

- [x] **SHOWS-WATCHED-001**: When `PATCH /api/v1/episodes/{show}/{season}/{episode}` is called with `{"watched": <bool>}` and the value differs from the episode's current watched flag, the system shall set the flag regardless of the episode's air date, set `watched_at` to the current UTC time when true and to NULL when false, and respond 200 with the full show detail.
- [x] **SHOWS-WATCHED-002**: If the episode named by `PATCH /api/v1/episodes/{show}/{season}/{episode}` does not exist, then the system shall respond 404 and write no watch-log row.
- [x] **SHOWS-WATCHED-003**: When a single-episode PATCH changes an episode's watched flag, the system shall write exactly one watch-log row (media type `tv`, scope `episode`, snapshotting the show name, poster, and episode name) in the same transaction as the update.
- [x] **SHOWS-WATCHED-004**: When `POST /api/v1/shows/{id}/bulk-watch` is called with scope `all`, `season`, or `through_episode` and a `watched` value, the system shall update only episodes within that scope that are aired and whose watched state differs from the requested value, then respond 200 with the full show detail.
- [x] **SHOWS-WATCHED-005**: The `through_episode` scope shall include every episode of earlier seasons and every episode of the named season with an episode number less than or equal to the named one.
- [x] **SHOWS-WATCHED-006**: When a bulk action targets an episode already in the requested state, the system shall leave that episode's `watched_at` unchanged.
- [x] **SHOWS-WATCHED-007**: When a bulk action changes at least one episode, the system shall write exactly one watch-log row (media type `tv`; scope `show`, `season`, or `through_episode` matching the request; `episode_count` equal to the number of episodes changed) in the same transaction; when it changes none, it shall write no row.
- [x] **SHOWS-WATCHED-008**: The bulk-watch request body shall be `{"scope": <scope>, "watched": <bool>}` where `<scope>` is `{"type":"all"}`, `{"type":"season","season_number":N}`, or `{"type":"through_episode","season_number":N,"episode_number":M}`.
- [x] **SHOWS-WATCHED-009**: When a bulk action is called with `watched: false`, the system shall clear both `watched` and `watched_at` on the aired episodes in scope.
- [x] **SHOWS-WATCHED-010**: If a single-episode PATCH requests the watched state the episode already has, then the system shall leave `watched_at` unchanged and write no watch-log row, and shall still respond 200 with the show detail.

## UI

- [x] **SHOWS-UI-001**: The Watchlist page shall render each show as a card linking to `/shows/{tmdb_id}`, with its poster (or a "No poster" placeholder), name, a progress chip reading `{watched_count}/{aired_count}`, a status pill when `status` is present, and `Next airs <date>` when `next_episode_air_date` is present.
- [x] **SHOWS-UI-002**: The Watchlist page shall show "Loading…" until the list arrives, `Error: <message>` if loading fails ("Load failed" when the rejection carries no message), and, when the list is empty, "Nothing tracked yet" with a link to Search.
- [x] **SHOWS-UI-003**: When the show-detail route id is not a finite number, the ShowDetail page shall show `Error: Invalid show id` without requesting the show.
- [x] **SHOWS-UI-004**: The ShowDetail header shall show the poster (or placeholder), name, a progress chip (its counts are specified by SHOWS-UI-013), the status pill when present, the first-air year followed by `–<last year>` only when the year of `last_air_date` differs from the year of `first_air_date`, `Watch on:` followed by the comma-joined providers when any exist, and the overview when present.
- [x] **SHOWS-UI-005**: The ShowDetail page shall render seasons collapsed by default, with a toggle button exposing `aria-expanded`, labelled by the season name or `Season N`, and showing `{watched_count}/{episode_count}`; when open, each episode row shall show `SxxExx` zero-padded, the episode name or `—`, and the air date when present.
- [x] **SHOWS-UI-006**: When the user clicks the header bulk button, the ShowDetail page shall send scope `all` with `watched: true` labelled "Mark all watched", or with `watched: false` labelled "Mark all unwatched" when the detail response's show-level `watched_count` equals its `aired_count` and `aired_count` is greater than 0 (unaired episodes do not count against it).
- [x] **SHOWS-UI-007**: When the user clicks a season's bulk button, the ShowDetail page shall send scope `season` for that season with `watched` set to the opposite of whether every episode in the season is watched; the button shall be disabled while the season's `episode_count` is 0.
- [x] **SHOWS-UI-008**: When the user changes an episode checkbox (accessible label `Mark S{season}E{episode} watched`), the ShowDetail page shall send the single-episode PATCH with the checkbox's new value.
- [x] **SHOWS-UI-009**: When the user clicks "Mark through here" on an episode, the ShowDetail page shall send scope `through_episode` for that episode with `watched: true`, never `false`.
- [x] **SHOWS-UI-010**: While any watched-state mutation is in flight, the ShowDetail page shall disable every episode checkbox and every bulk button, and on success shall replace its show data with the server's response.
- [x] **SHOWS-UI-011**: When the user clicks "Remove from watchlist", the ShowDetail page shall ask for confirmation, and on confirmation call the delete endpoint and navigate to `/`; if the user cancels, it shall make no request.
- [x] **SHOWS-UI-012**: If a mutation or removal fails, then the ShowDetail page shall show `Error: <message>` above the header ("Update failed" or "Delete failed" when the rejection carries no message) and keep the show visible.
- [x] **SHOWS-UI-013**: The ShowDetail header progress chip shall read `{watched_count}/{aired_count}` from the detail response's show-level counts, the same aired-only counts the Watchlist chip shows for that show.
- [x] **SHOWS-UI-014**: While the show detail is loading, the ShowDetail page shall render `Loading…`; if the initial show request rejects, it shall render `Error: <message>` (`Load failed` when the rejection carries no message) in place of the page.
- [x] **SHOWS-UI-015**: If a show has no seasons, the ShowDetail page shall render `No seasons available yet.` in place of the season list.
