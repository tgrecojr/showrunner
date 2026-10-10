# Arrow: resync

Keeping show metadata current from TMDB without touching watched state: the per-show upsert chain, the bounded serial full run, the cron trigger, the rate-gated manual trigger, and the Settings page.

## Status

**AUDITED** — last audited 2026-10-10 (git SHA `4da220e`). All 21 specs are implemented, annotated in code, and cited by at least one test; what remains open is the LLD's deferred list (overlapping runs, non-transactional per-show resync, stale rows, the 30 s manual timeout, no visibility of scheduled runs) and three `[inferred]` decisions awaiting confirmation.

## References

### HLD
- docs/high-level-design.md (System Design → resync)

### LLD
- docs/intent/resync/resync-design.md

### EARS
- docs/intent/resync/resync-specs.md (21 specs: 21 implemented, 0 deferred, 0 active gaps)

### Tests
- backend/tests/logic.rs — `resync_show_updates_metadata_and_episodes_preserving_watched`, `resync_show_skips_season_zero`, `resync_all_reports_per_show_success_and_failure`, `resync_all_with_no_shows_is_a_noop`, `resync_all_caps_shows_per_run`, `resync_all_below_the_ceiling_still_syncs_everything`, `resync_all_rotates_through_every_eligible_show_across_runs`, `resync_all_skips_recently_synced_ended_shows`, `resync_show_never_deletes_seasons_or_episodes_tmdb_dropped`, `resync_all_never_touches_movies`, `resync_all_warns_with_counts_when_the_ceiling_skips_shows`
- backend/tests/db.rs — `upsert_show_metadata_updates_existing`, `upsert_season_inserts_then_updates_on_conflict`, `upsert_episode_preserves_watched_state`, `list_resync_candidates_applies_status_and_slow_lane_rules`, `list_resync_candidates_orders_by_last_synced_then_name`
- backend/tests/api.rs — `sync_returns_per_show_results`, `repeated_sync_within_the_cooldown_is_rejected`, `cooldown_is_per_app_state_not_process_global`, `sync_collapses_internal_errors_in_the_per_show_message` (the content-type gate on `/sync` is tested there too, under `app`'s IDs)
- backend/src/scheduler.rs inline tests — `start_succeeds_with_valid_cron_and_returns_scheduler`, `start_returns_config_error_for_invalid_cron`
- backend/src/config.rs inline tests — `defaults_apply_when_only_required_vars_set`, `overrides_pick_up_env_vars`
- frontend/src/pages/Settings.test.tsx (5 tests)
- frontend/src/api/client.test.ts — `sync hits POST /sync`

### Code
- backend/src/logic/resync.rs — `resync_show`, `resync_all` (and `MAX_SHOWS_PER_RESYNC`, `ResyncReport`, `ResyncError`)
- backend/src/db/queries.rs — `upsert_show_metadata`, `upsert_season`, `upsert_episode_preserving_watched`, `list_resync_candidates` (and `ENDED_SHOW_RESYNC_DAYS`)
- backend/src/scheduler.rs — `start`
- backend/src/api/sync.rs — `manual_sync` (and `SyncResponse`, `SyncError`)
- backend/src/state.rs — `RateGate::try_acquire`, `AppState::new` (and `MANUAL_SYNC_MIN_INTERVAL`)
- backend/src/config.rs — `Config::from_env` (`ScheduleConfig.resync_cron` from `RESYNC_CRON`)
- frontend/src/pages/Settings.tsx — `Settings`; frontend/src/api/client.ts — `api.sync` (object-literal property, not annotated by convention)
- Consumed from other segments: `tmdb` (`TmdbClient::get_show`, `get_season`, `TmdbShow::us_providers`, `network_names`), `app` (`Config.schedule`, `AppState.tz`, `AppError::client_message`, the 30 s `API_REQUEST_TIMEOUT` in backend/src/lib.rs), `shows` (the `shows`/`seasons`/`episodes` tables it upserts into)
- Consumers: `app` (backend/src/lib.rs calls `scheduler::start` at startup and routes `POST /api/v1/sync` to `manual_sync`); `shows` and `up-next-calendar` read the rows resync refreshes

## Architecture

**Purpose:** Refresh the mirrored episode tree on a schedule or on demand, bounded against abuse, while never undoing user progress.

**Key Components:**
1. Upsert chain — show `UPDATE`, season and episode `ON CONFLICT DO UPDATE`; the episode SET list omits `watched` and `watched_at`.
2. `resync_all` — eligible shows (status not `Ended`/`Canceled`, or any show unsynced for 30 days) least-recently-synced first, serial loop over at most 100 with per-show error capture.
3. Triggers — `tokio-cron-scheduler` job in the configured timezone; `POST /sync` behind a 60 s per-state gate.
4. Settings page — the one manual control and its per-show result list.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Show | RESYNC-SHOW-001 to 005 | 5 | 0 | 0 |
| Run | RESYNC-RUN-001 to 006 | 6 | 0 | 0 |
| Trigger | RESYNC-TRIGGER-001 to 006 | 6 | 0 | 0 |
| UI | RESYNC-UI-001 to 004 | 4 | 0 | 0 |

**Summary:** 21 of 21 active specs implemented; 0 deferred. (Specs with no test citation: none.)

## Key Findings

1. **Cron and manual runs can overlap** — `scheduler.rs:start` runs `resync_all` without consulting `AppState.sync_gate`; nothing serializes two runs. LLD Deferred 1.
2. **Per-show resync is not transactional** — `resync.rs:resync_show` stamps `last_synced_at` via `upsert_show_metadata` before any season work, and each season/episode upsert autocommits. LLD Deferred 2.
3. **Nothing is ever deleted** — `queries.rs:upsert_season` / `upsert_episode_preserving_watched` are upsert-only, as RESYNC-SHOW-004 requires; stale rows stay in progress counts. LLD Deferred 3.
4. **Manual sync runs under the 30 s request timeout** — `lib.rs:API_REQUEST_TIMEOUT` cuts a long run to 408 after `sync.rs:manual_sync` has already consumed the gate window. LLD Deferred 4.
5. **Scheduled outcomes are log-only** — `scheduler.rs:start` logs the report; Settings shows neither schedule, timezone, nor last result. LLD Deferred 5.

## Work Required

### Must Fix
1. Confirm or refute the three `[inferred]` rows in the LLD decisions table (failure isolation, upsert-only with no deletes, per-statement autocommit).

### Should Fix
2. Serialize cron and manual runs (shared gate or a run lock).
3. Decide whether `resync_show` should be one transaction with `last_synced_at` written last.

### Nice to Have
4. Surface the schedule, timezone, and last scheduled result on Settings.
5. Collapse the duplicated `SyncResponse`/`SyncError` and `ResyncReport`/`ResyncError` structs.
