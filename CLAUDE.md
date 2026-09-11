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
- **Up Next** — per-show earliest unwatched aired episode, sorted by air_date ASC (longest-overdue first). SQL uses `ROW_NUMBER() OVER (PARTITION BY show ORDER BY air_date)`.
- **Progress display** — `12/27` format (watched / aired), no percentage.
- Axum serves React SPA static files with fallback to index.html for client-side routing.

## Environment Variables

See `env.example` for the full list. Required: `TMDB_API_KEY`. Optional: `TIMEZONE` (default `America/New_York`, IANA name), `RESYNC_CRON`, `CORS_ALLOWED_ORIGIN`, `RUST_LOG`, `SERVER_HOST`, `SERVER_PORT`, `DATABASE_URL`.
