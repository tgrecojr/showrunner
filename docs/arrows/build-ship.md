# Arrow: build-ship

How the one container image is built reproducibly, kept small and unprivileged, gated before merge, scanned and updated, published with signatures and attestations, and pruned.

## Status

**MAPPED** — sampled 2026-10-10 (git SHA `3b525e1`), not yet audited. Skeleton LLD and EARS specs were reverse-engineered from configuration; design rationale carries `[inferred]` markers until confirmed.

## References

### HLD
- docs/high-level-design.md (System Design → build-ship)

### LLD
- docs/intent/build-ship/build-ship-design.md

### EARS
- docs/intent/build-ship/build-ship-specs.md (17 specs: 16 implemented, 1 active gap)

### Tests
- None in the conventional sense. The gates are self-verifying on each run; image verification is documented in README.md:102-118 (`cosign verify`, `gh attestation verify`).

### Code
- Dockerfile; docker-compose.yml; .dockerignore; .gitignore
- .github/workflows/ci.yml; .github/workflows/docker-publish.yml; .github/workflows/supply-chain.yml; .github/workflows/ghcr-retention.yml
- renovate.json; frontend/osv-scanner.toml; frontend/vitest.config.ts:22-27 (coverage thresholds)
- Related docs: README.md:54-131, :215-224; SECURITY.md:32-42; CONTRIBUTING.md:37-63; .github/pull_request_template.md

## Architecture

**Purpose:** Produce a signed, scanned, reproducible image and keep untested or vulnerable changes off `main`.

**Key Components:**
1. Three-stage Dockerfile — lockfile-exact builds on glibc, distroless non-root runtime.
2. `ci.yml` — formatting, lint, typecheck, tests with coverage floors.
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

**Summary:** 17 of 17 active specs implemented; no gaps.

## Key Findings

1. **Publish does not gate on CI** — docker-publish.yml:23 `needs: supply-chain` only.
2. **`npm ci` hardening is inconsistent** — ci.yml:77 runs lifecycle scripts; Dockerfile:12 and supply-chain.yml:35 do not.
3. **No `concurrency:` groups** on ci.yml or docker-publish.yml.
4. **Semver tags will be pruned** — every `main` push adds a `sha-*` tag and retention keeps five (ghcr-retention.yml:25, docker-publish.yml:53); README.md:131 suggests pinning `0.1`.
5. **README backup instructions cannot work** against a shell-less runtime (README.md:220 vs Dockerfile:39-40).
6. **Variable passthrough disagrees with docs** — `DB_MAX_CONNECTIONS` documented but not in compose; `STATIC_DIR` in compose but undocumented; `SERVER_PORT` tunable in docs but the port mapping is hard-coded (docker-compose.yml:8-18).
7. **Floating `stable` Rust in CI vs pinned 1.98 in the image** (ci.yml:27, Dockerfile:17).
8. **Renovate residue** — overlapping Docker automerge rules, an unused stable-image list, `:enablePreCommit` without a config (renovate.json:28-41, :59-64, :8).
9. **Ignore files negate a dot-prefixed example file** while the template is `env.example` (.gitignore:5, .dockerignore:4).

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` decisions (scan-only publish gate, amd64-only).

### Should Fix
2. Add `--ignore-scripts` to ci.yml's `npm ci`.
3. Fix the README backup procedure and the variable documentation (cascade to `app` for `STATIC_DIR` / `DB_MAX_CONNECTIONS`).
4. Add `concurrency:` groups to CI and publish.

### Nice to Have
5. Make publish depend on CI, or document branch protection as the guarantee.
6. Clean up the Renovate rules and ignore-file naming.
