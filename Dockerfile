# Stage 1: Build frontend
# glibc (Debian slim), not alpine/musl: matches the CI environment the committed
# lockfile is resolved against, so `npm ci` installs exactly the audited tree.
FROM node:24-trixie-slim@sha256:50c3b2f6988dfc307b86e5301d69611af31f4789bdf232863b07d3b02fe55ae0 AS frontend-build
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

# Stage 2: Build backend
FROM rust:1.98-slim-trixie@sha256:17d1ba895198f9934c6314ec5346a0d5115372f3243390c3d731e242f35c2f27 AS backend-build
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

# Stage 3: Runtime — Chainguard glibc-dynamic. No shell, no package manager,
# no libssl (rustls handles TLS). The image's default user is uid 65532
# (nonroot) and it ships a CA bundle at /etc/ssl/certs/ca-certificates.crt
# which reqwest's rustls-platform-verifier picks up automatically.
FROM cgr.dev/chainguard/glibc-dynamic:latest@sha256:eaec65b25f35619be16f4992e7bae1128eafcf63c114f2859b800a7020c1ef70

WORKDIR /app

COPY --from=backend-build --chown=65532:65532 /app/target/release/showrunner-backend /app/showrunner-backend
COPY --from=frontend-build --chown=65532:65532 /app/frontend/dist /app/static
COPY --from=backend-build --chown=65532:65532 /rootfs/data /data

ENV STATIC_DIR=/app/static

EXPOSE 3001

ENTRYPOINT ["/app/showrunner-backend"]
