# Showrunner — TV & Movie Tracking

## Overview

Containerized web app for tracking TV shows watched across cable and streaming providers, plus a lightweight movie to-watch list. Rust/Axum backend with SQLite, React 19 SPA frontend. Sources show metadata from TMDB. Designed to run in a homelab via Docker Compose.

## Tech Stack

- Backend: Rust + Axum + sqlx (SQLite)
- Frontend: React 19 + TypeScript + Vite
- Database: SQLite (volume-mounted)
- External Data: TMDB API
- Async Runtime: Tokio + tokio-cron-scheduler
- Deployment: Docker Compose (single app container, SQLite file in named volume)

## Commands

### Backend
- `cd backend && cargo build` — build
- `cd backend && cargo test` — tests
- `cd backend && cargo fmt` — format
- `cd backend && cargo clippy` — lint
- `cd backend && cargo run` — run API server

### Frontend
- `cd frontend && npm install`
- `cd frontend && npm run dev` — dev server (5173) with API proxy
- `cd frontend && npm run build` — production build
- `cd frontend && npx tsc --noEmit` — typecheck
- `cd frontend && npm run lint` — lint + format check (Biome)
- `cd frontend && npm run format` — auto-format (Biome)

### Docker
- `docker compose up -d` — start (port 3001)
- `docker compose down`
- `docker compose build`

## Architecture

```
showrunner/
├── backend/
│   └── src/
│       ├── main.rs              # Axum server, static file serving, scheduler bootstrap
│       ├── config.rs            # Env-var-based configuration
│       ├── error.rs             # Error types with HTTP responses
│       ├── state.rs             # AppState (pool, tmdb client, tz) + today_in helper
│       ├── api/                 # Route handlers (one file per resource)
│       ├── db/                  # SQLite pool, queries, migrations
│       ├── models/              # Data structures
│       ├── logic/               # resync (the scheduler body)
│       ├── datasources/         # TMDB client
│       └── scheduler.rs         # tokio-cron-scheduler (resync)
├── frontend/
│   └── src/
│       ├── App.tsx              # React Router
│       ├── api/client.ts        # Fetch wrapper
│       ├── types/index.ts       # TS interfaces matching Rust models
│       ├── pages/               # Watchlist, UpNext, ShowDetail, Search, Calendar, History, Settings
│       └── components/
├── Dockerfile                   # Multi-stage: Node → Rust → slim runtime
└── docker-compose.yml           # app + sqlite volume
```

## API Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | /api/v1/health | Connection status |
| GET | /api/v1/search?q= | TMDB `search/multi` proxy for TV + movies (with `already_tracked` per result) |
| GET | /api/v1/shows | List watchlist with progress |
| POST | /api/v1/shows | Add show by TMDB id |
| GET | /api/v1/shows/:tmdb_id | Show detail with seasons/episodes |
| DELETE | /api/v1/shows/:tmdb_id | Remove show from watchlist |
| POST | /api/v1/shows/:tmdb_id/bulk-watch | Bulk mark (`scope`: `all` / `season` / `through_episode`) |
| PATCH | /api/v1/episodes/:show/:season/:ep | Toggle single episode watched |
| GET | /api/v1/movies | List movie to-watch list |
| POST | /api/v1/movies | Add movie by TMDB id |
| GET | /api/v1/movies/:tmdb_id | Movie detail (cast, directors, providers) |
| DELETE | /api/v1/movies/:tmdb_id | Remove movie (not logged) |
| POST | /api/v1/movies/:tmdb_id/watched | Mark movie watched (deletes row + writes watch log) |
| GET | /api/v1/watch-log?page=&per_page= | Paginated watched/unwatched history, newest first |
| GET | /api/v1/up-next | Per-show earliest unwatched aired episode |
| GET | /api/v1/calendar?start=&end= | Episodes airing in date range |
| POST | /api/v1/sync | Force TMDB resync (returns per-show success/failure) |

## Key Patterns

- **TMDB on-demand** — search proxied through backend (API key never leaves server). Adding a show fetches full season/episode tree once; nightly resync refreshes.
- **Movies are a separate, simpler track** — a flat to-watch list (`movies` table, no episodes/seasons). Search (`search/multi`) returns both TV and movies; adding a movie stores basic metadata. "Mark watched" is `POST /movies/:tmdb_id/watched` and "remove" is `DELETE /movies/:tmdb_id` — both delete the row (there's no watched-movie state), but only the former writes a watch-log entry. Movies are intentionally absent from resync, the calendar, and Up Next, which are all episode-driven.
- **Watch log (History page)** — append-only `watch_log` table written in the same transaction as every watched/unwatched mutation (`set_episode_watched`, `bulk_set_watched`, `mark_movie_watched` in `db/queries.rs`; inserts live in `db/watch_log.rs`). Rows snapshot title/poster and have no foreign keys so history survives removing a show or movie. Bulk actions log one row with `scope` (`show`/`season`/`through_episode`) and `episode_count` = episodes actually changed (the bulk UPDATEs filter `watched != ?`, which also preserves `watched_at` on already-watched episodes). `GET /watch-log` is offset-paginated, newest first, `per_page` capped at 100. The page is read-only by design — undo happens on the show/movie pages.
- **Secrets never reach clients or logs** — outbound TMDB URLs carry the `api_key` query param. `From<reqwest::Error>` strips the URL via `.without_url()`, and `AppError::client_message()` collapses internal variants (DB/HTTP/IO/JSON) to a generic string wherever an error is serialized into a response body (`IntoResponse`, and the per-item results of `/sync`).
- **Resync preserves user state** — `upsert_episode_preserving_watched` uses `ON CONFLICT … DO UPDATE` that writes the new TMDB metadata but **never** touches `watched`/`watched_at`.
- **Bulk-watch scopes** — three actions: mark whole show, mark season N, mark through episode SxEy. All filter to aired episodes (`air_date <= today`) so accidental marks don't apply to future airings. Single-episode `PATCH` does no filtering — escape hatch.
- **Configurable schedule** — `RESYNC_CRON` (cron expr, fires in `TIMEZONE`) drives `tokio-cron-scheduler`.
- **Local timezone for "today"** — `Config.timezone` (default `America/New_York`, override via `TIMEZONE`) is plumbed through `AppState.tz` to all date-comparison queries (`list_watchlist`, `bulk_set_watched`, `list_up_next`) and to the cron scheduler. Stored timestamps (`watched_at`, `last_synced_at`) remain UTC RFC3339.
- **Calendar** — full month grid, episodes shown by air date. Watched episodes get faded styling.
- **Up Next** — per-show earliest unwatched aired episode, sorted by air_date ASC (longest-overdue first). SQL uses `ROW_NUMBER() OVER (PARTITION BY show ORDER BY air_date)`. Each row shows the show's networks as pills (TMDB `networks` — broadcast/cable channels and streaming originals alike — stored in `shows.networks_json`, written on add and refreshed by resync) so it's obvious where to go watch. Watch providers are deliberately not shown here: TMDB's provider list is too noisy to read at a glance. Each row also carries an `N remaining` pill counting **aired** unwatched episodes only (same `air_date <= today` filter, never future airings); it switches to the indigo `status-pill-accent` style when N > 1 so being several weeks behind is obvious.
- **Progress display** — `12/27` format (watched / aired), no percentage.
- Axum serves React SPA static files with fallback to index.html for client-side routing.

## Environment Variables

See `env.example` for the full list. Required: `TMDB_API_KEY`. Optional: `TIMEZONE` (default `America/New_York`, IANA name), `RESYNC_CRON`, `CORS_ALLOWED_ORIGIN`, `RUST_LOG`, `SERVER_HOST`, `SERVER_PORT`, `DATABASE_URL`.

## LID
- Mode: Full
- Version: 1.3.0

## LID Tooling

- **Coherence check**: `scripts/coherence-check.mjs` — `node scripts/coherence-check.mjs` reports @spec integrity, citation coverage (every `[x]` spec cited in code and by a test or CI gate), arrow reference integrity, and staleness; `--strict` exits non-zero on a reverse orphan or an implemented spec with no code citation. Authoritative for those deterministic checks; LID skills invoke it instead of auditing in-prompt.

## Linked-Intent Development (MANDATORY)

**Consult the `linked-intent-dev` skill for ALL code changes.** All changes flow through the arrow of intent in one direction:

```
HLD → LLDs → EARS → Tests → Code
```

- **New features and refactors**: full six-phase workflow (HLD check → LLD check/draft → EARS → intent-narrowing edge audit → tests-first → code).
- **Bug fixes**: walk the arrow like any other change — find where behavior diverged from intent and cascade from there. No short-circuit.
- **If unsure**: use the full workflow.

Stop after each phase for user review. **Docs carry current intent, written to be read cold** — write each doc as if authored fresh today, from current intent alone: no narration of how it changed, no meaning that needs the conversation that produced it, no rebuttals to questions only a past discussion raised. Rationale, considered alternatives, and constraints a fresh author would independently write stay; record rejected alternatives and why in the LLD's Decisions & Alternatives table, not as asides in body prose.

**Memory vs. intent.** Before saving durable project knowledge to agent or tool memory, test whether it is project *intent* — would a fresh agent, in any tool, next session, need it to build this system correctly? If yes, record it in the arrow (HLD / LLD / EARS / decision doc), which travels and cascades — not in private, per-tool memory, where intent escapes the arrow. Knowledge about the user or how they like to work stays in memory.

### Navigation

| What you need | Where to look |
|---|---|
| High-level design | `docs/high-level-design.md` |
| Design tree (sub-HLDs, LLDs, their specs) | `docs/intent/` — one folder per node |
| EARS specs | beside each design doc as `{node}-specs.md` in the node's folder under `docs/intent/` |
| Decision docs | `docs/decisions/` (project-level) and `docs/intent/<segment>/decisions/` |
| Arrow of intent overlay | `docs/arrows/index.yaml` and per-segment docs in `docs/arrows/` |

### Terminology

- **HLD**: High-Level Design — single project-level doc at `docs/high-level-design.md`.
- **LLD**: Low-Level Design — detailed component design doc in `docs/intent/`. The design layer is a recursive tree: the root is the HLD, leaf LLDs own EARS, and a component deep enough to outgrow one doc becomes a sub-HLD (HLD-shaped, owns no EARS) with children beneath it. "HLD" and "LLD" are roles by position; depth-2 (one HLD over flat leaf LLDs) is the default.
- **EARS**: Easy Approach to Requirements Syntax — structured one-line requirements beside each design doc as `{node}-specs.md` in the node's folder under `docs/intent/`. IDs are path-concatenated — the root-to-leaf path of the owning segment plus a number — so a prefix grep gathers a subtree. Markers: `[x]` implemented, `[ ]` active gap, `[D]` deferred.
- **Arrow**: the unidirectional chain from vision to code (HLD → LLDs → EARS → Tests → Code). Strictly a DAG of intent.
- **Arrow segment**: the territory owned by one leaf LLD — the LLD itself plus the specs, tests, and code that cite its EARS IDs. The boundary is the leaf prefix. Within-segment cascade is free; across-segment cascade pauses.
- **Cascade**: propagating a change downstream through the arrow so adjacent levels stay coherent.

### Code annotations

Annotate code and tests with `@spec` comments citing EARS IDs:

```
// @spec SHOWS-WATCHED-004, SHOWS-WATCHED-005
```

Place the annotation at the *entry point of the behavior's implementation graph* — the topmost function or module owning the specified behavior, not every helper. When a behavior spans multiple subsystems (UI + API + database, for example), annotate at the entry point in each subsystem. Tests follow the same rule: annotate the test that directly exercises the spec, not every inner assertion.
