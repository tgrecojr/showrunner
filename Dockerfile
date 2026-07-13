# Stage 1: Build frontend
# glibc (Debian slim), not alpine/musl: matches the CI environment the committed
# lockfile is resolved against, so `npm ci` installs exactly the audited tree.
FROM node:24-slim@sha256:cb4e8f7c443347358b7875e717c29e27bf9befc8f5a26cf18af3c3dec80e58c5 AS frontend-build
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
FROM rust:1.96-slim-bookworm@sha256:e18a79fc84dfcfc3ab5ba72290398a644c135c97eaa881447fddc354ee4701a3 AS backend-build
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
FROM cgr.dev/chainguard/glibc-dynamic:latest@sha256:7ff79e2caef2b8a137ddaf9940fb790e91148482092363760d6661e4591fd54c

WORKDIR /app

COPY --from=backend-build --chown=65532:65532 /app/target/release/showrunner-backend /app/showrunner-backend
COPY --from=frontend-build --chown=65532:65532 /app/frontend/dist /app/static
COPY --from=backend-build --chown=65532:65532 /rootfs/data /data

ENV STATIC_DIR=/app/static

EXPOSE 3001

ENTRYPOINT ["/app/showrunner-backend"]
