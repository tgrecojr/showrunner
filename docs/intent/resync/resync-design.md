---
parent: high-level-design
prefix: RESYNC
---

# Resync

## Context and Design Philosophy

A show's season and episode tree is captured once when it is added; resync is how it stays current as TMDB announces new episodes, renames them, or fixes air dates. The one rule that matters most: a resync may rewrite any metadata, but it never touches `watched` or `watched_at`. User progress survives every refresh.

Resync runs two ways: a cron job on `RESYNC_CRON` in the configured `TIMEZONE`, and a manual trigger from the Settings page through `POST /api/v1/sync`. Because every API endpoint is unauthenticated and the watchlist is attacker-growable, both the amount of work one run does and how often a manual run can start are bounded.

## Per-show resync

`resync_show` (backend/src/logic/resync.rs:20-39):

1. Fetch the show from TMDB (`get_show`, with watch providers appended).
2. `upsert_show_metadata` (backend/src/db/queries.rs:495-530) — a plain `UPDATE` of name, overview, images, status, air dates, `in_production`, providers JSON, networks JSON, and `last_synced_at`. It is a no-op if the row is gone.
3. For each season summary with `season_number > 0`, fetch the season and `upsert_season` (queries.rs:532-555), an `INSERT … ON CONFLICT DO UPDATE` of name, overview, air date, and `episode_count` (the number of episodes fetched).
4. For each episode, `upsert_episode_preserving_watched` (queries.rs:559-589) — `INSERT … ON CONFLICT DO UPDATE` whose SET list names only `tmdb_id`, `name`, `overview`, `air_date`, `runtime`. `watched` and `watched_at` are absent from the SET list by design, so an existing row keeps them and a new row gets the defaults.

The steps are separate autocommit statements, not one transaction: `last_synced_at` is bumped in step 2 before any season work, and a failure fetching season *k* leaves seasons before *k* updated. Nothing is ever deleted; a season or episode TMDB has dropped stays in the tree.

## Full resync

`resync_all` (resync.rs:50-98):

- Lists every tracked show id ordered by name case-insensitively (queries.rs:591-597, no bound).
- Truncates to `MAX_SHOWS_PER_RESYNC` (100) and logs a warning with `total`, `limit`, and `skipped` when the ceiling is hit. Because the ordering is deterministic and there is no offset or recency ordering, the same first 100 names are selected on every run; shows past the 100th alphabetically are never resynced by this path. The intended selection is different on two counts: only shows still airing or expected to return are eligible (`RESYNC-RUN-006`), and when the eligible set exceeds the ceiling, runs rotate through it least-recently-synced first (`RESYNC-RUN-005`).
- Walks the ids **serially**, so at most one TMDB request is in flight (:68-69).
- A per-show failure becomes a `ResyncError { tmdb_id, message }` with `message = e.client_message()` so internal detail never reaches the `/sync` body (:76-84); the run continues. `resync_all` itself fails only if listing ids fails.
- Returns `ResyncReport { shows_synced, errors }` and logs counts and elapsed time.

## Triggers

**Scheduled.** `scheduler::start` (backend/src/scheduler.rs) builds one `tokio-cron-scheduler` job with `Job::new_async_tz(RESYNC_CRON, tz, …)`, so the six-field cron expression fires in `AppState.tz`. An unparseable expression is a startup `AppError::Config("Invalid RESYNC_CRON: …")`. Each fire runs `resync_all` and logs the report; nothing is persisted and nothing in the UI shows the last scheduled outcome. The scheduler handle is held for the process lifetime and never shut down. The cron path does not consult the manual-sync gate, so a cron fire and a manual sync can run concurrently.

**Manual.** `manual_sync` (backend/src/api/sync.rs) first calls `sync_gate.try_acquire(MANUAL_SYNC_MIN_INTERVAL)` (60 s, backend/src/state.rs:10, :27-37): check-and-record under one lock, per `AppState` rather than process-global. A second call inside the window is 429 `a sync ran less than 60s ago; try again shortly`. Otherwise it runs `resync_all` and responds 200 with `{ shows_synced, errors: [{ tmdb_id, message }] }`, still 200 when every show failed. The whole run executes inside the perimeter's 30 s request timeout; if cut off, the gate window is already consumed.

**Settings page** (frontend/src/pages/Settings.tsx): one section, "TMDB sync", with copy stating that watched state is preserved and the job also runs on the configured cron schedule. The button "Resync all shows from TMDB" reads "Syncing…" and is disabled while in flight; the result renders `Synced N show(s).`, ` M failed.` when applicable, and a list of `Show #<tmdb_id>: <message>`; a rejected request renders `Error: <message>` ("Sync failed" fallback). The cron expression and timezone are not displayed.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| User state under resync | Episode upsert never writes `watched` / `watched_at` | Full row replace; re-apply from a log | A TMDB refresh must never undo progress (queries.rs:557-558, CLAUDE.md). |
| Concurrency | Strictly serial per run | Bounded parallelism | One in-flight TMDB request is the tightest ceiling and needs no machinery (resync.rs:68-69). |
| Work per run | At most 100 shows | Unbounded; configurable | The show count is attacker-growable; the ceiling bounds the api-key call multiplier, not normal use (resync.rs:41-47). |
| Manual frequency | 60 s cooldown per `AppState` | Process-global; none | Stops an unauthenticated caller re-applying the ceiling in a loop; per-state keeps tests and future multi-instance setups independent (state.rs:12-17). |
| Schedule | `RESYNC_CRON` (6-field) evaluated in `TIMEZONE`, default 06:00 daily | UTC; fixed interval | "Nightly" should mean the user's night (env.example:14-19, scheduler.rs:13). |
| Failure isolation | Per-show errors collected; run and `/sync` return 200 | Abort on first error | `[inferred]` One bad show must not block the rest; the report makes partial outcomes visible. |
| Error text in the report | `client_message()` | Raw error Display | The `/sync` body is a 200 response, so internal detail must be filtered there too (resync.rs:80-82, backend/src/error.rs:53-54). |
| Specials | Season 0 skipped | Resync everything stored | Mirrors add (resync.rs:26). |
| Deletions | Never delete seasons or episodes | Reconcile against TMDB's list | `[inferred]` Upsert-only is simpler and cannot lose a watched row; the cost is stale rows. |
| Transaction scope | Per-statement autocommit | One transaction per show | `[inferred]` Keeps SQLite write locks short during a long serial run. |
| Selection under the cap | Intended: least-recently-synced eligible shows first (`RESYNC-RUN-005`); today: by name | Same alphabetical prefix every run | Every eligible show must be refreshed eventually; name order starves everything past the ceiling. |
| Eligibility | Intended: skip shows whose status is `Ended` or `Canceled` (`RESYNC-RUN-006`); today: every tracked show | Resync everything on every run | An ended show has nothing new to fetch; excluding it spends the ceiling where metadata can still change. |

## Open Questions & Future Decisions

### Resolved
1. ✅ **Every eligible show gets resynced.** When the eligible set exceeds the per-run ceiling, runs rotate through it least-recently-synced first rather than re-selecting the same alphabetical prefix. Not yet implemented (`list_tracked_show_ids` orders by name, queries.rs:593); tracked as `RESYNC-RUN-005`.
2. ✅ **Ended shows are not resynced.** A show whose TMDB status is `Ended` or `Canceled` has nothing new to fetch, so it is excluded from the eligible set, which also keeps the ceiling spent on shows that can change. Not yet implemented; tracked as `RESYNC-RUN-006`.

### Deferred
1. **Revival of an ended show.** Once excluded under `RESYNC-RUN-006`, a show that TMDB later flips back to `Returning Series` is never observed. Is a rare low-frequency pass over ended shows (or a manual per-show resync) wanted, or is "remove and re-add" acceptable?
2. **Status vocabulary.** Eligibility keys off TMDB's `status` strings; the ones observed are `Returning Series`, `Ended`, `Canceled`, `In Production`, `Planned`, `Pilot`. Confirm that only `Ended` and `Canceled` mean "no longer airing".
3. **Settings copy** says "Refreshes every show's seasons and episodes" (Settings.tsx:30); it will need to mention that ended shows are skipped once `RESYNC-RUN-006` lands.
4. **Overlapping runs.** The cron job bypasses the manual gate; a scheduled fire and a manual sync can run `resync_all` concurrently against the same rows.
5. **Partial updates.** `resync_show` is not transactional and stamps `last_synced_at` before season work, so a mid-run TMDB failure leaves a show marked synced with a half-updated tree.
6. **Stale rows.** Episodes TMDB has removed stay in the tree and in progress counts (surfaced to `shows`).
7. **Manual sync under the request timeout.** A library large enough to exceed 30 s gets a 408 while the gate window stays consumed.
8. **Upstream error wording.** `get_show` and `get_season` surface raw `TMDB returned 429`; those strings appear verbatim in the Settings error list. The app-wide intent is a friendly message; the mapping is `tmdb`'s.
9. **No visibility of scheduled runs.** Outcomes are only logged; Settings shows neither the schedule, the timezone, nor the last result.
10. **Shape duplication.** `SyncError` / `SyncResponse` duplicate `ResyncError` / `ResyncReport` field for field (sync.rs:9-19 vs resync.rs:8-18).
11. **Scheduler lifetime.** Held as `_scheduler` and never shut down (backend/src/lib.rs:250).

## References

- backend/src/logic/resync.rs; backend/src/scheduler.rs; backend/src/api/sync.rs
- backend/src/db/queries.rs:492-597 (`upsert_show_metadata`, `upsert_season`, `upsert_episode_preserving_watched`, `list_tracked_show_ids`)
- backend/src/state.rs (`MANUAL_SYNC_MIN_INTERVAL`, `RateGate`)
- backend/src/config.rs (`ScheduleConfig`, `RESYNC_CRON`, `TIMEZONE`)
- frontend/src/pages/Settings.tsx
- backend/tests/logic.rs (6 tests); backend/tests/api.rs:558-592, :1159-1250; backend/tests/db.rs:175-283, :314-352; backend/src/scheduler.rs tests
- frontend/src/pages/Settings.test.tsx (4 tests)
- Consumed: `tmdb` (`get_show`, `get_season`), `shows` (the tables it upserts into), `app` (`Config`, `AppState`, request timeout)
