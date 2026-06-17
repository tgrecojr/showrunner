# Stage 1: Build frontend
FROM node:24-alpine@sha256:21f403ab171f2dc89bad4dd69d7721bfd15f084ccb46cdd225f31f2bc59b5c9a AS frontend-build
WORKDIR /app/frontend
COPY frontend/package*.json ./
# Using `npm install` instead of `npm ci` because newer eslint/typescript-eslint
# pull in `unrs-resolver`, which has Linux-only optional native deps that an
# npm-on-macOS-generated lockfile omits. The lockfile is still committed and
# surfaces dep drift via PR diffs.
RUN npm install --no-audit --no-fund
COPY frontend/ ./
RUN npm run build

# Stage 2: Build backend
FROM rust:1.96-slim-bookworm@sha256:c8a94a78f67ec8c4d474ec7f71e0720f21eb7e584e158daec0874cafa7c30e4d AS backend-build
WORKDIR /app

# Copy manifests first for dependency caching
COPY backend/Cargo.toml backend/Cargo.lock* ./
RUN mkdir src && echo 'fn main() { println!("placeholder"); }' > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Copy actual source and force recompile
COPY backend/src ./src
RUN touch src/main.rs

RUN cargo build --release

# Pre-create the /data directory with chainguard's nonroot uid (65532) so the
# runtime stage can COPY it in — the distroless runtime has no shell to mkdir
# or chown at build time.
RUN mkdir -p /rootfs/data && chown -R 65532:65532 /rootfs

# Stage 3: Runtime — Chainguard glibc-dynamic. No shell, no package manager,
# no libssl (rustls handles TLS). The image's default user is uid 65532
# (nonroot) and it ships a CA bundle at /etc/ssl/certs/ca-certificates.crt
# which reqwest's rustls-platform-verifier picks up automatically.
FROM cgr.dev/chainguard/glibc-dynamic:latest@sha256:0dc86136587f0ac15d61d307dcd8193e4a9880d26d2f2659b9e2b142640eecc0

WORKDIR /app

COPY --from=backend-build --chown=65532:65532 /app/target/release/showrunner-backend /app/showrunner-backend
COPY --from=frontend-build --chown=65532:65532 /app/frontend/dist /app/static
COPY --from=backend-build --chown=65532:65532 /rootfs/data /data

ENV STATIC_DIR=/app/static

EXPOSE 3001

ENTRYPOINT ["/app/showrunner-backend"]
