# Build and ship — EARS specs

Prefix `SHIP`. Facets: `IMAGE` (Dockerfile and compose), `CI` (merge gates), `SUPPLY` (dependency scanning and updates), `RELEASE` (publishing, signing, retention).

## Image

- [x] **SHIP-IMAGE-001**: The image build shall compile the SPA in a digest-pinned Node 24 Debian-slim stage using `npm ci --ignore-scripts` against the committed lockfile, and compile the backend in a digest-pinned Rust 1.98 Debian-slim stage using `cargo build --release --locked`, failing the build when `Cargo.lock` is absent.
- [x] **SHIP-IMAGE-002**: The runtime image shall be a digest-pinned Chainguard `glibc-dynamic` base containing only the backend binary, the built static assets, and a `/data` directory, all owned by uid 65532, with no shell or package manager, running as uid 65532 and using rustls with the image's CA bundle for TLS.
- [x] **SHIP-IMAGE-003**: The runtime image shall set `STATIC_DIR=/app/static`, expose port 3001, and start the backend binary as its entrypoint.
- [x] **SHIP-IMAGE-004**: The compose definition shall build the image locally, restart the service unless stopped, publish host port 3001 to container port 3001, mount the named volume `showrunner_data` at `/data`, and pass `SERVER_HOST`, `SERVER_PORT`, `DATABASE_URL`, `TMDB_API_KEY`, `RESYNC_CRON`, `TIMEZONE`, `CORS_ALLOWED_ORIGIN`, and `RUST_LOG` from the environment with the application defaults, requiring `TMDB_API_KEY`.
- [x] **SHIP-IMAGE-005**: The Docker build context shall exclude `.git`, dotenv files, build outputs, `node_modules`, `dist`, Markdown files, `.claude/`, local data directories, and SQLite files.
- [x] **SHIP-IMAGE-006**: The runtime image shall declare a `HEALTHCHECK` that runs the backend binary with `--healthcheck` every 30 seconds with a 5-second timeout, a 10-second start period, and 3 retries, so that the container reports unhealthy when `GET /api/v1/health` has not returned HTTP 200 for three consecutive probes.

## CI

- [x] **SHIP-CI-001**: On every push to `main` and every pull request, CI shall fail unless `cargo fmt --check` passes, `cargo clippy --all-targets -- -D warnings` passes, backend tests pass with at least 85% line coverage, `tsc --noEmit` passes, `biome ci` passes, and frontend tests pass with coverage at or above the configured thresholds.
- [x] **SHIP-CI-002**: CI shall upload the backend and frontend coverage reports as artifacts even when a job fails.
- [x] **SHIP-CI-003**: Every workflow shall declare least-privilege permissions and reference every action by commit SHA with a version comment.
- [x] **SHIP-CI-004**: Every Monday at 13:00 UTC, CI shall run the supply-chain scan against `main` in absolute mode so that advisories against already-merged dependencies are surfaced.

## Supply

- [x] **SHIP-SUPPLY-001**: The supply-chain scan shall install frontend dependencies with `npm ci --ignore-scripts`, verify npm registry signatures with `npm audit signatures`, run a Socket Security scan of the frontend when the API key secret is present and skip it otherwise, scan both lockfiles with OSV, and run `cargo audit` on the backend.
- [x] **SHIP-SUPPLY-002**: When the supply-chain scan runs for a pull request, it shall scan the base branch's lockfiles and the pull request's lockfiles, fail if either scan produced no results, and fail only on vulnerabilities the pull request introduces.
- [x] **SHIP-SUPPLY-003**: When the supply-chain scan runs for a push to `main` or on a schedule, it shall fail on any known vulnerability in the repository's lockfiles.
- [x] **SHIP-SUPPLY-004**: Renovate shall pin Docker images and GitHub Actions by digest, group all Docker base images into a single pull request, group minor and patch updates into a single pull request auto-merged after a 3-day release age, auto-merge digest updates after a 1-day release age, never auto-merge major updates, and run lockfile maintenance weekly.

## Release

- [x] **SHIP-RELEASE-001**: When a push to `main` or a `v*` tag occurs and the supply-chain scan has passed, the publish workflow shall build the image and push it to `ghcr.io/tgrecojr/showrunner` tagged `sha-<short sha>`, plus `latest` for the default branch and `X.Y.Z` and `X.Y` for a semver tag; for pull requests it shall build without pushing.
- [x] **SHIP-RELEASE-002**: When an image is pushed, the publish workflow shall sign it with keyless cosign by digest, generate an SPDX SBOM from the pushed digest, and push SBOM and SLSA build-provenance attestations to the registry.
- [x] **SHIP-RELEASE-003**: Every Monday at 06:00 UTC and on manual dispatch, the retention workflow shall keep the `latest` tag and the five most recent tagged versions, and delete all untagged manifests.
