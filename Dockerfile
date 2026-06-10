# Stage 1: Build frontend
FROM node:24-alpine@sha256:fb71d01345f11b708a3553c66e7c74074f2d506400ea81973343d915cb64eef0 AS frontend-build
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
FROM rust:1.96-slim-bookworm@sha256:b5f842fac1e3b4ff718a652a8e0173b62d9403ec826ef4998880b9347db30684 AS backend-build
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
