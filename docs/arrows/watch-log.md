# Arrow: watch-log

The append-only history of every watched and unwatched action, written inside the mutating transaction by `shows` and `movies`, read newest-first through a paginated endpoint, and shown on the read-only History page.

## Status

**MAPPED** — sampled 2026-10-10 (git SHA `3b525e1`), not yet audited. Skeleton LLD and EARS specs were reverse-engineered from code; design rationale carries `[inferred]` markers until confirmed.

## References

### HLD
- docs/high-level-design.md (System Design → watch-log)

### LLD
- docs/intent/watch-log/watch-log-design.md

### EARS
- docs/intent/watch-log/watch-log-specs.md (18 specs: 18 implemented, 0 active gaps)

### Tests
- backend/tests/db.rs — writer side effects (:656-822), survival after show removal (:826), pagination and clamping (:842-882)
- backend/tests/api.rs — mark-watched and delete log counts (:1272-1302), page shape and validation (:1305-1382)
- backend/src/models/watch_log.rs unit test (`action_str`)
- frontend/src/pages/History.test.tsx (7 tests, including the `describeEntry` sentence matrix)
- Not covered: `client.test.ts` has no `watchLog` test

### Code
- backend/src/db/watch_log.rs (`insert_entry` :15, `count_entries` :42, `list_entries` :51)
- backend/src/api/watch_log.rs; backend/src/models/watch_log.rs
- backend/src/db/migrations/20260911000000_add_watch_log.sql
- frontend/src/pages/History.tsx; frontend/src/pages/watchLogText.ts; frontend/src/api/client.ts (:82)
- Writers in other segments: backend/src/db/queries.rs `set_episode_watched` (:788), `bulk_set_watched` (:920), `mark_movie_watched` (:391)

## Architecture

**Purpose:** Keep a trustworthy record of what was marked watched or unwatched, independent of whether the show or movie still exists.

**Key Components:**
1. `watch_log` table — snapshot columns, no foreign keys, ordered index.
2. `insert_entry` — transaction-only insert that stamps the time.
3. `GET /watch-log` — validated, clamped offset pagination.
4. History page — URL-driven paging, per-scope sentences, read-only by design.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Data | WATCHLOG-DATA-001 to 005 | 5 | 0 | 0 |
| API | WATCHLOG-API-001 to 005 | 5 | 0 | 0 |
| UI | WATCHLOG-UI-001 to 008 | 8 | 0 | 0 |

**Summary:** 18 of 18 active specs implemented.

## Key Findings

1. **Duplicate rows from repeated single-episode marks** — a consequence of `shows`' non-idempotent PATCH (backend/src/db/queries.rs:776-786), already resolved there as `SHOWS-WATCHED-010`; this segment inherits the fix.
2. **Vocabulary is unenforced** — `media_type`, `action`, `scope` have documented values but no CHECK constraints (add_watch_log.sql:9-11).
3. **Count and page are separate statements** (watch_log.rs:56-68); `total` can drift under concurrent writes.
4. **`page` is unbounded** while `per_page` is capped; a page past the end renders `Page 7 of 3` (History.tsx:81, :133).
5. **Display time is browser-local** (History.tsx:9-19) while the rest of the app reasons in the server's `TIMEZONE`.
6. **Borrowed styling** — rows use `upnext-*` classes and the `history-page` wrapper has no rule (frontend/src/index.css).
7. **Writer spec placement** — the obligation to log lives in `SHOWS-WATCHED-003`, `SHOWS-WATCHED-007`, and `MOVIES-API-009`; a change to the row shape cascades into both of those segments.

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` decisions (free-text vocabulary columns, unlinked movie titles, page in URL, browser-local time).

### Should Fix
2. Add a `client.test.ts` case for `watchLog` URL construction.
3. Decide whether `scope`/`action`/`media_type` get CHECK constraints in a migration.

### Nice to Have
4. Clamp or redirect an out-of-range `?page=`.
5. Show a loading state on page change.
