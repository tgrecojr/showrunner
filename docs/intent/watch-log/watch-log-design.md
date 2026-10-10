---
parent: high-level-design
prefix: WATCHLOG
---

# Watch log

## Context and Design Philosophy

The watch log is an append-only record of every watched and unwatched action, surfaced on the History page so an accidental bulk mark can be spotted. It is written at the moment a state change is applied, inside the same transaction, by the mutation paths that `shows` and `movies` own. Rows snapshot the title and poster and carry no foreign keys, so history outlives the show or movie it describes; marking a movie watched deletes the movie row, and the log row is the only trace left.

This segment owns the table, the insert contract, the paginated read endpoint, and the History page. It does not own the decision of *when* to log; each writer's segment carries that obligation as a spec that cites this one.

## Data model

`watch_log` (backend/src/db/migrations/20260911000000_add_watch_log.sql):

| Column | Type | Meaning |
|---|---|---|
| `id` | INTEGER PK AUTOINCREMENT | insertion order; tiebreaker for same-instant rows |
| `occurred_at` | TEXT NOT NULL | UTC RFC3339, stamped by `insert_entry` |
| `media_type` | TEXT NOT NULL | `tv` or `movie` |
| `action` | TEXT NOT NULL | `watched` or `unwatched` |
| `scope` | TEXT NOT NULL | `episode`, `season`, `show`, `through_episode`, or `movie` |
| `tmdb_id` | INTEGER NOT NULL | the show or movie id |
| `title`, `poster_path` | TEXT | snapshot of the show or movie at the time |
| `season_number`, `episode_number`, `episode_name` | nullable | present for `episode` and `through_episode`; season only for `season`; absent for `show` and `movie` |
| `episode_count` | INTEGER NOT NULL DEFAULT 1 | episodes whose state actually changed |

Index `idx_watch_log_order (occurred_at DESC, id DESC)` matches the read order. The vocabularies for `media_type`, `action`, and `scope` are comments on the schema, not CHECK constraints; the only place they are enumerated in code is the writers in backend/src/db/queries.rs.

## Writing

`insert_entry` (backend/src/db/watch_log.rs) takes `&mut Transaction`, not a pool, so a caller cannot log outside the transaction that performs the change. It stamps `occurred_at` itself. `NewWatchLogEntry` (backend/src/models/watch_log.rs) is built by the writer with borrowed strings; `action_str` maps the boolean to `watched` / `unwatched`.

Writers and what they record:

| Writer (segment) | Rows | `scope` | `episode_count` |
|---|---|---|---|
| single-episode PATCH (`shows`) | one per call | `episode` | 1 |
| bulk-watch (`shows`) | one, only if ≥ 1 episode changed | `show` / `season` / `through_episode` | episodes changed |
| mark movie watched (`movies`) | one | `movie` | 1 |

Adding, removing a show, removing a movie, and resync never write a row. There is no update or delete path for the table.

## Reading

`GET /api/v1/watch-log?page=&per_page=` (backend/src/api/watch_log.rs): `page` defaults to 1 and must be ≥ 1 (else 400 `page must be >= 1`); `per_page` defaults to 50, must be ≥ 1 (else 400 `per_page must be >= 1`), and is silently clamped to 100. `list_entries` (backend/src/db/watch_log.rs) re-clamps defensively, runs `COUNT(*)` and then the page query (`ORDER BY occurred_at DESC, id DESC LIMIT ? OFFSET ?`) as two separate statements, maps `poster_path` to a w185 `poster_url`, and returns `{ entries, page, per_page, total }` with the effective `per_page` echoed. A page past the end returns empty `entries` with the unchanged `total`.

## History page

`History` (frontend/src/pages/History.tsx):

- The page number comes from the `?page=` search param; anything that is not an integer ≥ 1 reads as 1 (`parsePage`). Navigation writes the param back, omitting it for page 1 (`goTo`), so the browser's back button and bookmarks work.
- Requests always use `per_page` 50 (the `PER_PAGE` const passed to `api.watchLog`). `totalPages` is computed from the server's echoed `per_page`.
- States: `Error:` when nothing has loaded; "Loading…"; "Nothing logged yet" when `total` is 0 (the early returns in `History`). On a page change the previous page stays visible until the new one arrives; if that request fails, `Error:` renders above the still-visible previous page and clears when the next page request starts.
- Rows reuse the Up Next row classes: poster or "No poster"; the title links to `/shows/{id}` for `tv` and is plain text for `movie` because the movie row no longer exists; a one-line sentence from `describeEntry`; a `<time dateTime>` showing `occurred_at` in the browser's locale and zone (`formatTime` and the `upnext-list` rows in `History`). `unwatched` rows get `history-row-unwatched` and render muted.
- The copy states the page is read-only: "Undo mistakes from the show or movie page." (the intro `status` paragraph in `History`).
- Pager: `Page X of Y · N entries`, Previous disabled on page 1, Next disabled on the last page (the `pager` block in `History`).

`describeEntry` (frontend/src/pages/watchLogText.ts) is a separate module because the lint configuration forbids non-component exports from component files. Its sentence matrix:

| `scope` | Sentence |
|---|---|
| `episode` | `Marked S02E05 "Name" watched` (name omitted when null; no count) |
| `season` | `Marked Season 2 watched · 8 episodes` |
| `show` | `Marked all episodes watched · 13 episodes` |
| `through_episode` | `Marked through S02E05 watched · 13 episodes` |
| `movie` | `Marked watched` (the title is rendered separately) |

The verb is `unwatched` for `unwatched` rows; missing season or episode numbers render as `00` in the code and `?` in the season sentence.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| Durability | Append-only, no foreign keys, title and poster snapshotted | FK to shows/movies with cascade | History must outlive the thing it describes; a watched movie's row is deleted (header comment of add_watch_log.sql). |
| Atomicity | `insert_entry` takes the caller's transaction | Log after commit | A log row is committed with the change it describes, never without it (module doc comment of backend/src/db/watch_log.rs). |
| Bulk granularity | One row per bulk action carrying `episode_count` | One row per episode | A season mark is one user action; the count still says how much changed (CLAUDE.md). |
| What is logged | Watched and unwatched changes only | Also adds and removals | The log answers "what did I watch?"; removing a movie is a change of mind, not a viewing (doc comments on `mark_movie_watched` and `delete_movie` in backend/src/api/movies.rs). |
| Pagination | Offset, 1-based, `per_page` capped at 100 | Cursor | A single-user history is small; the cap bounds an unauthenticated response (doc comment on `list_watch_log` in backend/src/api/watch_log.rs). |
| Order | Newest first by `occurred_at`, then `id` | Oldest first | Recent mistakes are what the page is for; `id` breaks same-instant ties (index definition). |
| Undo | None on this page | Inline undo button | Undo is the show or movie page's job; keeping History read-only keeps it a record (the intro `status` paragraph in History.tsx, CLAUDE.md). |
| Vocabulary columns | Free TEXT with documented values | CHECK constraints or lookup tables | `[inferred]` Writers are few and in one file; constraints were not judged worth a migration. |
| Movie titles | Plain text, not linked | Link to `/movies/{id}` | `[inferred]` The movie row is gone once watched, so the link would 404. |
| Page in URL | `?page=` search param | Component state | `[inferred]` Back button and bookmarks. |
| Time display | Browser locale and zone | Server `TIMEZONE`; raw UTC | `[inferred]` `occurred_at` is an instant, so the reader's local time is the natural rendering. |

## Open Questions & Future Decisions

### Resolved
*(none yet)*

### Deferred
1. **Vocabulary enforcement.** No CHECK constraints on `media_type`, `action`, `scope`; a writer bug would persist silently.
2. **`total` and the page are two statements**, so `total` can drift from the page under concurrent writes.
3. **Unbounded `page`.** `per_page` is capped but `page` is not; a huge offset scans.
4. **Page past the end** renders `Page 7 of 3` with an empty list and Next disabled; no clamp or redirect.
5. **Time zone of display.** Browser-local, whereas the rest of the app's "today" is the server's `TIMEZONE`.
6. **Styling reuse.** Rows borrow `upnext-*` classes; the `history-page` wrapper class has no stylesheet rule.
7. **No loading indicator on page change**; stale entries remain until the next page arrives.
8. **Test hygiene.** History.test.tsx has no `afterEach(vi.restoreAllMocks)`, unlike sibling page tests.

## References

- backend/src/db/watch_log.rs; backend/src/api/watch_log.rs; backend/src/models/watch_log.rs
- backend/src/db/migrations/20260911000000_add_watch_log.sql
- frontend/src/pages/History.tsx; frontend/src/pages/watchLogText.ts
- backend/tests/db.rs: writer side effects (`set_episode_watched_logs_watch_and_unwatch` through `watch_log_survives_show_removal`), pagination (`list_entries_paginates_newest_first`), clamping (`list_entries_clamps_per_page`); backend/tests/api.rs: `watch_log_exposes_no_update_or_delete_route`, `watch_log_returns_page_shape`, `watch_log_rejects_bad_page_and_clamps_per_page`, `mark_movie_watched_returns_204_then_404`, `delete_movie_writes_no_log_entry`
- frontend/src/pages/History.test.tsx (9 tests incl. the `describeEntry` matrix)
- Writers: `shows` (`set_episode_watched`, `bulk_set_watched`), `movies` (`mark_movie_watched`)
