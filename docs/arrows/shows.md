# Arrow: shows

Tracking a TV show: add with its full season/episode tree, list with progress, detail, remove, and every watched-state mutation, plus the definition of "today" and "aired" that other segments cite.

## Status

**AUDITED** — last audited 2026-10-10 (git SHA `4da220e`). All 40 specs are implemented, annotated in code, and cited by at least one test; what remains open is the LLD's deferred list (per-season count semantics, query fan-out, duplicate-add race, add-under-timeout, and the untested timezone boundary) and the six `[inferred]` decision rows awaiting confirmation.

## References

### HLD
- docs/high-level-design.md (System Design → shows)

### LLD
- docs/intent/shows/shows-design.md

### EARS
- docs/intent/shows/shows-specs.md (40 specs: 40 implemented, 0 deferred, 0 active gaps)

### Tests
- backend/tests/api.rs — `list_shows_returns_watchlist`, `add_show_fetches_from_tmdb_and_inserts`, `add_show_persists_nothing_when_a_season_fetch_fails`, `add_show_rejects_duplicate`, `add_show_returns_404_when_tmdb_missing`, `get_show_returns_detail`, `get_show_returns_404_when_unknown`, `delete_show_removes_and_returns_204`, `delete_show_returns_404_for_unknown`, `bulk_watch_marks_all_aired_episodes`, `bulk_watch_supports_season_and_through_episode`, `bulk_watch_returns_404_when_show_unknown`, `patch_episode_toggles_and_returns_show_detail`, `patch_episode_same_state_returns_200_detail_without_logging`, `patch_episode_404_when_episode_missing`, `get_show_detail_response_includes_show_level_counts`
- backend/tests/db.rs — `insert_show_full_persists_show_seasons_and_episodes`, `show_exists_and_delete_show`, `delete_show_cascades_to_seasons_and_episodes`, `list_watchlist_includes_progress_and_next_air_date`, `get_show_detail_handles_invalid_providers_json`, `list_up_next_includes_networks_and_resync_backfills_them`, `set_episode_watched_toggles_state`, `set_episode_watched_returns_false_when_episode_missing`, `bulk_set_watched_all_filters_to_aired_only`, `bulk_set_watched_season_scope`, `bulk_set_watched_through_episode_inclusive_and_aired_only`, `bulk_set_unwatched_clears_state`, `list_watchlist_is_capped`, `set_episode_watched_logs_watch_and_unwatch`, `set_episode_watched_same_state_preserves_watched_at_and_logs_nothing`, `set_episode_watched_false_on_unwatched_episode_logs_nothing`, `set_episode_watched_missing_episode_logs_nothing`, `bulk_set_watched_logs_one_entry_with_changed_count`, `bulk_set_watched_preserves_watched_at_on_already_watched`, `bulk_scopes_map_to_log_scopes`, `watch_log_survives_show_removal`, `get_show_detail_carries_aired_based_show_counts`
- backend/src/state.rs inline tests — `today_in_returns_iso_date`, `today_in_can_differ_across_timezones`
- frontend/src/pages/Watchlist.test.tsx (4 tests)
- frontend/src/pages/ShowDetail.test.tsx (26 tests)
- frontend/src/api/client.test.ts — `setEpisodeWatched PATCHes with body`, `bulkWatch POSTs scope + watched`

### Code
- backend/src/api/shows.rs — `list_shows`, `add_show`, `get_show`, `delete_show`, `BulkWatchScopeBody`, `bulk_watch`
- backend/src/api/episodes.rs — `patch_episode`
- backend/src/db/queries.rs — `MAX_LIST_ROWS`, `insert_show_full`, `delete_show`, `list_watchlist`, `get_show_detail`, `episode_counts`, `next_unaired_air_date`, `set_episode_watched`, `bulk_set_watched` (the unannotated `show_exists` and `get_watchlist_item` are reached through `add_show`)
- backend/src/state.rs — `today_in`
- backend/src/models/show.rs — `ShowRow`, `WatchlistItem`, `ShowDetail`, `SeasonDetail`, `EpisodeDetail` (wire shapes; the file's own annotations belong to `tmdb`)
- backend/src/db/migrations/20260509000000_initial.sql, 20260808000000_drop_notifications.sql, 20260921000000_add_show_networks.sql (never annotated; the owning module is queries.rs)
- frontend/src/pages/Watchlist.tsx — page component; frontend/src/pages/ShowDetail.tsx — page component
- frontend/src/api/client.ts — `BulkWatchScope` type (the `listShows`, `addShow`, `getShow`, `deleteShow`, `setEpisodeWatched`, `bulkWatch` properties of the `api` literal cannot carry annotations)
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
| UI | SHOWS-UI-001 to 015 | 15 | 0 | 0 |

**Summary:** 40 of 40 active specs implemented; 0 deferred. (Specs with no test citation: none.)

## Key Findings

1. **Per-season `watched_count` on detail ignores air date** (queries.rs:`get_show_detail`), unlike the show-level aired-only count from `episode_counts`. Now pinned as intended by SHOWS-API-009; still listed as LLD Deferred 1.
2. **Query fan-out** — `list_watchlist` issues 1 + 2N statements, `get_show_detail` 3 + S; bounded only by `MAX_LIST_ROWS`. LLD Deferred 2.
3. **Duplicate-add race** — check-then-insert (shows.rs:`add_show` → `show_exists`, then queries.rs:`insert_show_full`) turns a concurrent duplicate into a 500 instead of a 400. LLD Deferred 3.
4. **Add runs N+1 serial TMDB calls under the 30 s request timeout** (shows.rs:`add_show`; lib.rs:`API_REQUEST_TIMEOUT`); the response is cut off while fetches continue. LLD Deferred 4.
5. **`seasons.episode_count` is the number of fetched episodes** (queries.rs:`insert_show_full`), not TMDB's summary count. Pinned as intended by SHOWS-API-007; still LLD Deferred 5.
6. **`networks_json` is invisible to this segment's wire shapes** — absent from `ShowRow` and `ShowDetail` (models/show.rs); only Up Next reads it. LLD Deferred 6.
7. **Impossible-state error text reaches clients** — `AppError::Config("show vanished after insert")` (shows.rs:`add_show`) passes through `client_message()` verbatim as a 500 body. LLD Deferred 7.
8. **One `mutating` flag locks the whole detail page** during any single toggle (ShowDetail.tsx:`mutating`). LLD Deferred 8.
9. **Timezone boundary untested** — every integration test uses UTC (backend/tests/common/mod.rs:`ny_tz` is never called); the `state.rs` unit tests only check `today_in`'s shape, not a midnight crossing in queries. LLD Deferred 9.
10. **Stale doc comment** on queries.rs:`insert_show_full` names a `season_episodes` parameter that does not exist. LLD Deferred 10.

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` rows in the LLD decisions table (season 0 rationale, mutation response shape, TEXT dates, JSON columns, read-only Watchlist, post-remove navigation).

### Should Fix
2. Add a timezone-boundary test for `SHOWS-PROGRESS-001` and `SHOWS-WATCHED-004` using `ny_tz()`.
3. Move LLD Deferred 1 and 5 to Resolved: SHOWS-API-009 and SHOWS-API-007 now state the per-season count and `episode_count` semantics as intent.

### Nice to Have
4. Collapse the per-show count queries into a single grouped query.
5. Replace the check-then-insert on add with `INSERT … ON CONFLICT` or map the constraint error to 400.
6. Fix the stale `insert_show_full` doc comment.
