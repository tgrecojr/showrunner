# @spec SHIP-IMAGE-001
# Stage 1: Build frontend
# glibc (Debian slim), not alpine/musl: matches the CI environment the committed
# lockfile is resolved against, so `npm ci` installs exactly the audited tree.
FROM node:24-trixie-slim@sha256:7f486cabe892bff8fef04c0f7da8dba84a560be3e4e2740b7a86afc57ebec315 AS frontend-build
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
# `npm ci` installs strictly from the committed lockfile (the set CI audits),
# unlike `npm install` which re-resolves against the live registry at build
# time. `--ignore-scripts` blocks arbitrary lifecycle-script execution; the
# build toolchain (vite/esbuild) ships its binaries as platform packages, not
# install scripts, so the build still works.
RUN npm ci --ignore-scripts --no-audit --no-fund
COPY frontend/ ./
RUN npm run build

# @spec SHIP-IMAGE-001
# Stage 2: Build backend
FROM rust:1.99-slim-trixie@sha256:2752b332db73fdbb7dc576f06c82ed1f312005784ef913d7e04a28f5f55dc581 AS backend-build
WORKDIR /app

# Copy manifests first for dependency caching. No glob on Cargo.lock: the
# lockfile must be present so `--locked` can enforce it (a missing lockfile
# should fail the build, not silently resolve fresh versions).
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo 'fn main() { println!("placeholder"); }' > src/main.rs
RUN cargo build --release --locked 2>/dev/null || true
RUN rm -rf src

# Copy actual source and force recompile
COPY backend/src ./src
RUN touch src/main.rs

RUN cargo build --release --locked

# Pre-create the /data directory with chainguard's nonroot uid (65532) so the
# runtime stage can COPY it in — the distroless runtime has no shell to mkdir
# or chown at build time.
RUN mkdir -p /rootfs/data && chown -R 65532:65532 /rootfs

# @spec SHIP-IMAGE-002
# Stage 3: Runtime — Chainguard glibc-dynamic. No shell, no package manager,
# no libssl (rustls handles TLS). The image's default user is uid 65532
# (nonroot) and it ships a CA bundle at /etc/ssl/certs/ca-certificates.crt
# which reqwest's rustls-platform-verifier picks up automatically.
FROM cgr.dev/chainguard/glibc-dynamic:latest@sha256:eaec65b25f35619be16f4992e7bae1128eafcf63c114f2859b800a7020c1ef70

WORKDIR /app

COPY --from=backend-build --chown=65532:65532 /app/target/release/showrunner-backend /app/showrunner-backend
COPY --from=frontend-build --chown=65532:65532 /app/frontend/dist /app/static
COPY --from=backend-build --chown=65532:65532 /rootfs/data /data

# @spec SHIP-IMAGE-003, APP-CONFIG-004
ENV STATIC_DIR=/app/static

EXPOSE 3001

# @spec SHIP-IMAGE-006, APP-HEALTH-003
# The image has no shell or curl, so the binary probes itself (see
# `--healthcheck` in main.rs). Its own request timeout is 3 s, inside the 5 s
# here; the start period covers first-boot migrations.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/app/showrunner-backend", "--healthcheck"]

ENTRYPOINT ["/app/showrunner-backend"]
