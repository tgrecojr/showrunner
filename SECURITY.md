# Security Policy

## Reporting a vulnerability

**Please do not open a public issue for a security vulnerability.**

Report it privately via [GitHub's private vulnerability reporting](https://github.com/tgrecojr/showrunner/security/advisories/new). That opens a draft advisory only you and the maintainer can see.

Please include what the issue is, how to reproduce it, and what an attacker could achieve. I'll acknowledge within a few days. This is a personal project maintained in spare time, so please be patient — but a real vulnerability will get attention.

## Security model — read this before you deploy

Showrunner **has no authentication**. This is deliberate, not an oversight.

It is designed to run inside a trusted network (a homelab LAN, or behind a reverse proxy / VPN / SSO layer that you control). Every API endpoint is unauthenticated: anyone who can reach the port can read your watchlist, add or remove shows, mark episodes watched, and trigger a TMDB resync.

**Therefore:**

- ✅ **Do** run it on your LAN, on a Tailscale/WireGuard network, or behind an authenticating reverse proxy (Authelia, Authentik, oauth2-proxy, Cloudflare Access).
- ❌ **Do not** expose it directly to the public internet. There is nothing stopping a stranger from using it.

Authentication and multi-user support are a known future design topic. If you need them today, put an auth layer in front of it.

## What the app does protect

Even without auth, the app is built to avoid handing an attacker anything beyond the app's own data:

- **Your TMDB API key never reaches the browser.** All TMDB calls are proxied through the backend; the key lives only in the server's environment.
- **All database access uses bound parameters** via `sqlx` — no SQL is built by string concatenation.
- **The container runs as a non-root user** (uid 65532) on a distroless Chainguard base with no shell and no package manager.

## Supply chain

Published images are built by GitHub Actions and:

- **signed with [cosign](https://docs.sigstore.dev/) (keyless)**,
- ship an **SPDX SBOM attestation**, and
- ship a **SLSA build-provenance attestation**,

all pushed to the registry alongside the image. See [Verifying the image](README.md#verifying-the-image) for how to check them.

Dependencies are scanned in CI (`cargo audit`, `npm audit`, OSV, Socket) and updated automatically by Renovate.

## Handling secrets

`TMDB_API_KEY` is a secret. Keep it in your `.env` file, which is gitignored. Never paste it into an issue, a PR, or a log excerpt — redact it first.
