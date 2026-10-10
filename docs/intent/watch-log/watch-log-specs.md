# Watch log — EARS specs

Prefix `WATCHLOG`. Facets: `DATA` (table and insert contract), `API` (`GET /api/v1/watch-log`), `UI` (History page and its sentences). The obligation to *call* the insert belongs to the writer segments (`SHOWS-WATCHED-003`, `SHOWS-WATCHED-007`, `MOVIES-API-009`).

## Data

- [x] **WATCHLOG-DATA-001**: The system shall store each watch-log row with `occurred_at` (UTC RFC3339), `media_type` (`tv` or `movie`), `action` (`watched` or `unwatched`), `scope` (`episode`, `season`, `show`, `through_episode`, or `movie`), `tmdb_id`, a `title` snapshot, an optional `poster_path` snapshot, optional `season_number`, `episode_number`, and `episode_name`, and `episode_count` defaulting to 1, with no foreign keys.
- [x] **WATCHLOG-DATA-002**: The watch-log insert shall require the caller's open transaction and shall stamp `occurred_at` with the current UTC time at insert, so that a row is committed only together with the state change it describes.
- [x] **WATCHLOG-DATA-003**: Watch-log rows shall survive the removal of the show or movie they describe, including a movie deleted by being marked watched.
- [x] **WATCHLOG-DATA-004**: The system shall provide no path that updates or deletes a watch-log row.
- [x] **WATCHLOG-DATA-005**: The system shall write watch-log rows only for watched and unwatched state changes; adding a show or movie, removing a show, removing a movie, and resync shall write none.

## API

- [x] **WATCHLOG-API-001**: `GET /api/v1/watch-log` shall respond with `{ entries, page, per_page, total }`, with entries ordered by `occurred_at` descending and then `id` descending.
- [x] **WATCHLOG-API-002**: `GET /api/v1/watch-log` shall default `page` to 1 and `per_page` to 50; if `page` is less than 1 it shall respond 400 `page must be >= 1`, and if `per_page` is less than 1 it shall respond 400 `per_page must be >= 1`.
- [x] **WATCHLOG-API-003**: When `per_page` exceeds 100, `GET /api/v1/watch-log` shall clamp it to 100 and echo the effective value in the response.
- [x] **WATCHLOG-API-004**: When `page` is beyond the last page, `GET /api/v1/watch-log` shall respond 200 with empty `entries` and the unchanged `total`.
- [x] **WATCHLOG-API-005**: Each watch-log entry shall carry `poster_url` built from the snapshotted `poster_path` on the w185 TMDB image base, or null when no poster was snapshotted.

## UI

- [x] **WATCHLOG-UI-001**: The History page shall read its page number from the `?page=` search parameter, treating any value that is not an integer of at least 1 as page 1, and shall request 50 entries per page.
- [x] **WATCHLOG-UI-002**: When the user pages with Previous or Next, the History page shall write the new page number to `?page=`, omitting the parameter for page 1.
- [x] **WATCHLOG-UI-003**: The History page shall show "Loading…" until the first page arrives, `Error: <message>` if that load fails ("Load failed" when the rejection carries no message), and "Nothing logged yet" when `total` is 0.
- [x] **WATCHLOG-UI-004**: The History page shall render each entry with its poster (or "No poster"), its title as a link to `/shows/{tmdb_id}` when `media_type` is `tv` and as plain text when it is `movie`, a one-sentence description, and a `<time>` element whose `dateTime` is `occurred_at` and whose text is that instant in the browser's locale.
- [x] **WATCHLOG-UI-005**: The History page shall mark rows whose `action` is `unwatched` with the `history-row-unwatched` class so they render muted.
- [x] **WATCHLOG-UI-006**: The History page shall describe an `episode` entry as `Marked SxxExx "<episode name>" <verb>` (omitting the quoted name when null and never appending a count), a `season` entry as `Marked Season <n> <verb> · <count> episode(s)`, a `show` entry as `Marked all episodes <verb> · <count> episode(s)`, a `through_episode` entry as `Marked through SxxExx <verb> · <count> episode(s)`, and a `movie` entry as `Marked <verb>`, where `<verb>` is `watched` or `unwatched` per `action`.
- [x] **WATCHLOG-UI-007**: The History page shall show `Page <page> of <total pages> · <total> entry/entries`, computing total pages from the server's echoed `per_page` with a minimum of 1, and shall disable Previous on page 1 and Next on the last page.
- [x] **WATCHLOG-UI-008**: The History page shall offer no undo control and shall direct the user to the show or movie page for corrections.
