# Arrow: up-next-calendar

The two aired-episode views: Up Next (per show, the oldest unwatched aired episode with a remaining count and one-click advance) and Calendar (a month grid of air dates with watched episodes faded).

## Status

**MAPPED** — sampled 2026-10-10 (git SHA `3b525e1`), not yet audited. Skeleton LLD and EARS specs were reverse-engineered from code; design rationale carries `[inferred]` markers until confirmed.

## References

### HLD
- docs/high-level-design.md (System Design → up-next-calendar)

### LLD
- docs/intent/up-next-calendar/up-next-calendar-design.md

### EARS
- docs/intent/up-next-calendar/up-next-calendar-specs.md (24 specs: 24 implemented, 0 active gaps)

### Tests
- backend/tests/db.rs — up next (:286-363), calendar range (:366-386), up-next cap (:594)
- backend/tests/api.rs — calendar validation and range (:465-531), up next (:536-554), calendar date-normalization probes (:783-875)
- frontend/src/pages/UpNext.test.tsx (13 tests), frontend/src/pages/Calendar.test.tsx (10 tests)
- frontend/src/api/client.test.ts — `upNext` (:79), `calendar` (:130)

### Code
- backend/src/api/up_next.rs, backend/src/api/calendar.rs
- backend/src/db/queries.rs — `list_up_next` (:599), `list_calendar_episodes` (:682)
- backend/src/models/show.rs — `UpNextItem` (:117), `CalendarEpisode` (:131)
- frontend/src/pages/UpNext.tsx, frontend/src/pages/Calendar.tsx; frontend/src/api/client.ts (:39, :77)
- Consumed from other segments: `shows` (`today_in`, aired definition, `networks_json`, single-episode PATCH), `app` (home route)

## Architecture

**Purpose:** Turn the episode tree into two answers: what to watch now, and when things air.

**Key Components:**
1. Up Next query — one window-function statement selecting the oldest unwatched aired episode per show with a correlated `remaining` count, capped at 500.
2. Up Next page — rows with network pills and an accent on the remaining pill when more than one episode waits; Mark watched delegates to the single-episode PATCH and re-fetches.
3. Calendar endpoint — validated, canonicalized `start`/`end` within 92 days; uncapped range query.
4. Calendar page — fixed 42-cell Sunday-start grid, browser-local today highlight, episodes linking to their show.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Up Next | AIRING-UPNEXT-001 to 012 | 12 | 0 | 0 |
| Calendar | AIRING-CAL-001 to 012 | 12 | 0 | 0 |

**Summary:** 24 of 24 active specs implemented.

## Key Findings

1. **Two definitions of today** — the calendar highlight is browser-local and fixed at mount (frontend/src/pages/Calendar.tsx:55, :64); aired, remaining, and ordering use the server's `TIMEZONE` (`today_in`). They diverge across midnight and across zones.
2. **The calendar query is the only uncapped list** (backend/src/db/queries.rs:699-722); the 92-day window is its sole bound.
3. **Malformed-date contract is unpinned** — backend/tests/api.rs:816-864 branch on the status code, accepting either normalization or rejection for signed-year and non-padded inputs.
4. **Duplicate fetch implementations on Up Next** (UpNext.tsx:15-23 vs :25-40); a failed re-fetch after marking shows an error over stale rows.
5. **Overflowing cells clip silently** (`.calendar-cell` `overflow: hidden`, frontend/src/index.css:489).
6. **Network pill key is the name** (UpNext.tsx:116); duplicates would collide.
7. **English-only, Sunday-first, raw ISO dates** throughout both pages.

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` decisions (single-statement query, PATCH-then-refetch, 92-day cap, 42-cell grid, browser-local today, show-level links).
2. Resolve Finding 1 (whose today the calendar highlights) and Finding 3 (reject vs normalize malformed dates), then pin each with a spec and a test.

### Should Fix
3. Add a `MAX_LIST_ROWS`-style bound or document the window as the bound for the calendar query.
4. Collapse the two Up Next fetch paths into one guarded loader.

### Nice to Have
5. Overflow indicator for busy calendar days.
6. Key network pills by index or de-duplicate names at the source.
