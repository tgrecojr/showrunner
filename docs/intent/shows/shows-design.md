---
parent: high-level-design
prefix: SHOWS
---

# Shows

## Context and Design Philosophy

A show on the watchlist is a locally mirrored TMDB season/episode tree plus one bit of user state per episode: `watched`. Everything the user sees about progress is derived from that tree against "today" in the configured timezone. The segment owns adding a show (which pulls the whole tree once), listing and reading it, removing it, and every mutation of watched state, whether one checkbox or a bulk action. It also owns the definition of "aired" that other segments cite.

Two invariants shape the mutation paths. Bulk actions only ever touch episodes that have already aired, so a careless click cannot mark next week's episode watched. The single-episode toggle is the deliberate escape hatch and applies no date filter. Both write a row to the watch log inside the same transaction as the state change; the log's row shape and page belong to `watch-log`, but the obligation to write it lives here.

## Data model

Tables (backend/src/db/migrations/20260509000000_initial.sql, extended by 20260921000000_add_show_networks.sql):

| Table | Key | Columns of note |
|---|---|---|
| `shows` | `tmdb_id` | name, overview, poster_path, backdrop_path, status, first/last_air_date, in_production, watch_providers_json, networks_json, added_at, last_synced_at |
| `seasons` | (show_tmdb_id, season_number) | name, overview, air_date, episode_count; FK → shows ON DELETE CASCADE |
| `episodes` | (show_tmdb_id, season_number, episode_number) | tmdb_id, name, overview, air_date, runtime, watched, watched_at; FK → seasons ON DELETE CASCADE |

All dates are `TEXT` in `YYYY-MM-DD`; timestamps are UTC RFC3339 strings. Date comparisons in SQL are bytewise, which is correct only while every stored and bound value is zero-padded `YYYY-MM-DD`. `watched` and `in_production` are stored as integers and converted with `!= 0`.

Wire shapes (backend/src/models/show.rs): `WatchlistItem` (:68) carries progress counts and `next_episode_air_date` but no providers or networks; `ShowDetail` (:81) carries `watch_providers` and the full `seasons[].episodes[]` tree but no networks; `networks_json` is read only by `up-next-calendar`. `ShowRow` (:24) omits `networks_json` even though the column exists.

## Adding a show

`add_show` (backend/src/api/shows.rs:22-54):

1. Reject a `tmdb_id` already on the watchlist with 400 `show N is already on the watchlist` before any TMDB call (:26-31). The check and the later insert are separate statements; a concurrent duplicate add would hit the primary key and surface as a 500.
2. Fetch the show from TMDB; a TMDB 404 becomes HTTP 404 (`tmdb` owns that mapping).
3. Fetch every season with `season_number > 0`, one serial TMDB call each (:35-46). Season 0 (Specials) is never fetched or stored. All fetches complete before anything is written, so a failure mid-way persists nothing. The whole add runs under the perimeter's 30 s request timeout.
4. `insert_show_full` (backend/src/db/queries.rs:31-92) inserts show, seasons, and episodes in one transaction. US watch providers and network names are serialized as JSON arrays; empty strings become NULL; `last_synced_at` is stamped now. A season's stored `episode_count` is the number of episodes TMDB returned for it (:81), not TMDB's summary count.
5. Respond 201 with the new `WatchlistItem` (:50-53).

## Reading

- **Watchlist** — `list_watchlist` (queries.rs:137-168) selects up to `MAX_LIST_ROWS` (500) shows ordered by name case-insensitively, then issues two further queries per show (`episode_counts`, `next_unaired_air_date`), so a full list costs 1 + 2N statements.
- **Detail** — `get_show_detail` (queries.rs:204-282) reads the show, parses `watch_providers_json` (unparseable → empty list), reads seasons by number, then episodes per season by number (2 + S statements). Each season's `watched_count` counts every watched episode regardless of air date, which differs from the watchlist's aired-only `watched_count`.
- **Not found** — detail, delete, and bulk-watch all answer 404 `show N not on watchlist` for an unknown id.

## "Today" and aired

`today_in(tz)` (backend/src/state.rs:60-62) is the current UTC instant shifted into the configured `TIMEZONE` and formatted `YYYY-MM-DD`. An episode is **aired** when `air_date IS NOT NULL AND air_date <= today`. On the watchlist (`episode_counts`, queries.rs:451-469):

- `watched_count` = episodes that are watched **and** aired
- `aired_count` = aired episodes
- `total_count` = all episodes, including unaired and undated
- `next_episode_air_date` = earliest `air_date > today`, else null (queries.rs:471-486)

An episode marked watched before it airs (possible via the single-episode toggle) therefore counts in `total_count` but not `watched_count` until its air date passes.

## Watched-state mutations

| Path | Filter | Idempotent | Log rows | Returns |
|---|---|---|---|---|
| `PATCH /episodes/{show}/{season}/{ep}` → `set_episode_watched` (queries.rs:746-819) | none (any air date) | yes: `watched != ?` guard, so re-sending the current state touches neither `watched` nor `watched_at` | exactly one when the flag changed, scope `episode`, with show name, poster, and episode name snapshotted; none when it did not | full `ShowDetail` either way |
| `POST /shows/{id}/bulk-watch` → `bulk_set_watched` (queries.rs:838-952) | `watched != ?` and aired | yes: only rows whose state differs are touched, so `watched_at` on already-watched episodes is preserved | one row only if ≥1 episode changed, scope `show` / `season` / `through_episode`, `episode_count` = rows changed | full `ShowDetail` |

Bulk scopes: `all` (whole show), `season` (one season), `through_episode` (every earlier season plus the named season up to and including the named episode). The request body is a tagged object: `{"scope": {"type": "season", "season_number": 2}, "watched": true}` (shows.rs:80-97). Both paths snapshot names inside the transaction so the log row stays meaningful after the show is removed, and both rely on transaction drop for rollback when the target row is missing.

## Pages

**Watchlist** (frontend/src/pages/Watchlist.tsx) is read-only: a card grid, each card linking to the detail page, showing poster, name, the `watched/aired` progress chip, the status pill, and `Next airs <date>`. States: loading, `Error: …`, and an empty state linking to Search. `total_count` and `in_production` are received but not shown.

**ShowDetail** (frontend/src/pages/ShowDetail.tsx):

- A non-numeric route id short-circuits to `Invalid show id` (:18-22).
- The header chip sums `seasons[].watched_count` over `seasons[].episode_count` — total episodes including unaired — so it can disagree with the watchlist chip for the same show (:94-101).
- `allWatched` and `seasonAllWatched` compare against total episode counts (:102, :167-169); a show with unaired episodes can never flip the header button to "Mark all unwatched". The intended rule is aired-only (`SHOWS-UI-006`, `SHOWS-UI-013`).
- Seasons start collapsed; the toggle carries `aria-expanded`; rows show `SxxExx`, name or `—`, air date, a checkbox labelled `Mark S{s}E{e} watched`, and a `Mark through here` button that always sends `watched: true`.
- One `mutating` flag disables every checkbox and bulk button while any mutation is in flight; each response replaces the whole `show` object (no optimistic update).
- Remove asks for confirmation, calls DELETE, and navigates to `/` (Up Next). Failures render a banner above the header with the show still visible.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| What "adding" fetches | Whole season/episode tree once, then nightly resync | Fetch seasons lazily on first open | The watchlist needs per-show progress counts immediately; one upfront cost per add (README.md:249-250). |
| Season 0 | Skipped on add and resync | Store Specials | Specials would pollute progress counts (comment at shows.rs:35). `[inferred]` beyond that comment. |
| Add atomicity | All TMDB fetches first, then one transaction | Insert show then stream seasons | No partial shows on the watchlist; doc comment at queries.rs:29. |
| List bound | `MAX_LIST_ROWS = 500`, no pagination | Cursor pagination | Unauthenticated callers can grow the table; a hard bound caps response size and query count (queries.rs:16-26). |
| Bulk actions | Aired episodes only | Any episode in scope | Accidental marks must not apply to future airings (queries.rs:833-837, CLAUDE.md). |
| Single toggle | No air-date filter | Same filter as bulk | The checkbox is the explicit escape hatch (queries.rs:743-745). |
| Change detection | `watched != ?` guard on both the single toggle and bulk | Unconditional UPDATE | Returned count and log rows reflect real changes and `watched_at` is preserved; a stale client (second tab, double-click) re-sending the current state is a no-op rather than a duplicate history row. Existence is checked separately so a missing episode is still a 404. |
| Watch log timing | Inserted inside the mutation's transaction | Fire-and-forget after commit | A log row is committed atomically with the change it describes (backend/src/db/watch_log.rs:1-2). |
| "Today" | Configured `TIMEZONE`, default America/New_York | Server UTC; browser-local | Aired means aired where the user lives (CLAUDE.md, env.example:14). |
| Progress format | `watched/aired`, no percentage | Percentage bar | CLAUDE.md:99; aired is the denominator that can actually change. |
| Mutation response | Full `ShowDetail` | Changed episode or count only | `[inferred]` Lets the page replace state wholesale without client-side merging. |
| Dates in SQL | `TEXT` compared bytewise | Julian day or epoch columns | `[inferred]` Simple and sufficient while inputs are zero-padded; the hazard is documented at backend/src/api/calendar.rs:44-48. |
| Providers and networks | JSON text columns on `shows` | Normalized join tables | `[inferred]` Read-only lists displayed as pills; no querying by provider. |
| Where mutations live | Detail page only; Watchlist is read-only | Quick actions on cards | `[inferred]` |
| After remove | Navigate to `/` (Up Next) | Back to Watchlist | `[inferred]` Up Next is the home route. |

## Open Questions & Future Decisions

### Resolved
1. ✅ **Progress is aired-based everywhere.** The detail header chip and the "Mark all unwatched" flip use aired-only counts, matching the Watchlist. The page currently sums total episodes (ShowDetail.tsx:94-102); tracked as `SHOWS-UI-006` and `SHOWS-UI-013`. Supplying aired counts to the detail page is an API change for this segment to design.

### Deferred
1. **Per-season `watched_count` on detail** counts watched episodes regardless of air date (queries.rs:245), unlike the watchlist count.
2. **Query fan-out.** `list_watchlist` runs 1 + 2N statements and `get_show_detail` 2 + S. Acceptable at 500-row bound, or worth collapsing into joins?
3. **Duplicate-add race.** Check-then-insert (shows.rs:26, queries.rs:43) can surface a primary-key violation as 500 rather than 400.
4. **Add under the request timeout.** N+1 serial TMDB calls for an N-season show all run inside the perimeter's 30 s timeout; the response is cut off but the fetches are not cancelled.
5. **`seasons.episode_count` semantics.** Derived from fetched episodes (queries.rs:81), not TMDB's summary count.
6. **`networks_json`** is absent from `ShowRow` and `ShowDetail`; only Up Next surfaces it. Should detail show networks too?
7. **Impossible-state errors** use `AppError::Config("show vanished after insert")` (shows.rs:52), whose text reaches the client as a 500 body.
8. **Global `mutating` lock** disables every control on the page during one checkbox toggle.
9. **Year range** renders `2024–2024` when first and last air dates differ within one year (ShowDetail.tsx:128-131).
10. **Timezone coverage.** Every backend test uses UTC; the midnight boundary in the configured zone is untested.
11. **Stale doc comment** at queries.rs:30 names a parameter `season_episodes` that does not exist.
12. **Cascade from resync.** Resync upserts never delete episodes TMDB has dropped; stale rows stay on the tree and in progress counts (owned by `resync`, noted here because the counts are this segment's).

## References

- backend/src/api/shows.rs, backend/src/api/episodes.rs
- backend/src/db/queries.rs:1-282 (add, exists, delete, list, detail), :445-490 (counts, next air date), :744-940 (watched mutations)
- backend/src/models/show.rs, backend/src/state.rs (`today_in`)
- backend/src/db/migrations/20260509000000_initial.sql, 20260808000000_drop_notifications.sql, 20260921000000_add_show_networks.sql
- frontend/src/pages/Watchlist.tsx, frontend/src/pages/ShowDetail.tsx
- backend/tests/db.rs, backend/tests/api.rs (show, episode, bulk tests); frontend/src/pages/Watchlist.test.tsx, ShowDetail.test.tsx
- Consumed: `tmdb` (`get_show`, `get_season`, provider/network extraction), `watch-log` (`insert_entry`, row shape)
- Consumers: `search` (`tracked_tmdb_ids_in`, add endpoint), `up-next-calendar` (`today_in`, `networks_json`), `resync` (upserts into these tables)
