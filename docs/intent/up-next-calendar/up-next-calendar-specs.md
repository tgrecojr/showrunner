# Up Next and Calendar — EARS specs

Prefix `AIRING`. Facets: `UPNEXT` (the `GET /api/v1/up-next` query and the Up Next page) and `CAL` (the `GET /api/v1/calendar` endpoint and the Calendar page). "Aired" means `air_date` is non-null and not after today in the configured `TIMEZONE`, as defined by `SHOWS-PROGRESS-001`.

## Up Next

- [x] **AIRING-UPNEXT-001**: `GET /api/v1/up-next` shall return, for each show that has at least one unwatched aired episode, exactly one item: the unwatched aired episode with the earliest `air_date`, breaking ties by lowest season number then lowest episode number.
- [x] **AIRING-UPNEXT-002**: `GET /api/v1/up-next` shall omit shows that have no unwatched aired episodes.
- [x] **AIRING-UPNEXT-003**: `GET /api/v1/up-next` shall order items by the chosen episode's `air_date` ascending, then by show name case-insensitively, and return at most 500 items.
- [x] **AIRING-UPNEXT-004**: Each Up Next item's `remaining` shall equal the number of that show's unwatched aired episodes, excluding unaired and undated episodes.
- [x] **AIRING-UPNEXT-005**: Each Up Next item shall carry `networks` parsed from the show's stored `networks_json`, as an empty list when the column is NULL or unparseable.
- [x] **AIRING-UPNEXT-006**: The Up Next page shall render each item with its poster (or "No poster") and show name both linking to `/shows/{show_tmdb_id}`, one pill per network and none when the list is empty, `SxxExx` zero-padded, the episode name when present, the episode overview when present, a `{remaining} remaining` pill, and `aired <air_date>`.
- [x] **AIRING-UPNEXT-007**: While an item's `remaining` is greater than 1, the Up Next page shall style its remaining pill with the accent style and title it `<n> aired episodes not yet watched`; while `remaining` is 1 it shall use the neutral pill style and the singular title.
- [x] **AIRING-UPNEXT-008**: While the list is non-empty, the Up Next page shall show the summary `<n> show(s) with unwatched aired episodes. Sorted by oldest unwatched first.` with "show" singular for exactly one item, and shall apply no client-side sorting or filtering.
- [x] **AIRING-UPNEXT-009**: While the list is empty, the Up Next page shall show "You're all caught up" with a link to Search.
- [x] **AIRING-UPNEXT-010**: If the initial Up Next load fails, then the page shall show only `Error: <message>` ("Load failed" when the rejection carries no message).
- [x] **AIRING-UPNEXT-011**: When the user clicks Mark watched on an Up Next row, the page shall call the single-episode PATCH for that episode with `watched: true`, show "Marking…" and disable that row's button only, and on success re-fetch the whole list.
- [x] **AIRING-UPNEXT-012**: If a Mark watched request from Up Next fails, then the page shall show `Error: <message>` above the list ("Update failed" when the rejection carries no message) and keep the list visible.

## Calendar

- [x] **AIRING-CAL-001**: `GET /api/v1/calendar` shall require `start` and `end` query parameters in `YYYY-MM-DD` form, and if either fails to parse it shall respond 400 with `invalid start date: <value>` or `invalid end date: <value>`.
- [x] **AIRING-CAL-002**: If `end` is earlier than `start`, then `GET /api/v1/calendar` shall respond 400 with the error `end must be >= start`.
- [x] **AIRING-CAL-003**: If the span from `start` to `end` exceeds 92 days, then `GET /api/v1/calendar` shall respond 400 with the error `range <n> days exceeds max of 92`.
- [x] **AIRING-CAL-004**: `GET /api/v1/calendar` shall bind the parsed dates re-formatted as zero-padded `YYYY-MM-DD`, never the raw query strings, so that the SQL comparison against stored dates is bytewise-correct.
- [x] **AIRING-CAL-005**: `GET /api/v1/calendar` shall return every tracked show's episode whose `air_date` is non-null and within `start` through `end` inclusive, ordered by air date, then show name case-insensitively, then season and episode number, each with show id and name, poster, season and episode numbers, episode name, `air_date`, and `watched`, with no row cap.
- [x] **AIRING-CAL-006**: The Calendar page shall render a `<Month> <Year>` heading, Prev, Today, and Next buttons, a Sunday-first weekday header, and a fixed 42-cell grid starting on the Sunday on or before the first of the month.
- [x] **AIRING-CAL-007**: When the visible month changes, the Calendar page shall request `/api/v1/calendar` with `start` equal to the first visible cell's date and `end` equal to the last visible cell's date.
- [x] **AIRING-CAL-008**: When the user clicks Prev or Next, the Calendar page shall move one month, wrapping December to January of the next year and January to December of the previous year; when the user clicks Today it shall return to the month of the date captured when the page mounted.
- [x] **AIRING-CAL-009**: The Calendar page shall place each episode in the cell whose date equals its `air_date`, rendering it as a link to `/shows/{show_tmdb_id}` showing the show name and `SxxExx`, a decorative poster image when present, and the episode name as the link title.
- [x] **AIRING-CAL-010**: The Calendar page shall mark watched episodes with the `calendar-ep-watched` class so they render faded rather than hidden.
- [x] **AIRING-CAL-011**: The Calendar page shall mark cells outside the displayed month with `calendar-cell-other`, and shall mark with `calendar-cell-today` the cell matching the `today` carried by the most recent calendar response, falling back to the browser-local date only until the first response arrives.
- [x] **AIRING-CAL-012**: The Calendar page shall always render the grid; while a fetch is in flight it shall show "Loading…" below the grid, and if the fetch fails it shall show `Error: <message>` above the grid ("Load failed" when the rejection carries no message).
- [x] **AIRING-CAL-013**: `GET /api/v1/calendar` shall include `today`, the current date in the configured `TIMEZONE` formatted `YYYY-MM-DD`, in every successful response, so the page highlights the same day the aired rules use.
