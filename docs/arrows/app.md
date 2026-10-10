# Arrow: app

The frame the features sit in: startup configuration, the unauthenticated-API perimeter (content-type gate, concurrency ceiling, body and time limits, CORS, security headers), error-to-HTTP mapping, the health probe, and SPA delivery with its router, navigation, and fetch wrapper.

## Status

**AUDITED** — last audited 2026-10-10 (git SHA `4da220e`). All 24 specs are implemented, annotated in code, and cited by at least one test; what remains open is the small design items the LLD lists as deferred.

## References

### HLD
- docs/high-level-design.md (System Design → app)

### LLD
- docs/intent/app/app-design.md

### EARS
- docs/intent/app/app-specs.md (24 specs: 24 implemented, 0 deferred, 0 active gaps)

### Tests
- backend/src/config.rs inline tests — `defaults_apply_when_only_required_vars_set`, `overrides_pick_up_env_vars`, `missing_tmdb_api_key_fails`, `empty_tmdb_api_key_fails`, `invalid_port_fails`, `invalid_timezone_fails`, `empty_optional_vars_treated_as_absent`
- backend/src/db/pool.rs inline tests — `create_pool_runs_migrations_on_memory_db`, `create_pool_uses_default_max_connections_when_env_invalid`
- backend/src/error.rs inline tests — `not_found_maps_to_404_with_message`, `invalid_data_maps_to_400`, `config_maps_to_500_with_message`, `upstream_maps_to_502`, `database_error_hidden_from_response_body`
- backend/src/lib.rs inline tests — `log_filter_defaults_to_info_when_rust_log_is_unset`, `log_filter_honours_rust_log`, `cors_invalid_origin_panics`, `security_headers_are_applied_to_responses`
- backend/src/scheduler.rs inline tests — `start_returns_config_error_for_invalid_cron`
- backend/tests/api.rs — `health_returns_ok_when_db_responsive`, `health_returns_503_degraded_when_db_unreachable`, `form_encoded_post_to_sync_is_rejected`, `text_plain_post_to_sync_is_rejected`, `json_suffix_and_charset_parameter_are_accepted`, `get_requests_are_unaffected_by_the_content_type_gate`, `wildcard_branch_grants_no_more_than_the_named_origin_branch`, `wildcard_cors_does_not_expose_all_response_headers`, `wildcard_cors_never_allows_credentials`, `wildcard_cors_still_allows_any_origin_with_the_explicit_method_list`, `named_origin_branch_still_denies_other_origins`, `api_routes_live_only_under_the_v1_prefix`, `request_bodies_over_one_mebibyte_are_rejected_with_413`, `the_sixty_fifth_in_flight_request_is_shed_with_503`, `requests_running_past_thirty_seconds_get_408`, `static_fallback_serves_index_for_unknown_paths_but_not_for_api_paths`, `missing_static_dir_leaves_the_app_api_only`
- backend/tests/healthcheck.rs (4 tests, all on the `--healthcheck` probe)
- frontend/src/App.test.tsx (7 tests, one per route except `/movies` and `/movies/:tmdbId`)
- frontend/src/components/Layout.test.tsx (3 tests)
- frontend/src/api/client.test.ts — `builds /api/v1 URLs with JSON content-type and parses JSON success`, `encodes search query strings safely`, `returns undefined for 204 No Content`, `rejects with ApiError carrying the status and the bare server message`, `falls back to raw text when error body is not JSON`, `falls back to raw text when JSON has no error field`, `listShows hits /shows`
- frontend/src/pages/*.test.tsx — the error-display tests in Calendar, History, MovieDetail, Movies, Search, Settings, ShowDetail, UpNext, and Watchlist cite APP-SPA-006 alongside their own segment's ID

### Code
- backend/src/lib.rs — `with_security_headers`, `require_json_content_type`, `build_api_router`, `build_cors_layer`, `log_filter`, `with_static_fallback`, `run`
- backend/src/main.rs — `main` (the `--healthcheck` dispatch)
- backend/src/config.rs — `Config::from_env`
- backend/src/db/pool.rs — `create_pool`
- backend/src/error.rs — `AppError::client_message`, `AppError::into_response`
- backend/src/api/health.rs — `health_check`, `probe`, `probe_from_env`
- backend/src/scheduler.rs — `start` (the `RESYNC_CRON` parse failure is a startup configuration error)
- Dockerfile — the `STATIC_DIR` `ENV` and the `HEALTHCHECK` instruction
- docker-compose.yml — the `app` service (`STATIC_DIR` and `RUST_LOG` environment)
- frontend/src/App.tsx — `App`
- frontend/src/components/Layout.tsx — `Layout`
- frontend/src/api/client.ts — `request`, the `api` object (query encoding)
- frontend/src/pages/*.tsx (APP-SPA-006 on every page component)
- Consumed from other segments: `TmdbClient::new` (tmdb), `scheduler::start` and `resync_all` (resync), `AppState::new` (state.rs; `today_in` there belongs to shows), and every feature route handler mounted by `build_api_router`.
- Consumers: every feature segment — routes are mounted by `build_api_router`, pages are routed by `App` inside `Layout`, handlers return `AppError`, and pages call the API through `request`.

## Architecture

**Purpose:** Make an unauthenticated API safe enough for a home LAN and deliver the SPA from the same process.

**Key Components:**
1. `Config::from_env` and `create_pool` — fail-fast startup from environment variables and embedded migrations.
2. Router middleware — content-type gate, load-shed concurrency limit, body limit, timeout; CORS on the API; security headers over everything.
3. `AppError` — status mapping with generic text for internal variants.
4. SPA shell — static serving with history fallback, `BrowserRouter` routes, `Layout` nav, the `request` fetch wrapper that rejects with `ApiError { status, message }`.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Config | APP-CONFIG-001 to 005 | 5 | 0 | 0 |
| HTTP | APP-HTTP-001 to 008 | 8 | 0 | 0 |
| Errors | APP-ERR-001 to 002 | 2 | 0 | 0 |
| Health | APP-HEALTH-001 to 003 | 3 | 0 | 0 |
| SPA | APP-SPA-001 to 006 | 6 | 0 | 0 |

**Summary:** 24 of 24 active specs implemented; 0 deferred. (Specs with no test citation: none.)

## Key Findings

1. **Two settings bypass `Config`** — `STATIC_DIR` (backend/src/lib.rs:`run`) and `DB_MAX_CONNECTIONS` (backend/src/db/pool.rs:`create_pool`). `STATIC_DIR` is set by the Dockerfile and docker-compose.yml but appears in neither env.example nor README; `DB_MAX_CONNECTIONS` is documented in both but is not passed by docker-compose.yml. LLD Deferred item 1.
2. **Invalid `CORS_ALLOWED_ORIGIN` panics** (backend/src/lib.rs:`build_cors_layer`) rather than returning the `Config` error every other setting uses; `cors_invalid_origin_panics` pins the panic. LLD Deferred item 2.
3. **Unparsable `SERVER_HOST` silently binds `0.0.0.0`** (backend/src/lib.rs:`run`). LLD Deferred item 3.
4. **The 30 s timeout drops long handlers mid-work** (backend/src/lib.rs:`build_api_router`): a many-season add or a manual sync gets a 408 while the upstream calls continue. LLD Deferred item 4.
5. **`Config` error text is client-visible** on 500 responses (backend/src/error.rs:`client_message`); handlers use the variant for impossible states. LLD Deferred item 5.
6. **No catch-all route** in frontend/src/App.tsx:`App`; an unknown path renders an empty shell. LLD Deferred item 7.
7. **Eight hand-copied fetch skeletons** — the `cancelled`-flag load pattern is duplicated across the eight data-loading pages with inconsistent loading, error, and empty structure. LLD Deferred item 9.
8. **`App.test.tsx` does not exercise the two movie routes** — `/movies` and `/movies/:tmdbId` are the only APP-SPA-001 routes without a routing test.
9. **backend/src/lib.rs is 393 lines**, over the repository's 300-line guideline. LLD Deferred item 10.

## Work Required

### Must Fix

### Should Fix
1. Fold `STATIC_DIR` and `DB_MAX_CONNECTIONS` into `Config`; document `STATIC_DIR` in env.example and README, and pass `DB_MAX_CONNECTIONS` through docker-compose.yml.

### Nice to Have
2. Return a `Config` error for a bad CORS origin instead of panicking.
3. Add a not-found route and a shared data-loading hook.
4. Add `/movies` and `/movies/:tmdbId` cases to frontend/src/App.test.tsx.
5. Split backend/src/lib.rs (perimeter middleware and startup are separable) to get under the 300-line guideline.
