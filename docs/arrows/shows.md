# Arrow: shows

Tracking a TV show: add with its full season/episode tree, list with progress, detail, remove, and every watched-state mutation, plus the definition of "today" and "aired" that other segments cite.

## Status

**MAPPED** — sampled 2026-10-10 (git SHA `3b525e1`), not yet audited. Skeleton LLD and EARS specs were reverse-engineered from code; design rationale carries `[inferred]` markers until confirmed.

## References

### HLD
- docs/high-level-design.md (System Design → shows)

### LLD
- docs/intent/shows/shows-design.md

### EARS
- docs/intent/shows/shows-specs.md (38 specs: 35 implemented, 3 active gaps)

### Tests
- backend/tests/api.rs — list/add/get/delete (:173-330), bulk-watch (:332-425), episode PATCH (:428-462)
- backend/tests/db.rs — insert/exists/delete (:24-95), watchlist/detail (:97-171), single and bulk watched (:390-537), list cap (:556-575, :615), watch-log side effects of show mutations (:656-793, :826-839)
- backend/src/state.rs unit tests (`today_in`), backend/src/models/show.rs unit tests (image URLs, request shape)
- frontend/src/pages/Watchlist.test.tsx (4 tests), frontend/src/pages/ShowDetail.test.tsx (22 tests)
- frontend/src/api/client.test.ts — `listShows`, `addShow`, `getShow`, `deleteShow`, `setEpisodeWatched`, `bulkWatch`

### Code
- backend/src/api/shows.rs, backend/src/api/episodes.rs
- backend/src/db/queries.rs — `insert_show_full` (:31), `show_exists` (:121), `delete_show` (:129), `list_watchlist` (:137), `get_watchlist_item` (:170), `get_show_detail` (:204), `episode_counts` (:451), `next_unaired_air_date` (:471), `set_episode_watched` (:744), `BulkScope` (:810), `bulk_set_watched` (:826)
- backend/src/models/show.rs; backend/src/state.rs (`today_in`, :60)
- backend/src/db/migrations/20260509000000_initial.sql, 20260808000000_drop_notifications.sql, 20260921000000_add_show_networks.sql
- frontend/src/pages/Watchlist.tsx, frontend/src/pages/ShowDetail.tsx; frontend/src/api/client.ts (:38-46, :59-75)
- Consumed from other segments: `tmdb` (`get_show`, `get_season`, `us_providers`, `network_names`), `watch-log` (`insert_entry`)

## Architecture

**Purpose:** Keep a local mirror of each tracked show's episodes and the user's watched state, and derive progress against today in the configured timezone.

**Key Components:**
1. Add path — serial TMDB fetch of show and every non-special season, then one insert transaction.
2. Read models — `WatchlistItem` (aired-based progress, next air date) and `ShowDetail` (full tree, show-level aired-based progress, per-season total-based counts, providers).
3. Watched mutations — unfiltered single-episode toggle and aired-only bulk scopes, both guarded by `watched != ?` so only real changes write a watch-log row, in the same transaction.
4. Pages — read-only Watchlist grid and the ShowDetail page that hosts every mutation.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| API | SHOWS-API-001 to 011 | 11 | 0 | 0 |
| Progress | SHOWS-PROGRESS-001 to 004 | 4 | 0 | 0 |
| Watched | SHOWS-WATCHED-001 to 010 | 10 | 0 | 0 |
| UI | SHOWS-UI-001 to 013 | 13 | 0 | 0 |

**Summary:** 38 of 38 active specs implemented; no gaps.

## Key Findings

1. **Per-season `watched_count` on detail ignores air date** (queries.rs:245), unlike the watchlist's aired-only count (:454).
2. **Query fan-out** — `list_watchlist` issues 1 + 2N statements (:152-166), `get_show_detail` 2 + S (:232-243); bounded only by the 500-row cap.
3. **Duplicate-add race** — check-then-insert (shows.rs:26, queries.rs:43) turns a concurrent duplicate into a 500 instead of a 400.
4. **Add runs N+1 serial TMDB calls under the 30 s request timeout** (shows.rs:43-46; lib.rs:35); the response can be cut off while fetches continue.
5. **`seasons.episode_count` is derived from fetched episodes** (queries.rs:81), not TMDB's summary count.
6. **`networks_json` is invisible to this segment's wire shapes** — absent from `ShowRow` (models/show.rs:24-37) and `ShowDetail`; only Up Next reads it (queries.rs:638).
7. **Impossible-state error text reaches clients** — `AppError::Config("show vanished after insert")` (shows.rs:52) is passed through `client_message()` as a 500 body.
8. **One `mutating` flag locks the whole detail page** during any single toggle (ShowDetail.tsx:146, :154, :199, :212, :244).
9. **Timezone boundary untested** — every backend test uses UTC (backend/tests/common/mod.rs:33 `ny_tz()` is never called).
10. **Stale doc comment** at queries.rs:30 (`season_episodes` parameter does not exist).

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` rows in the LLD decisions table (mutation response shape, TEXT dates, JSON columns, read-only Watchlist, post-remove navigation).

### Should Fix
2. Add a timezone-boundary test for `SHOWS-PROGRESS-001` and `SHOWS-WATCHED-004`.
3. Align per-season `watched_count` semantics with the watchlist, or document the difference as intended.

### Nice to Have
4. Collapse the per-show count queries into a single grouped query.
5. Replace the check-then-insert on add with `INSERT … ON CONFLICT` or map the constraint error to 400.
6. Fix the stale `insert_show_full` doc comment.
