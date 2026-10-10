# Arrow: up-next-calendar

The two aired-episode views: Up Next (per show, the oldest unwatched aired episode with a remaining count and one-click advance) and Calendar (a month grid of air dates with watched episodes faded).

## Status

**AUDITED** — last audited 2026-10-10 (git SHA `4da220e`). All 24 specs are implemented, annotated in code, and cited by tests; what remains open is the LLD's six `[inferred]` decisions and its nine Deferred items (chiefly whose "today" the calendar highlights and the malformed-date contract).

## References

### HLD
- docs/high-level-design.md (System Design → up-next-calendar)

### LLD
- docs/intent/up-next-calendar/up-next-calendar-design.md

### EARS
- docs/intent/up-next-calendar/up-next-calendar-specs.md (24 specs: 24 implemented, 0 deferred, 0 active gaps)

### Tests
- backend/tests/db.rs — `list_up_next_picks_oldest_unwatched_aired_per_show`, `list_up_next_includes_networks_and_resync_backfills_them`, `list_up_next_skips_shows_with_no_unwatched_aired_episodes`, `list_up_next_is_capped`, `list_calendar_episodes_filters_by_range`
- backend/tests/api.rs — `up_next_returns_oldest_unwatched_per_show`, `calendar_returns_episodes_in_range`, `calendar_rejects_invalid_dates`, `calendar_rejects_inverted_range`, `calendar_rejects_range_exceeding_92_days`, `calendar_signed_year_does_not_bypass_the_92_day_cap`, `calendar_non_zero_padded_date_is_normalized_before_it_reaches_sql`, `calendar_ordinary_range_still_returns_the_in_window_episode`
- frontend/src/pages/UpNext.test.tsx (13 tests)
- frontend/src/pages/Calendar.test.tsx (11 tests)
- frontend/src/api/client.test.ts — `calendar passes start/end as query params`

### Code
- backend/src/api/up_next.rs — `list_up_next` handler
- backend/src/api/calendar.rs — `get_calendar` (parse, inverted-range and 92-day checks, re-formatted binding)
- backend/src/db/queries.rs — `list_up_next`, `list_calendar_episodes`, `MAX_LIST_ROWS` (shared with `shows` and `movies`)
- frontend/src/pages/UpNext.tsx — `UpNext` component, `markWatched`
- frontend/src/pages/Calendar.tsx — `Calendar` component, `buildVisibleRange`
- Consumed from other segments: `shows` (`today_in`, the aired definition, `networks_json`, the single-episode PATCH `api.setEpisodeWatched`), `app` (home route `/` → Up Next, `/calendar` route, `api.upNext` / `api.calendar` in the fetch wrapper)
- Consumers: none (no other segment calls into this one)

## Architecture

**Purpose:** Turn the episode tree into two answers: what to watch now, and when things air.

**Key Components:**
1. Up Next query — one window-function statement selecting the oldest unwatched aired episode per show with a correlated `remaining` count, capped at `MAX_LIST_ROWS` (500); `networks_json` parsed into a list, empty when NULL or unparseable.
2. Up Next page — rows with network pills and an accent on the remaining pill when more than one episode waits; Mark watched delegates to the single-episode PATCH and re-fetches.
3. Calendar endpoint — validated, canonicalized `start`/`end` within 92 days; uncapped range query ordered by air date, show name, season, episode.
4. Calendar page — fixed 42-cell Sunday-start grid, browser-local today highlight captured at mount, episodes linking to their show, watched episodes faded.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Up Next | AIRING-UPNEXT-001 to 012 | 12 | 0 | 0 |
| Calendar | AIRING-CAL-001 to 012 | 12 | 0 | 0 |

**Summary:** 24 of 24 active specs implemented; 0 deferred. (Specs with no test citation: none.)

## Key Findings

1. **Two definitions of today** — `Calendar.tsx:Calendar` captures `new Date()` at mount (browser-local); aired, remaining, and ordering use the server's `TIMEZONE` via `today_in`. LLD Deferred 1; HLD Open Question 1.
2. **The calendar query is the only uncapped list** — `queries.rs:list_calendar_episodes` has no `LIMIT`; the 92-day window in `calendar.rs:get_calendar` is its sole bound. LLD Deferred 2.
3. **Malformed-date contract is unpinned** — `api.rs:calendar_signed_year_does_not_bypass_the_92_day_cap` and `api.rs:calendar_non_zero_padded_date_is_normalized_before_it_reaches_sql` branch on the status code, accepting either normalization (200) or rejection (400). LLD Deferred 3.
4. **Duplicate fetch implementations on Up Next** — `UpNext.tsx:load` and the mount effect in `UpNext.tsx:UpNext` make the same call; only the effect carries the `cancelled` guard, and a failed post-mark re-fetch shows an error over stale rows. LLD Deferred 4.
5. **Overflowing cells clip silently** — `.calendar-eps` (frontend/src/index.css) sets `overflow: hidden`, so a day with many episodes drops chips without an indicator. LLD Deferred 5.
6. **Network pill key is the name** — `UpNext.tsx:UpNext` keys each `network-pill` by the network string; duplicates would collide. LLD Deferred 6.
7. **English-only, Sunday-first, raw ISO dates** throughout both pages (`Calendar.tsx` `MONTHS`/`WEEKDAYS` literals, `aired <air_date>` in `UpNext.tsx`). LLD Deferred 7.

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` decisions in the LLD table (single-statement query, PATCH-then-refetch, 92-day cap, 42-cell grid, browser-local today, show-level links).
2. Resolve Finding 1 (whose today the calendar highlights) and Finding 3 (reject vs normalize malformed dates), then pin each with a spec and a test.

### Should Fix
3. Add a `MAX_LIST_ROWS`-style bound to `list_calendar_episodes` or record the 92-day window as the intended bound in the LLD.
4. Collapse the two Up Next fetch paths into one guarded loader.

### Nice to Have
5. Overflow indicator for busy calendar days.
6. Key network pills by index or de-duplicate names at the source.
