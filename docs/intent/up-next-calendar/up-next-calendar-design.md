---
parent: high-level-design
prefix: AIRING
---

# Up Next and Calendar

## Context and Design Philosophy

These are the two views that answer "what has aired?" from the episode tree that `shows` maintains. **Up Next** is the home page: one row per show, the oldest unwatched episode that has already aired, sorted so the show you are furthest behind on is at the top, with a one-click mark-watched that advances the row. **Calendar** is a month grid of air dates for every tracked show, watched episodes faded, each linking back to its show.

Both derive "aired" from the definition `shows` owns: `air_date` is non-null and not after today in the configured `TIMEZONE`. Neither owns any persistent state; Up Next's mark-watched delegates to the single-episode PATCH in `shows`. Movies never appear in either view.

## Up Next

### Query

`list_up_next` (backend/src/db/queries.rs:599-680) is one SQL statement:

1. A CTE over `episodes` filtered to `watched = 0 AND air_date IS NOT NULL AND air_date <= today`, numbering rows per show with `ROW_NUMBER() OVER (PARTITION BY show ORDER BY air_date, season_number, episode_number)`.
2. The outer select keeps `rn = 1`, joins `shows` for name, poster, and `networks_json`, and computes `remaining` as a correlated count of the same filter for that show.
3. Ordered by the chosen episode's `air_date` ascending, then show name case-insensitively; bounded by `MAX_LIST_ROWS` (500).

`networks_json` is parsed into a list; NULL or unparseable JSON yields an empty list (:667-671). The response item (`UpNextItem`, backend/src/models/show.rs:117) carries show id, name, poster, networks, season and episode numbers, episode name and overview, `air_date`, and `remaining`. The handler (backend/src/api/up_next.rs) adds nothing.

### Page

`UpNext` (frontend/src/pages/UpNext.tsx):

- States: a load failure before any data renders only `Error: <message>`; loading renders only "Loading…"; an empty list renders the heading and "You're all caught up" with a link to Search (:60-74).
- Summary line: `N show(s) with unwatched aired episodes. Sorted by oldest unwatched first.` (:79-82). Order is trusted from the server; the page does no sorting or filtering.
- Row: poster and title both link to `/shows/{id}`; one `network-pill` per network, omitted when empty; `SxxExx` plus episode name when present; overview when present; a `{remaining} remaining` pill; `aired <air_date>` (:85-146).
- Remaining pill: when `remaining > 1` it takes the `status-pill-accent` style and the title `N aired episodes not yet watched`; when exactly 1 it stays neutral with the singular title (:134-143).
- **Mark watched**: calls the single-episode PATCH with `watched: true`, shows "Marking…" and disables only that row's button, then re-fetches the whole list; a failure renders an error banner above the list while the list stays (:42-58, :83, :148-155).

The initial fetch and the post-mark `load()` are two separate implementations of the same call (:15-23 vs :25-40); only the effect carries the `cancelled` guard.

## Calendar

### Endpoint

`get_calendar` (backend/src/api/calendar.rs):

1. `start` and `end` are required and parsed as `%Y-%m-%d`; a parse failure is 400 `invalid start date: <raw>` / `invalid end date: <raw>` (:28-31).
2. `end < start` is 400 `end must be >= start` (:33-35); a span over `MAX_RANGE_DAYS` (92) is 400 `range N days exceeds max of 92` (:36-42).
3. The **parsed** dates are re-formatted before binding (:44-53). `air_date` is TEXT compared bytewise, so a raw string such as `+009999-10-01` (which chrono accepts) must not reach SQL in a form that sorts below every stored date.
4. `list_calendar_episodes` (queries.rs:682-737) returns every episode with a non-null `air_date` in `[start, end]`, joined to its show, ordered by air date, show name, season, episode. There is no row cap; the date window is the bound.

Each `CalendarEpisode` (models/show.rs:131) carries show id and name, poster, season and episode numbers, episode name, `air_date`, and `watched`.

### Page

`Calendar` (frontend/src/pages/Calendar.tsx):

- The visible range is always 42 cells: the Sunday on or before the 1st through six weeks later (:36-52). Weeks start on Sunday; month and weekday names are English literals (:6-20).
- "Today" is `new Date()` captured once at mount in the browser's local zone (:55, :64) and used for the initial month, the `calendar-cell-today` highlight, and the **Today** button. This is browser time, not the server's `TIMEZONE`.
- The fetch runs whenever the visible range changes, requesting the first through last cell (:66-86). Episodes are grouped by exact `air_date` string (:88-96).
- Prev and Next wrap across year boundaries (:98-114).
- Each episode renders as a link to `/shows/{id}` with the show name and `SxxExx`, a decorative poster (`alt=""`) when present, and the episode name as the link title; watched episodes add `calendar-ep-watched`, which the stylesheet fades (:166-191).
- Cells outside the month get `calendar-cell-other`. An error renders above the grid; "Loading…" renders below it; the grid itself always renders, so an empty month is simply empty (:140, :198).

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| What Up Next shows | Per show, the oldest unwatched aired episode; sorted oldest first | Newest episode; sorted by show name | Longest-overdue first makes falling behind visible (CLAUDE.md). |
| Where to watch | Network pills from `networks_json`; watch providers deliberately absent | Provider pills | TMDB's provider list is too noisy to read at a glance; networks say where the show airs (CLAUDE.md, add_show_networks.sql:1-4). |
| Remaining count | Aired unwatched only; accent style when more than one | Count all unwatched; always accent | Future episodes are not "behind"; one pending episode is normal, several is the signal (CLAUDE.md; frontend/src/index.css:244). |
| Up Next query shape | Single statement with a window function and correlated count | Per-show queries in Rust | `[inferred]` One round trip for the home page; bounded by the 500-row cap. |
| Mark watched from Up Next | Single-episode PATCH, then re-fetch the list | Bulk `through_episode`; patch the row locally | `[inferred]` The row is by construction aired, so the escape-hatch path is safe; re-fetching lets the server pick the next episode. |
| Calendar range cap | 92 days server-side | No cap; cap equal to the 42-cell grid | `[inferred]` Bounds the only list query without a row cap; leaves room for a quarter view. |
| Calendar binding | Bind re-formatted parsed dates | Bind the raw query strings | A signed or non-padded year passes the span check but mis-sorts against stored TEXT dates (calendar.rs:44-48). |
| Watched episodes on the calendar | Shown, faded | Hidden | They still aired; fading keeps the month honest (CLAUDE.md). |
| Grid shape | Fixed 42 cells, Sunday start | Variable rows; locale-aware start | `[inferred]` Stable layout across months; single-locale homelab app. |
| Calendar "today" | Browser-local date at mount | Server-provided today in `TIMEZONE` | `[inferred]` No API exposes the server's today; see open question 1. |
| Calendar links | To the show page, not an episode anchor | Deep link to the episode row | `[inferred]` No episode-level route exists. |

## Open Questions & Future Decisions

### Resolved
*(none yet)*

### Deferred
1. **Two definitions of today.** The calendar highlight uses browser-local time fixed at mount (Calendar.tsx:55, :64); aired, remaining, and Up Next ordering use the server's `TIMEZONE`. A page left open past midnight keeps the old highlight. Should the server expose its today, or is the drift acceptable?
2. **No row cap on the calendar query.** Every other list is bounded by `MAX_LIST_ROWS`; the calendar relies on the 92-day window alone (queries.rs:699-722).
3. **Unpinned malformed-date contract.** backend/tests/api.rs:816-864 accepts either 200-with-normalized-results or 400 for signed-year and non-zero-padded inputs. Which is intended?
4. **Duplicate fetch code on Up Next** (`load()` vs the effect) and a stale list on a failed post-mark re-fetch.
5. **Clipped cells.** `.calendar-cell` hides overflow (index.css:489); a day with many episodes silently drops chips.
6. **Network pill keys** are the network name (UpNext.tsx:116); duplicate names would collide.
7. **Locale.** English month and weekday names, Sunday start, raw `YYYY-MM-DD` dates in the UI.
8. **Fetch window.** The 42-cell grid requests up to six days before and twelve after the month; those episodes render in `calendar-cell-other` cells.
9. **Mark watched semantics.** Up Next's button marks only the shown episode; marking "through here" from Up Next is not offered.

## References

- backend/src/api/up_next.rs, backend/src/api/calendar.rs
- backend/src/db/queries.rs:599-737 (`list_up_next`, `list_calendar_episodes`)
- backend/src/models/show.rs:117-140 (`UpNextItem`, `CalendarEpisode`)
- frontend/src/pages/UpNext.tsx, frontend/src/pages/Calendar.tsx
- backend/tests/db.rs:286-386 (up next, calendar), :594-611 (cap); backend/tests/api.rs:465-556, :783-875
- frontend/src/pages/UpNext.test.tsx (13 tests), frontend/src/pages/Calendar.test.tsx (10 tests)
- Consumed: `shows` (`today_in`, the aired definition, `networks_json`, the single-episode PATCH), `app` (home route `/` → Up Next)
