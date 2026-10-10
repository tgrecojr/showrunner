# Arrow: app

The frame the features sit in: startup configuration, the unauthenticated-API perimeter (content-type gate, concurrency ceiling, body and time limits, CORS, security headers), error-to-HTTP mapping, the health probe, and SPA delivery with its router, navigation, and fetch wrapper.

## Status

**MAPPED** — sampled 2026-10-10 (git SHA `3b525e1`), not yet audited. Skeleton LLD and EARS specs were reverse-engineered from code; design rationale carries `[inferred]` markers until confirmed.

## References

### HLD
- docs/high-level-design.md (System Design → app)

### LLD
- docs/intent/app/app-design.md

### EARS
- docs/intent/app/app-specs.md (23 specs: 21 implemented, 2 active gaps)

### Tests
- backend/src/lib.rs tests — JSON content-type matching, CORS branches including the invalid-origin panic
- backend/src/config.rs tests — defaults, overrides, missing/blank key, bad port, bad timezone
- backend/src/db/pool.rs tests — `DB_MAX_CONNECTIONS` parsing; backend/src/error.rs tests — status and body mapping
- backend/tests/api.rs — health (:76), content-type gate (:878-974), CORS parity and wildcard limits (:976-1156)
- frontend/src/App.test.tsx (7 routes), frontend/src/components/Layout.test.tsx (3), frontend/src/api/client.test.ts (15)
- Not covered: load shed (503), body limit, timeout (408), security headers, static fallback, the two movie routes in `App.test.tsx`

### Code
- backend/src/lib.rs (`with_security_headers` :58, `require_json_content_type` :93, `build_api_router` :120, `build_cors_layer` :184, `run` :234); backend/src/main.rs
- backend/src/config.rs; backend/src/db/pool.rs; backend/src/error.rs; backend/src/api/health.rs
- frontend/src/main.tsx; frontend/src/App.tsx; frontend/src/components/Layout.tsx; frontend/src/api/client.ts; frontend/src/types/index.ts; frontend/src/index.css; frontend/index.html; frontend/vite.config.ts
- env.example

## Architecture

**Purpose:** Make an unauthenticated API safe enough for a home LAN and deliver the SPA from the same process.

**Key Components:**
1. `Config::from_env` and `create_pool` — fail-fast startup from environment variables and embedded migrations.
2. Router middleware — content-type gate, load-shed concurrency limit, body limit, timeout; CORS on the API; security headers over everything.
3. `AppError` — status mapping with generic text for internal variants.
4. SPA shell — static serving with history fallback, `BrowserRouter` routes, `Layout` nav, the `request` fetch wrapper.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Config | APP-CONFIG-001 to 005 | 5 | 0 | 0 |
| HTTP | APP-HTTP-001 to 008 | 8 | 0 | 0 |
| Errors | APP-ERR-001 to 002 | 2 | 0 | 0 |
| Health | APP-HEALTH-001 to 002 | 1 | 0 | 1 |
| SPA | APP-SPA-001 to 006 | 5 | 0 | 1 |

**Summary:** 21 of 23 active specs implemented; 2 gaps (`APP-HEALTH-002` degraded health is 503, `APP-SPA-006` strip the `API <status>:` prefix on every page).

## Key Findings

1. **Health is always 200** (backend/src/api/health.rs:23-27) and nothing probes it; a dead database is invisible to any status-code check. Intended: `APP-HEALTH-002`, probed by `SHIP-IMAGE-006`.
2. **Two settings bypass `Config`** — `STATIC_DIR` (backend/src/lib.rs:254) and `DB_MAX_CONNECTIONS` (backend/src/db/pool.rs:8); the first is undocumented, the second is not passed by `docker-compose.yml`.
3. **Invalid `CORS_ALLOWED_ORIGIN` panics** (lib.rs:217) rather than returning the `Config` error every other setting uses; a test pins the panic.
4. **Unparsable `SERVER_HOST` silently binds `0.0.0.0`** (lib.rs:269).
5. **The 30 s timeout drops long handlers mid-work** (lib.rs:177-180): a many-season add or a manual sync gets a 408 while the upstream calls continue.
6. **`Config` error text is client-visible** on 500 responses (backend/src/error.rs:57-61); handlers use it for impossible states.
7. **No catch-all route** in frontend/src/App.tsx; an unknown path renders an empty shell.
8. **Error prefix stripping exists only on MovieDetail** (frontend/src/pages/MovieDetail.tsx:7); every other page shows `API 502: …`. Intended: `APP-SPA-006`.
9. **Eight hand-copied fetch skeletons** with inconsistent loading, error, and empty structure (sweep cross-file note).
10. **Perimeter layers are untested** — no test exercises load shed, the body limit, the timeout, or the security headers.

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` decisions (request bounds, localhost CORS default, SQLite mode, Up Next as home, thin fetch wrapper, plain stylesheet).
2. Implement `APP-HEALTH-002` and `APP-SPA-006` (the latter ideally in one shared place).

### Should Fix
3. Fold `STATIC_DIR` and `DB_MAX_CONNECTIONS` into `Config` and document them.
4. Add tests for the load shed, body limit, timeout, and security headers.

### Nice to Have
5. Return a `Config` error for a bad CORS origin instead of panicking.
6. Add a not-found route and a shared data-loading hook.
