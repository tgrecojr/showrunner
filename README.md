# Showrunner

**A self-hosted TV & movie tracker for the streaming era.** Search for shows, build a watchlist, mark episodes watched, see what's airing on a calendar, get a "what should I watch next" list, and keep a movie to-watch list.

Built for a homelab: one Docker container, a SQLite file on a volume, no external database, no accounts to manage.

<!-- Add screenshots here — the Watchlist grid and the Calendar page make the strongest first impression. -->

---

## Contents

- [What it does](#what-it-does)
- [Quick start](#quick-start-docker)
- [Verifying the image](#verifying-the-image)
- [Updating](#updating)
- [Local development](#local-development)
- [Configuration](#configuration)
- [Homelab / reverse proxy](#homelab--reverse-proxy)
- [Backup & restore](#backup--restore)
- [Security model](#security-model)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [License](#license)

---

## What it does

- **Search** — find TV shows and movies via [TMDB](https://www.themoviedb.org/); results show whether you're already tracking them.
- **Watchlist** — an at-a-glance grid with progress (`12/27` = watched / aired), status, and next air date per show.
- **Show detail** — collapsible season/episode list with watched checkboxes, "where to watch" providers, and bulk-mark controls (whole show / whole season / through a given episode).
- **Up Next** — the next unwatched episode for each show, sorted longest-overdue first.
- **Calendar** — a full month grid of what airs on each day; click an episode to jump to its show.
- **Movies** — a separate to-watch list for films: add a movie, see its cast, directors, and where to stream, then mark it watched (which removes it from the list). Movies are standalone — they don't appear on the calendar or Up Next, which are episode-based.
- **Auto-resync** — a nightly TMDB refresh (configurable) pulls in newly announced episodes automatically. **Your watched/unwatched state is always preserved.**

Your TMDB API key stays on the server — it's never shipped to the browser.

---

## Quick start (Docker)

You need:

- **Docker** and the **Docker Compose** plugin.
- A free **TMDB API key** — sign up at <https://www.themoviedb.org/settings/api> and copy the **API Read Access Token (v3 auth)**.

A prebuilt, signed image is published to the GitHub Container Registry, so you don't need to clone the repo or build anything. Create a folder with two files:

**`docker-compose.yml`**

```yaml
services:
  app:
    image: ghcr.io/tgrecojr/showrunner:latest
    restart: unless-stopped
    ports:
      - "3001:3001"
    env_file: .env
    volumes:
      - showrunner_data:/data

volumes:
  showrunner_data:
```

**`.env`** (only `TMDB_API_KEY` is required — see [Configuration](#configuration) for the rest)

```dotenv
TMDB_API_KEY=your_key_here
TIMEZONE=America/New_York
```

Then:

```bash
docker compose up -d
docker compose logs -f app     # watch it start
```

Open **<http://localhost:3001>** and start adding shows.

The SQLite database lives in the `showrunner_data` volume — it survives `docker compose down` / `up` and image updates. To stop the app: `docker compose down`.

> **Prefer to build from source?** Clone this repo, copy `env.example` to `.env` and set `TMDB_API_KEY`, then run `docker compose up -d --build`. The bundled `docker-compose.yml` builds the image locally.

---

## Verifying the image

Every published image is signed with [cosign](https://docs.sigstore.dev/) (keyless) and ships with an SPDX **SBOM** and a **SLSA build-provenance** attestation. You can verify all of it before running anything.

**Signature** (identity is the GitHub Actions workflow that built it):

```bash
cosign verify ghcr.io/tgrecojr/showrunner:latest \
  --certificate-identity-regexp 'https://github.com/tgrecojr/showrunner/.*' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

**Build provenance and SBOM** (with the GitHub CLI):

```bash
gh attestation verify oci://ghcr.io/tgrecojr/showrunner:latest --owner tgrecojr
```

---

## Updating

```bash
docker compose pull      # fetch the newest image
docker compose up -d     # recreate the container
```

Your data is untouched — it lives in the `showrunner_data` volume, not in the image. Database schema migrations run automatically on startup.

To pin a specific version instead of tracking `latest`, set the tag in `docker-compose.yml` (e.g. `ghcr.io/tgrecojr/showrunner:0.1`). Available tags are on the [package page](https://github.com/tgrecojr/showrunner/pkgs/container/showrunner).

---

## Local development

Two terminals.

**Terminal 1 — backend** (Rust, serves the API on port 3001):

```bash
cd backend
cp ../env.example .env
# In backend/.env: set TMDB_API_KEY, and point the DB at a local file:
#   DATABASE_URL=sqlite://./showrunner.db
cargo run
```

**Terminal 2 — frontend** (Vite dev server with hot reload, proxies `/api` to 3001):

```bash
cd frontend
npm install      # first time only
npm run dev
```

Open **<http://localhost:5173>**.

Before pushing, run the same checks CI runs — see [CONTRIBUTING.md](CONTRIBUTING.md#before-you-push).

---

## Configuration

Everything is configured with environment variables. `env.example` in this repo lists them all with defaults.

| Variable | Default | Notes |
|---|---|---|
| `TMDB_API_KEY` | _(required)_ | TMDB v3 API key. No quotes around the value. |
| `TIMEZONE` | `America/New_York` | IANA zone name (e.g. `Europe/London`). Drives "today" comparisons and the resync cron. |
| `RESYNC_CRON` | `0 0 6 * * *` | 6-field cron (`sec min hour dom mon dow`), interpreted in `TIMEZONE`. Default = 6:00 AM local. |
| `SERVER_HOST` | `0.0.0.0` | Bind address. |
| `SERVER_PORT` | `3001` | Bind port. |
| `DATABASE_URL` | `sqlite:///data/showrunner.db` | SQLite path. In Docker this points at the mounted volume. |
| `DB_MAX_CONNECTIONS` | `5` | SQLite connection pool size. Rarely needs changing. |
| `CORS_ALLOWED_ORIGIN` | _(same-origin)_ | Set to your external origin if the SPA is served from a different host; `*` allows any origin. |
| `RUST_LOG` | `info` | `env_logger`/`tracing` filter syntax, e.g. `showrunner_backend=debug`. |

### Changing the schedule

`RESYNC_CRON` is 6-field cron, interpreted in your `TIMEZONE`:

| Schedule | Cron |
|---|---|
| 6 AM daily (default) | `0 0 6 * * *` |
| Every 12 hours | `0 0 */12 * * *` |
| Hourly | `0 0 * * * *` |
| Mondays at 4 AM | `0 0 4 * * 1` |

---

## Homelab / reverse proxy

The app listens on `0.0.0.0:3001` and serves both the API (`/api/v1/*`) and the React SPA (everything else, with SPA fallback for client-side routing).

Behind Traefik / nginx / Caddy:

- Forward the host's traffic to `http://showrunner:3001`.
- If your proxy terminates on a different origin than the browser hits, set `CORS_ALLOWED_ORIGIN` to that external URL (e.g. `https://shows.home.lan`).
- Path prefixes are fine — mount it at any subpath your proxy strips.

Example Traefik labels:

```yaml
labels:
  - traefik.enable=true
  - traefik.http.routers.showrunner.rule=Host(`shows.home.lan`)
  - traefik.http.services.showrunner.loadbalancer.server.port=3001
```

> ⚠️ **Do not expose Showrunner directly to the public internet.** It has no authentication by design — see [Security model](#security-model).

---

## Backup & restore

The entire database is one SQLite file in the `showrunner_data` volume. Back it up with SQLite's online backup (safe while the app is running):

```bash
docker compose exec app sqlite3 /data/showrunner.db ".backup '/data/backup.db'"
docker cp "$(docker compose ps -q app):/data/backup.db" ./showrunner-backup.db
```

To restore, stop the stack and put the file back into the volume before starting again.

---

## Security model

**Showrunner has no authentication, and that is intentional.** It is meant to run on a trusted network (a homelab LAN, a Tailscale/WireGuard mesh, or behind an authenticating reverse proxy such as Authelia, Authentik, or Cloudflare Access). Anyone who can reach the port has full control of the app's data.

- ✅ Run it on your LAN, over a private VPN, or behind an SSO/auth proxy.
- ❌ Never publish it straight to the internet.

What the app *does* protect, even without auth: your TMDB API key never leaves the server, all database access uses bound parameters, and the container runs as a non-root user on a distroless base.

Full details — including how to report a vulnerability privately — are in [SECURITY.md](SECURITY.md).

---

## Troubleshooting

**`TMDB_API_KEY is required` on startup**
Set `TMDB_API_KEY` in `.env` and restart. Don't wrap the value in quotes.

**`TIMEZONE 'X' is not a valid IANA zone`**
Use a full name from the [IANA tz database](https://en.wikipedia.org/wiki/List_of_tz_database_time_zones) — e.g. `America/Los_Angeles`, `Europe/London`, `Asia/Tokyo` — not an abbreviation like `EST`.

**Adding a long-running show takes 5–10 seconds**
The backend fetches each season's episodes from TMDB one at a time. For a 7-season show that's ~8 calls. This is normal, and only happens once per add.

**The calendar is empty**
Episodes only appear for shows on your watchlist. Add some shows first, then resync from **Settings → Resync** if you added them recently.

---

## Contributing

Contributions are welcome. **Please open an issue to discuss a change before opening a pull request** — it saves you from building something that turns out to be out of scope or already in progress.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide, including the checks to run before you push.

Architecture notes, key patterns, and the full API table live in [CLAUDE.md](CLAUDE.md).

---

## License

Released under the [MIT License](LICENSE) — free to use, modify, and distribute, including commercially. No warranty.
