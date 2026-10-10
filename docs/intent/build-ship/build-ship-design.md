---
parent: high-level-design
prefix: SHIP
---

# Build and ship

## Context and Design Philosophy

Showrunner ships as one container image. This segment owns how that image is built reproducibly from committed lockfiles, how small and unprivileged its runtime is, which checks gate a merge, how dependencies are scanned and kept current, and how a published image is signed and attested so a homelab operator can verify what they pull. The guiding posture is supply-chain conservatism: install exactly what was audited, run nothing at install time, pin every base image and action by digest, and let Renovate move the pins.

## Image

`Dockerfile` has three stages:

1. **`frontend-build`** on `node:24-trixie-slim` (digest-pinned). Copies `package.json` and `package-lock.json`, runs `npm ci --ignore-scripts --no-audit --no-fund`, then `npm run build`. glibc Debian rather than Alpine so the lockfile resolves to the same tree CI audited (the comment above the `frontend-build` stage's `FROM`); `--ignore-scripts` because the toolchain ships platform binaries, not install hooks (the comment above its `RUN npm ci`).
2. **`backend-build`** on `rust:1.98-slim-trixie` (digest-pinned). Copies `Cargo.toml` and `Cargo.lock` without a glob so a missing lockfile fails the build (`COPY backend/Cargo.toml backend/Cargo.lock`), warms a dependency layer with a placeholder `main.rs`, then `cargo build --release --locked`. It also pre-creates `/rootfs/data` owned by uid 65532, because the runtime has no shell to `mkdir` or `chown` (the `backend-build` stage's `RUN mkdir -p /rootfs/data` instruction).
3. **Runtime** on `cgr.dev/chainguard/glibc-dynamic:latest` (digest-pinned). No shell, no package manager, no libssl; TLS is rustls using the image's CA bundle (the comment above the runtime stage's `FROM`). It receives only the binary, the built `dist/` as `/app/static`, and `/data`, all owned by 65532, and runs as that uid by image default. `ENV STATIC_DIR=/app/static`, `EXPOSE 3001`, entrypoint is the binary. `HEALTHCHECK` runs the binary's own `--healthcheck` mode (`app`) every 30 s with a 5 s timeout, a 10 s start period, and 3 retries, so the container reports `unhealthy` once the database probe has failed three times in a row.

`docker-compose.yml` builds locally, restarts unless stopped, publishes `3001:3001`, mounts the named volume `showrunner_data` at `/data`, and passes `SERVER_HOST`, `SERVER_PORT`, `DATABASE_URL`, `TMDB_API_KEY` (required: compose refuses to start when it is unset or empty, before the container exists), `RESYNC_CRON`, `TIMEZONE`, `CORS_ALLOWED_ORIGIN`, and `RUST_LOG` with defaults, plus a hard-coded `STATIC_DIR`. `DB_MAX_CONNECTIONS` is not passed. The compose file declares no `healthcheck` of its own and inherits the image's; `docker compose ps` shows the state. Docker does not restart a container for being unhealthy, so the status is for operators and orchestrators to act on. `.dockerignore` keeps `.git`, dotenv files, build outputs, `node_modules`, `dist`, Markdown, `.claude/`, `data/`, and `*.db` out of the context.

## Merge gates (`ci.yml`)

On push to `main` and on pull requests, two jobs under `permissions: contents: read`:

| Job | Steps |
|---|---|
| Backend (Rust) | stable toolchain with clippy, rustfmt, llvm-tools; `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo llvm-cov --lcov --fail-under-lines 85`; upload `lcov.info` |
| Frontend | Node 24; `npm ci`; `npx tsc --noEmit`; `npx biome ci .`; `npm run test:coverage` (vitest thresholds 85/85/85/80 from `vitest.config.ts`); upload `lcov.info` |

A third job runs only on the Monday 13:00 UTC schedule and calls the supply-chain workflow in absolute mode, so an advisory that lands against already-merged dependencies surfaces weekly (the `schedule` trigger and the `supply-chain` job). Every action is SHA-pinned with a version comment, maintained by Renovate's `helpers:pinGitHubActionDigests`.

## Supply-chain scan (`supply-chain.yml`)

A reusable `workflow_call` job with an optional `SOCKET_SECURITY_API_KEY` secret, optional so bot-authored PRs, which cannot read secrets, still run the rest (the `workflow_call` secret's `required: false` declaration). Steps: `npm ci --ignore-scripts`; `npm audit signatures` (registry signature verification, not a vulnerability audit); a Socket Security scan of `frontend/` when the key is present, using an exact-pinned CLI version because `@latest` would execute arbitrary code in a job holding the key (step `Socket Security scan (frontend)`); OSV scanning in two modes; `cargo audit`.

OSV runs in **diff mode** on pull requests: the base branch's lockfiles are fetched into `.osv-base/`, both old and new lockfile sets are scanned with `continue-on-error`, a check refuses to compare if either scan wrote no results, and the reporter fails only on vulnerabilities the PR introduces (the `(PR diff mode)` steps through `Report vulnerabilities introduced by this PR`). On pushes to `main` and on the schedule it runs in **absolute mode** over the tree and fails on any known vulnerability.

## Publish (`docker-publish.yml`)

On push to `main`, on `v*` tags, and on pull requests, under a workflow-level `permissions: contents: read` that the publish job alone widens (packages, id-token, attestations, artifact-metadata): the supply-chain scan runs first, then `build-and-push`. Pull requests build only. Pushes log in to GHCR with the workflow token, derive tags (`latest` on the default branch, semver `X.Y.Z` and `X.Y` from `v*` tags, `sha-<short>` always), build with GitHub Actions cache, push, then `cosign sign --yes` keyless against the digest, generate an SPDX SBOM from the pushed digest, and attach SBOM and SLSA build-provenance attestations to the registry. The build is single-platform (the runner's `linux/amd64`). The publish job depends on the scan but not on `ci.yml`.

## Retention and dependency updates

`ghcr-retention.yml` runs Mondays 06:00 UTC and on demand, with `packages: write`, keeping `latest` plus the five most recent tagged versions and deleting untagged manifests; it relies on the cleanup action being referrer-aware so signatures and attestations of kept images survive (step `Prune GHCR (keep latest + 5 most recent tagged)`). Because every `main` push adds a `sha-*` tag, older semver tags fall out of the five.

`renovate.json`: digest-pins Docker images and GitHub Actions; groups all Docker base images into one PR so the Rust builder's glibc cannot get ahead of the runtime's (the rule described `Keep all Docker base images … in one PR`); groups minor and patch updates into one PR with a 3-day release age; digest updates after 1 day; majors are never auto-merged and get a `major-update` label; vulnerability alerts are labelled `security`; lockfile maintenance runs weekly. Runs on a night-and-weekend schedule in America/New_York with `platformAutomerge`, which depends on branch protection requiring the gates above.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| Builder base | Debian-slim glibc images | Alpine/musl | The committed lockfile is resolved against glibc in CI; building on the same libc installs exactly the audited tree (Dockerfile, the comment above the `frontend-build` stage's `FROM`). |
| Install discipline | `npm ci --ignore-scripts`; `cargo build --locked` with the lockfile required | `npm install`; tolerate a missing lockfile | Install only what was audited; run no lifecycle scripts; fail rather than resolve fresh (Dockerfile, the comment above `frontend-build`'s `RUN npm ci`, and `COPY backend/Cargo.toml backend/Cargo.lock`). |
| Runtime base | Chainguard `glibc-dynamic`, uid 65532, rustls | Debian slim; Alpine; scratch | No shell or package manager to abuse, no libssl to patch; the image's CA bundle serves rustls (Dockerfile, the comment above the runtime stage's `FROM`; SECURITY.md, "What the app does protect"). |
| Pinning | Every image and action by digest, moved by Renovate | Floating tags | Reproducible builds; updates arrive as reviewed PRs (renovate.json, the `helpers:pinGitHubActionDigests` preset and the `matchCategories: ["docker"]` rule's `pinDigests`). |
| Base-image updates | All Docker bases in one PR | Independent PRs | Builder and runtime glibc must move together (renovate.json, the rule described `Keep all Docker base images … in one PR`). |
| What gates a merge | fmt, clippy `-D warnings`, 85% backend line coverage, tsc, Biome, vitest thresholds | Lint only | CONTRIBUTING.md, "Before you push"; the PR template restates the same commands. |
| What gates a publish | The supply-chain scan | Also the CI lint/test jobs | `[inferred]` Branch protection on PR merges is relied on to keep untested code off `main`. |
| OSV on PRs | Diff mode, fail only on introduced vulnerabilities | Absolute on every PR | An advisory against a dependency already on `main` must not block unrelated PRs (supply-chain.yml, the `OSV scanning` comment above the `(PR diff mode)` steps). |
| Socket secret | Optional; step self-skips | Required | Bot PRs run without secrets and would hard-fail the call (supply-chain.yml, the `workflow_call` secret's `required: false` declaration). |
| Image trust | Keyless cosign signature, SPDX SBOM attestation, SLSA provenance | Unsigned; key-based signing | Verifiable with the workflow identity and no key to protect (SECURITY.md, "Supply chain"; README.md, "Verifying the image"). |
| Retention | `latest` + 5 tagged, untagged pruned, weekly | Keep everything | Bounded registry use with rollback headroom (ghcr-retention.yml, step `Prune GHCR (keep latest + 5 most recent tagged)`). |
| Platforms | `linux/amd64` only | Multi-arch | `[inferred]` Matches the homelab host. |
| Health check | `HEALTHCHECK` in the image invoking the binary's `--healthcheck` mode | No check; curl in the image; a compose `healthcheck` command | A dead database with a live process should fail the container's health, not hide behind a 200. The distroless image has no shell or curl, and a compose `healthcheck` also executes inside the container, so the binary's own probe is the only command that can run there; declaring it in the image means every consumer of the image gets it, not only this compose file. |
| Health check cadence | 30 s interval, 5 s timeout, 10 s start period, 3 retries | Docker defaults (30 s / 30 s / 0 s / 3) | The probe's own 3 s limit fits inside 5 s; the start period covers migrations on first boot; three failures filter a single slow `SELECT 1`. |

## Open Questions & Future Decisions

### Resolved
*(none yet)*

### Deferred
1. **Unhealthy does not restart.** `restart: unless-stopped` acts on exit, not on health; recovering from a wedged database needs an operator or an external watcher.
2. **Publish does not depend on CI.** `docker-publish.yml`'s `build-and-push` job `needs` only the scan; a `main` push failing clippy or tests would still publish `latest` if it ever bypassed branch protection.
3. **Inconsistent `npm ci` hardening.** `ci.yml`'s `frontend` job (step `Install dependencies`) runs lifecycle scripts; the Dockerfile and the scan do not.
4. **No `concurrency:` groups** on CI or publish; rapid pushes can race to tag `latest`.
5. **Semver tags are pruned** by the keep-5 rule once enough `sha-*` tags accrue, while README.md ("Updating") suggests pinning `0.1`.
6. **README backup procedure** (`docker compose exec app sqlite3 …`, README.md, "Backup & restore") cannot run in a shell-less image.
7. **Compose and docs disagree on variables.** `DB_MAX_CONNECTIONS` is documented but not passed; `STATIC_DIR` is passed but undocumented; overriding `SERVER_PORT` breaks the hard-coded `3001:3001` mapping.
8. **Toolchain drift.** CI uses floating `stable` Rust; the image pins 1.98.
9. **Renovate residue.** Overlapping automerge rules for Docker (branch vs grouped PR), a stable-image list naming services this repo does not use, and `:enablePreCommit` with no pre-commit config.
10. **Ignore-file naming.** Both ignore files negate a dot-prefixed example file; the committed template is `env.example` without the dot.
11. **OSV SARIF** is written but never uploaded to code scanning; annotations are the only surfacing.
12. **Socket org** is hard-coded to `grecolabs`.

## References

- Dockerfile; docker-compose.yml; .dockerignore; .gitignore; env.example
- .github/workflows/ci.yml, docker-publish.yml, supply-chain.yml, ghcr-retention.yml
- renovate.json; frontend/osv-scanner.toml; frontend/vitest.config.ts (coverage thresholds)
- README.md ("Quick start (Docker)", "Verifying the image", "Updating", "Backup & restore"); SECURITY.md ("Supply chain"); CONTRIBUTING.md ("Before you push"); .github/pull_request_template.md
- Consumers: `app` (`STATIC_DIR`, env passthrough, health endpoint)
