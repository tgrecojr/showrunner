# Arrow: build-ship

How the one container image is built reproducibly, kept small and unprivileged, gated before merge, scanned and updated, published with signatures and attestations, and pruned.

## Status

**AUDITED** — last audited 2026-10-10 (git SHA `4da220e`). All 17 specs implemented and annotated; the two `[inferred]` decisions in the LLD (scan-only publish gate, amd64-only) are still unconfirmed, and the LLD's twelve Deferred items remain open.

## References

### HLD
- docs/high-level-design.md (System Design → build-ship)

### LLD
- docs/intent/build-ship/build-ship-design.md

### EARS
- docs/intent/build-ship/build-ship-specs.md (17 specs: 17 implemented, 0 deferred, 0 active gaps)

### Tests
- The gate steps are their own checks (HLD → Annotation Conventions). `SHIP-IMAGE-002` to `006` describe the built image and have no check; `SHIP-SUPPLY-004` lives in JSON and cannot be cited.
- .github/workflows/ci.yml — job `backend`: steps `Check formatting`, `Lint with Clippy`, `Run tests with coverage`, `Upload backend coverage report`; job `frontend`: steps `Install dependencies`, `Type check`, `Lint and check formatting`, `Run tests with coverage`, `Upload frontend coverage report`; job `supply-chain` (schedule-only call of supply-chain.yml, absolute mode).
- .github/workflows/supply-chain.yml — job `scan`: steps `Install frontend deps (no scripts)`, `Verify npm package signatures`, `Socket Security scan (frontend)`, `Fetch base-branch lockfiles (PR diff mode)`, `OSV scan base lockfiles (PR diff mode)`, `OSV scan PR lockfiles (PR diff mode)`, `Check that both scans completed (PR diff mode)`, `Report vulnerabilities introduced by this PR`, `OSV scan (absolute, main / scheduled)`, `cargo audit (backend)`.
- .github/workflows/docker-publish.yml — job `supply-chain` (call of supply-chain.yml); job `build-and-push`: steps `Extract metadata (tags, labels)`, `Build and push Docker image` (the only check that exercises `SHIP-IMAGE-001`; builds without pushing on pull requests), `Install cosign`, `Sign image with cosign (keyless)`, `Generate SBOM (SPDX)`, `Attest SBOM`, `Attest build provenance`.
- .github/workflows/ghcr-retention.yml — job `prune`: step `Prune GHCR (keep latest + 5 most recent tagged)`.
- Image verification by an operator is documented in README.md → Verifying the image (`cosign verify`, `gh attestation verify`).

### Code
- Dockerfile — stages `frontend-build` and `backend-build` (`SHIP-IMAGE-001`), the Chainguard runtime stage (`SHIP-IMAGE-002`), `ENV STATIC_DIR` / `EXPOSE` / `ENTRYPOINT` (`SHIP-IMAGE-003`), `HEALTHCHECK` (`SHIP-IMAGE-006`).
- docker-compose.yml — service `app` (`SHIP-IMAGE-004`; shared with `APP-CONFIG-004`, `APP-CONFIG-005`).
- .dockerignore — the whole file (`SHIP-IMAGE-005`).
- backend/Cargo.toml — the `reqwest` dependency with `rustls` in place of native-tls (`SHIP-IMAGE-002`).
- frontend/vitest.config.ts — `coverage.thresholds` (`SHIP-CI-001`).
- .github/workflows/ci.yml — `on` triggers (`SHIP-CI-001`, `SHIP-CI-004`), `permissions` (`SHIP-CI-003`), jobs `backend` and `frontend` (`SHIP-CI-001`), their upload steps (`SHIP-CI-002`), job `supply-chain` (`SHIP-CI-004`, `SHIP-SUPPLY-003`).
- .github/workflows/supply-chain.yml — job `scan` (`SHIP-SUPPLY-001`), its `permissions` (`SHIP-CI-003`), the PR diff-mode OSV steps (`SHIP-SUPPLY-002`), the absolute OSV step (`SHIP-SUPPLY-003`).
- .github/workflows/docker-publish.yml — workflow and job `permissions` (`SHIP-CI-003`), job `supply-chain` (`SHIP-SUPPLY-003`), job `build-and-push` and its metadata step (`SHIP-RELEASE-001`), the build step (`SHIP-IMAGE-001`), the cosign / SBOM / attestation steps (`SHIP-RELEASE-002`).
- .github/workflows/ghcr-retention.yml — `permissions` (`SHIP-CI-003`), job `prune` (`SHIP-RELEASE-003`).
- renovate.json — `SHIP-SUPPLY-004` (JSON cannot carry a `@spec` comment; the LLD's Retention and dependency updates section is its only anchor).
- Related, unannotated: .gitignore; env.example; frontend/osv-scanner.toml (per-lockfile OSV ignore list read by the scan); README.md (Quick start, Verifying the image, Updating, Backup & restore); SECURITY.md (Supply chain); CONTRIBUTING.md (Before you push); .github/pull_request_template.md.
- Consumed from other segments: `app` — the binary's `--healthcheck` mode (`main`, `api::health::probe_from_env`) that the image's `HEALTHCHECK` runs, and the `STATIC_DIR` setting the runtime stage and compose file supply.
- Consumers: `app` (`STATIC_DIR`, environment passthrough, health endpoint).

## Architecture

**Purpose:** Produce a signed, scanned, reproducible image and keep untested or vulnerable changes off `main`.

**Key Components:**
1. Three-stage Dockerfile — lockfile-exact builds on glibc, distroless non-root runtime with a self-probing `HEALTHCHECK`.
2. `ci.yml` — formatting, lint, typecheck, tests with coverage floors; weekly absolute supply-chain scan of `main`.
3. `supply-chain.yml` — signatures, Socket, OSV diff/absolute, cargo audit.
4. `docker-publish.yml` and `ghcr-retention.yml` — scan-gated push, keyless signing, SBOM and provenance, weekly prune.
5. `renovate.json` — digest pinning and grouped, soaked auto-merges.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Image | SHIP-IMAGE-001 to 006 | 6 | 0 | 0 |
| CI | SHIP-CI-001 to 004 | 4 | 0 | 0 |
| Supply | SHIP-SUPPLY-001 to 004 | 4 | 0 | 0 |
| Release | SHIP-RELEASE-001 to 003 | 3 | 0 | 0 |

**Summary:** 17 of 17 active specs implemented; 0 deferred. (Specs with no test citation: SHIP-IMAGE-002, SHIP-IMAGE-003, SHIP-IMAGE-004, SHIP-IMAGE-005, SHIP-IMAGE-006 — image properties with no separate check; SHIP-SUPPLY-004 — uncitable JSON.)

## Key Findings

All nine are recorded as Deferred items in the LLD (Open Questions & Future Decisions → Deferred) and are still true at `4da220e`:

1. **Publish does not gate on CI** — docker-publish.yml:`build-and-push` has `needs: supply-chain` only (LLD Deferred 2).
2. **`npm ci` hardening is inconsistent** — ci.yml:`frontend` step `Install dependencies` runs lifecycle scripts; Dockerfile:`frontend-build` and supply-chain.yml:`Install frontend deps (no scripts)` do not (LLD Deferred 3).
3. **No `concurrency:` groups** on ci.yml or docker-publish.yml; only ghcr-retention.yml:`prune` declares one (LLD Deferred 4).
4. **Semver tags will be pruned** — docker-publish.yml:`Extract metadata` adds a `sha-*` tag on every `main` push and ghcr-retention.yml:`prune` keeps five, while README.md → Updating suggests pinning `showrunner:0.1` (LLD Deferred 5).
5. **README backup instructions cannot work** — README.md → Backup & restore runs `docker compose exec app sqlite3 …` against the shell-less Dockerfile runtime stage (LLD Deferred 6).
6. **Variable passthrough disagrees with docs** — README.md → Configuration lists `DB_MAX_CONNECTIONS`, which docker-compose.yml:`app` does not pass; compose hard-codes `STATIC_DIR`, which the README does not document; `SERVER_PORT` is tunable but the `3001:3001` mapping is fixed (LLD Deferred 7).
7. **Floating `stable` Rust in CI vs pinned 1.98 in the image** — ci.yml:`backend` and supply-chain.yml:`scan` use `dtolnay/rust-toolchain@…# stable`; Dockerfile:`backend-build` pins `rust:1.98-slim-trixie` (LLD Deferred 8).
8. **Renovate residue** — renovate.json: overlapping Docker automerge rules (`automergeType: branch` vs the grouped PR), a stable-image list naming postgres/mariadb/nginx/redis/traefik, and `:enablePreCommit` without a pre-commit config (LLD Deferred 9).
9. **Ignore files negate a dot-prefixed example file** — .gitignore and .dockerignore both carry `!.env.example` while the committed template is `env.example` (LLD Deferred 10).

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` rows in the LLD decisions table (scan-only publish gate, amd64-only).

### Should Fix
2. Add `--ignore-scripts` to ci.yml's `Install dependencies` step (Finding 2).
3. Fix the README backup procedure and the variable documentation (Findings 5, 6; cascade to `app` for `STATIC_DIR` / `DB_MAX_CONNECTIONS`).
4. Add `concurrency:` groups to CI and publish (Finding 3).

### Nice to Have
5. Make publish depend on CI, or record branch protection as the guarantee in the LLD (Finding 1).
6. Clean up the Renovate rules and the ignore-file example name (Findings 8, 9).
7. Decide whether CI should pin the Rust toolchain to the image's version (Finding 7).
