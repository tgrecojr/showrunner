# Stage 1: Build frontend
FROM node:24-alpine AS frontend-build
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
FROM rust:1.95-slim-bookworm AS backend-build
WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy manifests first for dependency caching
COPY backend/Cargo.toml backend/Cargo.lock* ./
RUN mkdir src && echo 'fn main() { println!("placeholder"); }' > src/main.rs
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Copy actual source and force recompile
COPY backend/src ./src
RUN touch src/main.rs

RUN cargo build --release

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*

# Non-root user; /data writable for SQLite file
RUN useradd -m -s /bin/bash appuser && mkdir -p /data && chown appuser:appuser /data

WORKDIR /app

COPY --from=backend-build /app/target/release/showrunner-backend ./showrunner-backend
COPY --from=frontend-build /app/frontend/dist ./static

ENV STATIC_DIR=/app/static

USER appuser

EXPOSE 3001

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3001/api/v1/health || exit 1

CMD ["./showrunner-backend"]
