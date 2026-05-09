# Showrunner

A self-hosted TV show tracker for the streaming era. Search shows, build a watchlist, mark episodes watched (one at a time or in bulk), see what's airing on a calendar, get a "what should I watch next" list, and optionally fire Slack notifications when new episodes drop.

Designed for a homelab. Single Docker container, SQLite on a volume, no auth.

---

## Features

- **Search** — find shows via TMDB; results show whether you're already tracking them
- **Watchlist** — at-a-glance grid with progress (`12/27`), status, and next-air-date per show
- **Show detail** — collapsible season/episode list with watched checkboxes, "where to watch" providers, bulk-mark controls (whole show / season / through-this-episode)
- **Up Next** — quick list of the next unwatched episode for each show, sorted longest-overdue first
- **Calendar** — full month grid of episodes airing on each day; click an episode to jump to the show
- **Notifications** — optional Slack webhook fires when a watchlisted episode airs today; deduped so restarts don't re-fire
- **Auto-resync** — daily TMDB refresh on a configurable cron; new episodes appear automatically; user state (watched/watched_at) is preserved
- **Manual sync + test notification** — buttons in the Settings page

---

## Quick start (Docker)

You need:

- A free **TMDB API key** — sign up at https://www.themoviedb.org/settings/api (use the **API Read Access Token (v3 auth)**)
- Docker + Docker Compose

```bash
git clone <this-repo> showrunner
cd showrunner

# Seed env file (the hook on macOS blocks writing .env directly,
# so we ship the template as `env.example` — copy it locally)
cp env.example .env
# Edit .env and set at minimum:
#   TMDB_API_KEY=your_key_here

docker compose up -d --build
```

Then open **http://localhost:3001**.

Watch the logs:

```bash
docker compose logs -f app
```

Stop it:

```bash
docker compose down
```

The SQLite database lives in the `showrunner_data` Docker volume — it survives `down` / `up` cycles and image rebuilds.

---

## Local development

Two terminals.

**Terminal 1 — backend** (Rust, serves the API on port 3001):

```bash
cd backend
cp ../env.example .env
# Edit backend/.env: set TMDB_API_KEY,
# and change DATABASE_URL so it doesn't point at /data:
#   DATABASE_URL=sqlite://./showrunner.db
cargo run
```

**Terminal 2 — frontend** (Vite dev server with HMR, proxies `/api` to 3001):

```bash
cd frontend
npm install   # first time only
npm run dev
```

Then open **http://localhost:5173**.

---

## Configuration

All configuration is via environment variables. See `env.example` for the full list with defaults.

| Var | Default | Notes |
|---|---|---|
| `TMDB_API_KEY` | _(required)_ | TMDB v3 API key |
| `SLACK_WEBHOOK_URL` | _(blank = disabled)_ | Incoming-webhook URL; if set, sends a message when a watchlisted episode airs today |
| `TIMEZONE` | `America/New_York` | IANA zone name. Used for "today" comparisons and the resync cron expression |
| `RESYNC_CRON` | `0 0 6 * * *` | 6-field cron (sec min hour dom mon dow). Fires in `TIMEZONE`. Default = 6:00 AM local |
| `NOTIFICATION_CHECK_INTERVAL_MINUTES` | `60` | How often to scan for episodes airing today |
| `SERVER_HOST` | `0.0.0.0` | |
| `SERVER_PORT` | `3001` | |
| `DATABASE_URL` | `sqlite:///data/showrunner.db` | Path inside the container's volume mount |
| `CORS_ALLOWED_ORIGIN` | _(same-origin)_ | Set to your external origin if serving the SPA from a different host; `*` to allow any |
| `RUST_LOG` | `info` | Standard env-filter syntax (e.g. `showrunner_backend=debug`) |

### Slack setup

1. In Slack: **Apps → Incoming Webhooks → Add to Slack**, pick a channel, copy the webhook URL
2. Set `SLACK_WEBHOOK_URL` in `.env`, restart the container
3. **Settings → Send test notification** to verify

### Changing the schedule

`RESYNC_CRON` uses 6-field cron in your `TIMEZONE`:

| Schedule | Cron |
|---|---|
| 6 AM daily (default) | `0 0 6 * * *` |
| Every 12 hours | `0 0 */12 * * *` |
| Hourly | `0 0 * * * *` |
| Mondays at 4 AM | `0 0 4 * * 1` |

---

## Homelab / reverse proxy

The app listens on `0.0.0.0:3001` and serves both the API (`/api/v1/*`) and the SPA (everything else, with SPA fallback for client-side routing).

Behind Traefik / nginx / Caddy:

- Forward all traffic for the host to `http://showrunner:3001`
- If your reverse proxy is on a different origin than what the browser hits, set `CORS_ALLOWED_ORIGIN` to your external URL (e.g. `https://shows.home.lan`)
- The app doesn't care about path prefixes — mount it at any subpath if your proxy strips it

Example Traefik labels:

```yaml
labels:
  - traefik.enable=true
  - traefik.http.routers.showrunner.rule=Host(`shows.home.lan`)
  - traefik.http.services.showrunner.loadbalancer.server.port=3001
```

---

## Backup

The whole DB is one SQLite file inside the `showrunner_data` volume. To back it up:

```bash
docker compose exec app sqlite3 /data/showrunner.db ".backup '/data/backup.db'"
docker cp $(docker compose ps -q app):/data/backup.db ./showrunner-backup.db
```

Restore by stopping the stack and replacing the file in the volume.

---

## Troubleshooting

**"TMDB_API_KEY is required" on startup**
Set `TMDB_API_KEY` in `.env` and restart. Don't include quotes around the value.

**"TIMEZONE 'X' is not a valid IANA zone"**
Use a name from the [IANA tz database](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones), e.g. `America/Los_Angeles`, `Europe/London`, `Asia/Tokyo`. Not abbreviations like `EST`.

**Adding a long-running show takes 5–10 seconds**
Backend fetches each season's episodes sequentially from TMDB. For a 7-season show that's ~8 calls. This is normal.

**Calendar shows nothing**
Episodes only appear for shows on your watchlist. Add some shows first; resync (Settings → Resync) if recent.

**Notifications not firing**
- Confirm `SLACK_WEBHOOK_URL` is set: check `/api/v1/health`, the `notifiers` array should include `"slack"`
- Use **Settings → Send test notification** to verify connectivity
- Real notifications fire via the interval task — check `docker compose logs app` for `notification check` lines
- The dedupe table `notification_log` prevents re-firing for the same (show, season, episode, channel). If you want to re-test for a real episode, clear it: `DELETE FROM notification_log;`

**`docker compose build` fails with npm lockfile errors**
The Dockerfile uses `npm install` (not `npm ci`) to handle the cross-platform optional-deps issue with newer ESLint plugins. If it still fails, try `docker compose build --no-cache`.

---

## Development conventions

See [`CLAUDE.md`](./CLAUDE.md) for codebase architecture, key patterns (notifier abstraction, resync upsert preserving watched state, timezone handling), and the full API table.

---

## License

Personal project — do whatever you want with it.
