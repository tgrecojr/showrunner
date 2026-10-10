# Arrow: resync

Keeping show metadata current from TMDB without touching watched state: the per-show upsert chain, the bounded serial full run, the cron trigger, the rate-gated manual trigger, and the Settings page.

## Status

**MAPPED** — sampled 2026-10-10 (git SHA `3b525e1`), not yet audited. Skeleton LLD and EARS specs were reverse-engineered from code; design rationale carries `[inferred]` markers until confirmed.

## References

### HLD
- docs/high-level-design.md (System Design → resync)

### LLD
- docs/intent/resync/resync-design.md

### EARS
- docs/intent/resync/resync-specs.md (21 specs: 19 implemented, 2 active gaps)

### Tests
- backend/tests/logic.rs — per-show preservation and season-0 skip (:14, :64), per-show failure isolation (:98), empty run (:125), 100-show cap (:162, :184)
- backend/tests/db.rs — `upsert_show_metadata` (:175), `upsert_season` (:200), `upsert_episode_preserves_watched_state` (:230), networks backfill (:314), id ordering (:276)
- backend/tests/api.rs — `/sync` report shape (:558), cooldown and per-state gate (:1187, :1222), content-type gate on `/sync` (:903-962, owned by `app`)
- backend/src/scheduler.rs unit tests (valid and invalid cron)
- frontend/src/pages/Settings.test.tsx (4 tests); frontend/src/api/client.test.ts `sync` (:137)

### Code
- backend/src/logic/resync.rs (`resync_show` :20, `MAX_SHOWS_PER_RESYNC` :48, `resync_all` :50)
- backend/src/scheduler.rs; backend/src/api/sync.rs
- backend/src/db/queries.rs — `upsert_show_metadata` (:495), `upsert_season` (:532), `upsert_episode_preserving_watched` (:559), `list_tracked_show_ids` (:591)
- backend/src/state.rs — `MANUAL_SYNC_MIN_INTERVAL` (:10), `RateGate` (:19)
- frontend/src/pages/Settings.tsx; frontend/src/api/client.ts (:81)
- Consumed from other segments: `tmdb` (`get_show`, `get_season`), `app` (`Config.schedule`, `AppState.tz`, 30 s request timeout)

## Architecture

**Purpose:** Refresh the mirrored episode tree on a schedule or on demand, bounded against abuse, while never undoing user progress.

**Key Components:**
1. Upsert chain — show `UPDATE`, season and episode `ON CONFLICT DO UPDATE`; the episode SET list omits `watched` and `watched_at`.
2. `resync_all` — serial loop over at most 100 ids with per-show error capture.
3. Triggers — `tokio-cron-scheduler` job in the configured timezone; `POST /sync` behind a 60 s per-state gate.
4. Settings page — the one manual control and its per-show result list.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Show | RESYNC-SHOW-001 to 005 | 5 | 0 | 0 |
| Run | RESYNC-RUN-001 to 006 | 4 | 0 | 2 |
| Trigger | RESYNC-TRIGGER-001 to 006 | 6 | 0 | 0 |
| UI | RESYNC-UI-001 to 004 | 4 | 0 | 0 |

**Summary:** 19 of 21 active specs implemented; 2 gaps (`RESYNC-RUN-005` rotate least-recently-synced first, `RESYNC-RUN-006` skip ended and canceled shows).

## Key Findings

1. **The per-run cap never rotates** — ids are ordered by name with no offset (backend/src/db/queries.rs:593), so the same first 100 are chosen every run while the warning at backend/src/logic/resync.rs:61 claims the rest are "deferred to the next run". Intended behavior: `RESYNC-RUN-005` and `RESYNC-RUN-006`.
2. **Cron and manual runs can overlap** — the scheduled job (backend/src/scheduler.rs:19) bypasses `RateGate`; nothing serializes two `resync_all` executions.
3. **Per-show resync is not transactional** — `last_synced_at` is written first (resync.rs:22) and each season/episode is its own statement; a mid-run failure leaves a partially updated show marked synced.
4. **Nothing is ever deleted** — stale seasons and episodes persist (queries.rs:532-589 are upsert-only).
5. **Manual sync runs under the 30 s request timeout** (backend/src/lib.rs:35) with the gate already consumed (sync.rs:24).
6. **Raw upstream status text reaches the Settings error list** via `get_show` / `get_season` (backend/src/datasources/tmdb.rs:85-90, :136-142).
7. **Scheduled outcomes are log-only** (scheduler.rs:20-25); the UI cannot show the last run.

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` decisions (failure isolation, upsert-only with no deletes, per-statement autocommit, name ordering under the cap).
2. Implement `RESYNC-RUN-005` (rotation by `last_synced_at`) and `RESYNC-RUN-006` (exclude `Ended` / `Canceled`), and update the Settings copy and the warning text to match.

### Should Fix
3. Serialize cron and manual runs (shared gate or a run lock).
4. Decide whether `resync_show` should be one transaction with `last_synced_at` written last.

### Nice to Have
5. Surface the schedule, timezone, and last scheduled result on Settings.
6. Collapse the duplicated report/response structs.
