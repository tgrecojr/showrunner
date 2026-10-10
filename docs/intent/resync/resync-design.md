---
parent: high-level-design
prefix: RESYNC
---

# Resync

## Context and Design Philosophy

A show's season and episode tree is captured once when it is added; resync is how it stays current as TMDB announces new episodes, renames them, or fixes air dates. The one rule that matters most: a resync may rewrite any metadata, but it never touches `watched` or `watched_at`. User progress survives every refresh.

Resync runs two ways: a cron job on `RESYNC_CRON` in the configured `TIMEZONE`, and a manual trigger from the Settings page through `POST /api/v1/sync`. Because every API endpoint is unauthenticated and the watchlist is attacker-growable, both the amount of work one run does and how often a manual run can start are bounded.

## Per-show resync

`resync_show` (backend/src/logic/resync.rs):

1. Fetch the show from TMDB (`get_show`, with watch providers appended).
2. `upsert_show_metadata` (backend/src/db/queries.rs) — a plain `UPDATE` of name, overview, images, status, air dates, `in_production`, providers JSON, networks JSON, and `last_synced_at`. It is a no-op if the row is gone.
3. For each season summary with `season_number > 0`, fetch the season and `upsert_season` (queries.rs), an `INSERT … ON CONFLICT DO UPDATE` of name, overview, air date, and `episode_count` (the number of episodes fetched).
4. For each episode, `upsert_episode_preserving_watched` (queries.rs) — `INSERT … ON CONFLICT DO UPDATE` whose SET list names only `tmdb_id`, `name`, `overview`, `air_date`, `runtime`. `watched` and `watched_at` are absent from the SET list by design, so an existing row keeps them and a new row gets the defaults.

The steps are separate autocommit statements, not one transaction: `last_synced_at` is bumped in step 2 before any season work, and a failure fetching season *k* leaves seasons before *k* updated. Nothing is ever deleted; a season or episode TMDB has dropped stays in the tree.

## Full resync

`resync_all` (resync.rs):

- Selects the **eligible** shows (`list_resync_candidates`, queries.rs): every show whose stored TMDB `status` is not `Ended` or `Canceled` (a NULL or unrecognized status counts as eligible, so a new status TMDB invents can never silently freeze a show), plus any ended or canceled show whose `last_synced_at` is more than 30 days old. The 30-day comparison is done with SQLite's `datetime()` so the stored RFC3339 strings compare as instants regardless of their UTC suffix form. Ended shows therefore get a slow lane rather than a freeze: a revival TMDB flips back to `Returning Series`, or a late fix to an old air date, is picked up within a month.
- Orders the eligible set by `last_synced_at` ascending (NULL first) and then name, and takes the first `MAX_SHOWS_PER_RESYNC` (100). Because every successful resync bumps `last_synced_at`, consecutive runs rotate through the whole eligible set, and a show whose fetch keeps failing keeps the head slot and is retried first rather than starved; that costs one slot of 100 and is the intended trade-off.
- Logs a warning with `total` (eligible), `limit`, and `skipped` when the eligible set exceeds the ceiling.
- Walks the ids **serially**, so at most one TMDB request is in flight (the `for id in ids` loop in `resync_all`).
- A per-show failure becomes a `ResyncError { tmdb_id, message }` with `message = e.client_message()` so internal detail never reaches the `/sync` body (the `Err` arm of that loop); the run continues. `resync_all` itself fails only if listing ids fails.
- Returns `ResyncReport { shows_synced, errors }` and logs counts and elapsed time.

## Triggers

**Scheduled.** `scheduler::start` (backend/src/scheduler.rs) builds one `tokio-cron-scheduler` job with `Job::new_async_tz(RESYNC_CRON, tz, …)`, so the six-field cron expression fires in `AppState.tz`. An unparseable expression is a startup `AppError::Config("Invalid RESYNC_CRON: …")`. Each fire runs `resync_all` and logs the report; nothing is persisted and nothing in the UI shows the last scheduled outcome. The scheduler handle is held for the process lifetime and never shut down. The cron path does not consult the manual-sync gate, so a cron fire and a manual sync can run concurrently.

**Manual.** `manual_sync` (backend/src/api/sync.rs) first calls `sync_gate.try_acquire(MANUAL_SYNC_MIN_INTERVAL)` (60 s, backend/src/state.rs: `MANUAL_SYNC_MIN_INTERVAL`, `RateGate::try_acquire`): check-and-record under one lock, per `AppState` rather than process-global. A second call inside the window is 429 `a sync ran less than 60s ago; try again shortly`. Otherwise it runs `resync_all` and responds 200 with `{ shows_synced, errors: [{ tmdb_id, message }] }`, still 200 when every show failed. The whole run executes inside the perimeter's 30 s request timeout; if cut off, the gate window is already consumed.

**Settings page** (frontend/src/pages/Settings.tsx): one section, "TMDB sync", with copy stating which shows a run refreshes (those still airing or expected to return, with ended shows checked about monthly), that watched state is preserved, and that the job also runs on the configured cron schedule. The button "Resync all shows from TMDB" reads "Syncing…" and is disabled while in flight; the result renders `Synced N show(s).`, ` M failed.` when applicable, and a list of `Show #<tmdb_id>: <message>`; a rejected request renders `Error: <message>` ("Sync failed" fallback). The cron expression and timezone are not displayed.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| User state under resync | Episode upsert never writes `watched` / `watched_at` | Full row replace; re-apply from a log | A TMDB refresh must never undo progress (the doc comment on `upsert_episode_preserving_watched` in queries.rs, CLAUDE.md). |
| Concurrency | Strictly serial per run | Bounded parallelism | One in-flight TMDB request is the tightest ceiling and needs no machinery (the serial `for id in ids` loop in `resync_all`). |
| Work per run | At most 100 shows | Unbounded; configurable | The show count is attacker-growable; the ceiling bounds the api-key call multiplier, not normal use (the doc comment on `MAX_SHOWS_PER_RESYNC` in resync.rs). |
| Manual frequency | 60 s cooldown per `AppState` | Process-global; none | Stops an unauthenticated caller re-applying the ceiling in a loop; per-state keeps tests and future multi-instance setups independent (the doc comment on `RateGate` in state.rs). |
| Schedule | `RESYNC_CRON` (6-field) evaluated in `TIMEZONE`, default 06:00 daily | UTC; fixed interval | "Nightly" should mean the user's night (the `TIMEZONE` and `RESYNC_CRON` entries in env.example; `Job::new_async_tz` in `scheduler::start`). |
| Failure isolation | Per-show errors collected; run and `/sync` return 200 | Abort on first error | One bad show must not block the rest; the report makes partial outcomes visible. |
| Error text in the report | `client_message()` | Raw error Display | The `/sync` body is a 200 response, so internal detail must be filtered there too (the `ResyncError` construction in `resync_all`; `AppError::client_message` in backend/src/error.rs). |
| Specials | Season 0 skipped | Resync everything stored | Mirrors add (the `season_number == 0` skip in `resync_show`). |
| Deletions | Never delete seasons or episodes | Reconcile against TMDB's list | Upsert-only is simpler and cannot lose a watched row; the cost is stale rows. Reconciling unwatched rows against TMDB's list is the design to adopt if stale rows surface in practice. |
| Transaction scope | Per-statement autocommit | One transaction per show | Keeps SQLite write locks short during a long serial run. |
| Selection under the cap | Least-recently-synced eligible shows first, then name | Alphabetical prefix every run; random sample | Every eligible show must be refreshed eventually; name order starves everything past the ceiling, and `last_synced_at` is already written by every successful resync. |
| Eligibility | Status not `Ended` / `Canceled`, or `last_synced_at` older than 30 days | Resync everything every run; freeze ended shows permanently; key off `in_production`; a manual per-show resync | An ended show rarely changes, so refreshing it daily wastes ceiling; freezing it forever would miss revivals and TMDB corrections, and remove-and-re-add loses watched state. `in_production` is false between seasons too, so it cannot mean "still airing". A monthly slow lane costs a few slots per run and needs no new endpoint. |
| Failing shows under rotation | Keep the head slot until a fetch succeeds | Stamp `last_synced_at` on failure | `last_synced_at` must mean "TMDB answered"; retrying a broken show first costs one slot and keeps it from being silently starved. |

## Open Questions & Future Decisions

### Resolved
*(none yet)*

### Deferred
1. **Overlapping runs.** The cron job bypasses the manual gate; a scheduled fire and a manual sync can run `resync_all` concurrently against the same rows.
2. **Partial updates.** `resync_show` is not transactional and stamps `last_synced_at` before season work, so a mid-run TMDB failure leaves a show marked synced with a half-updated tree.
3. **Stale rows.** Episodes TMDB has removed stay in the tree and in progress counts (surfaced to `shows`).
4. **Manual sync under the request timeout.** A library large enough to exceed 30 s gets a 408 while the gate window stays consumed.
5. **No visibility of scheduled runs.** Outcomes are only logged; Settings shows neither the schedule, the timezone, nor the last result.
6. **Shape duplication.** `SyncError` / `SyncResponse` duplicate `ResyncError` / `ResyncReport` field for field (backend/src/api/sync.rs vs backend/src/logic/resync.rs).
7. **Scheduler lifetime.** Held as `_scheduler` and never shut down (the `_scheduler` binding in `run`, backend/src/lib.rs).

## References

- backend/src/logic/resync.rs; backend/src/scheduler.rs; backend/src/api/sync.rs
- backend/src/db/queries.rs (`upsert_show_metadata`, `upsert_season`, `upsert_episode_preserving_watched`, `list_resync_candidates`)
- backend/src/state.rs (`MANUAL_SYNC_MIN_INTERVAL`, `RateGate`)
- backend/src/config.rs (`ScheduleConfig`, `RESYNC_CRON`, `TIMEZONE`)
- frontend/src/pages/Settings.tsx
- backend/tests/logic.rs (11 tests); backend/tests/api.rs (`sync_returns_per_show_results`, `repeated_sync_within_the_cooldown_is_rejected`, `cooldown_is_per_app_state_not_process_global`, `sync_collapses_internal_errors_in_the_per_show_message`); backend/tests/db.rs (`upsert_show_metadata_updates_existing`, `upsert_season_inserts_then_updates_on_conflict`, `upsert_episode_preserves_watched_state`, `list_resync_candidates_applies_status_and_slow_lane_rules`, `list_resync_candidates_orders_by_last_synced_then_name`); backend/src/scheduler.rs tests
- frontend/src/pages/Settings.test.tsx (5 tests)
- Consumed: `tmdb` (`get_show`, `get_season`), `shows` (the tables it upserts into), `app` (`Config`, `AppState`, request timeout)
