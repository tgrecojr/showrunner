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

`list_up_next` (backend/src/db/queries.rs) is one SQL statement:

1. A CTE over `episodes` filtered to `watched = 0 AND air_date IS NOT NULL AND air_date <= today`, numbering rows per show with `ROW_NUMBER() OVER (PARTITION BY show ORDER BY air_date, season_number, episode_number)`.
2. The outer select keeps `rn = 1`, joins `shows` for name, poster, and `networks_json`, and computes `remaining` as a correlated count of the same filter for that show.
3. Ordered by the chosen episode's `air_date` ascending, then show name case-insensitively; bounded by `MAX_LIST_ROWS` (500).

`networks_json` is parsed into a list; NULL or unparseable JSON yields an empty list (the `networks` field in `list_up_next`'s row-to-`UpNextItem` map). The response item (`UpNextItem`, backend/src/models/show.rs) carries show id, name, poster, networks, season and episode numbers, episode name and overview, `air_date`, and `remaining`. The handler (backend/src/api/up_next.rs) adds nothing.

### Page

`UpNext` (frontend/src/pages/UpNext.tsx):

- States: a load failure before any data renders only `Error: <message>`; loading renders only "Loading…"; an empty list renders the heading and "You're all caught up" with a link to Search (the early returns at the top of `UpNext`'s render).
- Summary line: `N show(s) with unwatched aired episodes. Sorted by oldest unwatched first.` (the `status` paragraph under the `Up Next` heading). Order is trusted from the server; the page does no sorting or filtering.
- Row: poster and title both link to `/shows/{id}`; one `network-pill` per network, omitted when empty; `SxxExx` plus episode name when present; overview when present; a `{remaining} remaining` pill; `aired <air_date>` (the `upnext-row` list item).
- Remaining pill: when `remaining > 1` it takes the `status-pill-accent` style and the title `N aired episodes not yet watched`; when exactly 1 it stays neutral with the singular title (the `status-pill` span in the `upnext-row`).
- **Mark watched**: calls the single-episode PATCH with `watched: true`, shows "Marking…" and disables only that row's button, then re-fetches the whole list; a failure renders an error banner above the list while the list stays (`markWatched`, the `status-error` paragraph above `upnext-list`, and the `upnext-action` button).

The initial fetch and the post-mark `load()` are two separate implementations of the same call (`load()` vs the `useEffect` body in `UpNext`); only the effect carries the `cancelled` guard.

## Calendar

### Endpoint

`get_calendar` (backend/src/api/calendar.rs):

1. `start` and `end` are required and parsed as `%Y-%m-%d`; a parse failure is 400 `invalid start date: <raw>` / `invalid end date: <raw>` (the two `NaiveDate::parse_from_str` calls in `get_calendar`).
2. `end < start` is 400 `end must be >= start`; a span over `MAX_RANGE_DAYS` (92) is 400 `range N days exceeds max of 92` (the two guards in `get_calendar`).
3. The **parsed** dates are re-formatted before binding (the `list_calendar_episodes` call in `get_calendar`). `air_date` is TEXT compared bytewise, so a raw string such as `+009999-10-01` (which chrono accepts) must not reach SQL in a form that sorts below every stored date.
4. `list_calendar_episodes` (queries.rs) returns every episode with a non-null `air_date` in `[start, end]`, joined to its show, ordered by air date, show name, season, episode. There is no row cap; the date window is the bound.

Each `CalendarEpisode` (models/show.rs) carries show id and name, poster, season and episode numbers, episode name, `air_date`, and `watched`.

### Page

`Calendar` (frontend/src/pages/Calendar.tsx):

- The visible range is always 42 cells: the Sunday on or before the 1st through six weeks later (`buildVisibleRange`). Weeks start on Sunday; month and weekday names are English literals (`WEEKDAYS`, `MONTHS`).
- "Today" is `new Date()` captured once at mount in the browser's local zone (the `today` and `todayIso` memos in `Calendar`) and used for the initial month, the `calendar-cell-today` highlight, and the **Today** button. This is browser time, not the server's `TIMEZONE`.
- The fetch runs whenever the visible range changes, requesting the first through last cell (the `useEffect` keyed on `range.startIso`/`range.endIso`). Episodes are grouped by exact `air_date` string (`episodesByDate`).
- Prev and Next wrap across year boundaries (`goPrev`, `goNext`).
- Each episode renders as a link to `/shows/{id}` with the show name and `SxxExx`, a decorative poster (`alt=""`) when present, and the episode name as the link title; watched episodes add `calendar-ep-watched`, which the stylesheet fades (the `calendar-ep` list item; `.calendar-ep-watched .calendar-ep-link` in index.css).
- Cells outside the month get `calendar-cell-other`. An error renders above the grid; "Loading…" renders below it; the grid itself always renders, so an empty month is simply empty (the `status-error` paragraph above `calendar-grid` and the `Loading…` paragraph below it).

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| What Up Next shows | Per show, the oldest unwatched aired episode; sorted oldest first | Newest episode; sorted by show name | Longest-overdue first makes falling behind visible (CLAUDE.md). |
| Where to watch | Network pills from `networks_json`; watch providers deliberately absent | Provider pills | TMDB's provider list is too noisy to read at a glance; networks say where the show airs (CLAUDE.md, the header comment of `20260921000000_add_show_networks.sql`). |
| Remaining count | Aired unwatched only; accent style when more than one | Count all unwatched; always accent | Future episodes are not "behind"; one pending episode is normal, several is the signal (CLAUDE.md; `.status-pill-accent` in frontend/src/index.css). |
| Up Next query shape | Single statement with a window function and correlated count | Per-show queries in Rust | `[inferred]` One round trip for the home page; bounded by the 500-row cap. |
| Mark watched from Up Next | Single-episode PATCH, then re-fetch the list | Bulk `through_episode`; patch the row locally | `[inferred]` The row is by construction aired, so the escape-hatch path is safe; re-fetching lets the server pick the next episode. |
| Calendar range cap | 92 days server-side | No cap; cap equal to the 42-cell grid | `[inferred]` Bounds the only list query without a row cap; leaves room for a quarter view. |
| Calendar binding | Bind re-formatted parsed dates | Bind the raw query strings | A signed or non-padded year passes the span check but mis-sorts against stored TEXT dates (the binding comment in `get_calendar`, calendar.rs). |
| Watched episodes on the calendar | Shown, faded | Hidden | They still aired; fading keeps the month honest (CLAUDE.md). |
| Grid shape | Fixed 42 cells, Sunday start | Variable rows; locale-aware start | `[inferred]` Stable layout across months; single-locale homelab app. |
| Calendar "today" | Browser-local date at mount | Server-provided today in `TIMEZONE` | `[inferred]` No API exposes the server's today; see open question 1. |
| Calendar links | To the show page, not an episode anchor | Deep link to the episode row | `[inferred]` No episode-level route exists. |

## Open Questions & Future Decisions

### Resolved
*(none yet)*

### Deferred
1. **Two definitions of today.** The calendar highlight uses browser-local time fixed at mount (the `today` and `todayIso` memos in `Calendar`); aired, remaining, and Up Next ordering use the server's `TIMEZONE`. A page left open past midnight keeps the old highlight. Should the server expose its today, or is the drift acceptable?
2. **No row cap on the calendar query.** Every other list is bounded by `MAX_LIST_ROWS`; the calendar relies on the 92-day window alone (the SQL in `list_calendar_episodes`).
3. **Unpinned malformed-date contract.** `calendar_signed_year_does_not_bypass_the_92_day_cap` and `calendar_non_zero_padded_date_is_normalized_before_it_reaches_sql` (backend/tests/api.rs) accept either 200-with-normalized-results or 400 for signed-year and non-zero-padded inputs. Which is intended?
4. **Duplicate fetch code on Up Next** (`load()` vs the effect) and a stale list on a failed post-mark re-fetch.
5. **Clipped cells.** `.calendar-eps` (the episode list inside each cell) hides overflow (index.css); a day with many episodes silently drops chips.
6. **Network pill keys** are the network name (the `network-pill` span's `key` in `UpNext`); duplicate names would collide.
7. **Locale.** English month and weekday names, Sunday start, raw `YYYY-MM-DD` dates in the UI.
8. **Fetch window.** The 42-cell grid requests up to six days before and twelve after the month; those episodes render in `calendar-cell-other` cells.
9. **Mark watched semantics.** Up Next's button marks only the shown episode; marking "through here" from Up Next is not offered.

## References

- backend/src/api/up_next.rs, backend/src/api/calendar.rs
- backend/src/db/queries.rs (`list_up_next`, `list_calendar_episodes`)
- backend/src/models/show.rs (`UpNextItem`, `CalendarEpisode`)
- frontend/src/pages/UpNext.tsx, frontend/src/pages/Calendar.tsx
- backend/tests/db.rs (`list_up_next_picks_oldest_unwatched_aired_per_show`, `list_up_next_includes_networks_and_resync_backfills_them`, `list_up_next_skips_shows_with_no_unwatched_aired_episodes`, `list_calendar_episodes_filters_by_range`, `list_up_next_is_capped`); backend/tests/api.rs (`calendar_returns_episodes_in_range`, `calendar_rejects_invalid_dates`, `calendar_rejects_inverted_range`, `calendar_rejects_range_exceeding_92_days`, `up_next_returns_oldest_unwatched_per_show`, `calendar_signed_year_does_not_bypass_the_92_day_cap`, `calendar_non_zero_padded_date_is_normalized_before_it_reaches_sql`, `calendar_ordinary_range_still_returns_the_in_window_episode`)
- frontend/src/pages/UpNext.test.tsx (13 tests), frontend/src/pages/Calendar.test.tsx (11 tests)
- Consumed: `shows` (`today_in`, the aired definition, `networks_json`, the single-episode PATCH), `app` (home route `/` → Up Next)
