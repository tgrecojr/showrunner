# Arrow: watch-log

The append-only history of every watched and unwatched action, written inside the mutating transaction by `shows` and `movies`, read newest-first through a paginated endpoint, and shown on the read-only History page.

## Status

**AUDITED** — last audited 2026-10-10 (git SHA `4da220e`). All 19 specs are implemented, annotated in code, and cited by at least one test; what remains open is the LLD's own deferred items.

## References

### HLD
- docs/high-level-design.md (System Design → watch-log)

### LLD
- docs/intent/watch-log/watch-log-design.md

### EARS
- docs/intent/watch-log/watch-log-specs.md (19 specs: 19 implemented, 0 deferred, 0 active gaps)

### Tests
- backend/tests/db.rs — `set_episode_watched_logs_watch_and_unwatch`, `mark_movie_watched_deletes_and_logs_snapshot`, `delete_movie_does_not_log`, `watch_log_survives_show_removal`, `list_entries_paginates_newest_first`, `list_entries_clamps_per_page`
- backend/tests/api.rs — `mark_movie_watched_returns_204_then_404`, `delete_movie_writes_no_log_entry`, `watch_log_exposes_no_update_or_delete_route`, `watch_log_returns_page_shape`, `watch_log_rejects_bad_page_and_clamps_per_page`
- backend/src/models/watch_log.rs inline test — `action_str_maps_bool`
- frontend/src/pages/History.test.tsx (9 tests, including the `describeEntry` sentence matrix and the page-change error case)
- Not covered: frontend/src/api/client.test.ts has no `watchLog` URL-construction case

### Code
- backend/src/db/watch_log.rs — module header (table ownership, survival, no update/delete path), `insert_entry`, `list_entries`; `count_entries` is an unannotated helper
- backend/src/api/watch_log.rs — `list_watch_log` (with `WatchLogQuery`)
- backend/src/models/watch_log.rs — `NewWatchLogEntry`; `WatchLogRow`, `WatchLogEntry`, `WatchLogPage`, and `action_str` are unannotated shapes and helpers
- backend/src/db/migrations/20260911000000_add_watch_log.sql — the table; never annotated (sqlx checksums migrations), cited through the db module instead
- frontend/src/pages/History.tsx — `History`
- frontend/src/pages/watchLogText.ts — `describeEntry`
- frontend/src/api/client.ts — `api.watchLog` (property of the `api` literal, not annotated by convention)
- Consumed from other segments: `poster_url` (`shows`, backend/src/models/show.rs); `AppError::InvalidData` and the `/api/v1/watch-log` route registration in backend/src/lib.rs (`app`); the `/history` route in frontend/src/App.tsx (`app`).
- Consumers: `shows` calls `insert_entry` from `set_episode_watched` and `bulk_set_watched`, and `movies` from `mark_movie_watched` (all in backend/src/db/queries.rs); each writer's obligation to log is specified in its own segment (`SHOWS-WATCHED-003`, `SHOWS-WATCHED-007`, `MOVIES-API-009`).

## Architecture

**Purpose:** Keep a trustworthy record of what was marked watched or unwatched, independent of whether the show or movie still exists.

**Key Components:**
1. `watch_log` table — snapshot columns, no foreign keys, ordered index.
2. `insert_entry` — transaction-only insert that stamps the time.
3. `GET /watch-log` — validated, clamped offset pagination.
4. History page — URL-driven paging, per-scope sentences, read-only by design; a failed page change shows the error above the still-visible previous page.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Data | WATCHLOG-DATA-001 to 005 | 5 | 0 | 0 |
| API | WATCHLOG-API-001 to 005 | 5 | 0 | 0 |
| UI | WATCHLOG-UI-001 to 009 | 9 | 0 | 0 |

**Summary:** 19 of 19 active specs implemented; 0 deferred. (Specs with no test citation: none.)

## Key Findings

1. **Vocabulary is unenforced** — `media_type`, `action`, `scope` are free TEXT with no CHECK constraints (backend/src/db/watch_log.rs:`insert_entry` binds whatever the writer passes). LLD Deferred 1.
2. **Count and page are separate statements** — backend/src/db/watch_log.rs:`list_entries` runs `count_entries` then the page query, so `total` can drift under concurrent writes. LLD Deferred 2.
3. **`page` is unbounded and a page past the end renders `Page 7 of 3`** — backend/src/api/watch_log.rs:`list_watch_log` caps `per_page` but not `page`; frontend/src/pages/History.tsx:`History` computes `totalPages` from `total` and shows the requested page number regardless. LLD Deferred 3 and 4.
4. **Display time is browser-local** — frontend/src/pages/History.tsx:`formatTime` uses `toLocaleString` with no zone, while the rest of the app reasons in the server's `TIMEZONE`. LLD Deferred 5.
5. **Borrowed styling** — frontend/src/pages/History.tsx:`History` renders rows with `upnext-*` classes, and the `history-page` wrapper class has no rule in frontend/src/index.css (only `history-row-unwatched`, `history-time`, `history-sentence` do). LLD Deferred 6.
6. **Writer spec placement** — the obligation to log lives in `SHOWS-WATCHED-003`, `SHOWS-WATCHED-007`, and `MOVIES-API-009`; a change to `NewWatchLogEntry` (backend/src/models/watch_log.rs) cascades into both of those segments.
7. **Test hygiene** — frontend/src/pages/History.test.tsx has `beforeEach` reset only and no `afterEach(vi.restoreAllMocks)`, unlike sibling page tests. LLD Deferred 8.

## Work Required

### Must Fix

### Should Fix
1. Add a frontend/src/api/client.test.ts case for `api.watchLog` URL construction (`/watch-log?page=&per_page=`).

### Nice to Have
2. Clamp or redirect an out-of-range `?page=` (Finding 3).
3. Show a loading state on page change (LLD Deferred 7).
4. Add `afterEach(vi.restoreAllMocks)` to History.test.tsx (Finding 7).
