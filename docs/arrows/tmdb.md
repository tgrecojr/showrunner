# Arrow: tmdb

The adapter for the only external data source: the HTTP client, the TMDB response shapes the app reads, the reductions to providers, networks, directors, and image URLs, and the rules that keep the API key out of errors.

## Status

**MAPPED** — sampled 2026-10-10 (git SHA `3b525e1`), not yet audited. Skeleton LLD and EARS specs were reverse-engineered from code; design rationale carries `[inferred]` markers until confirmed.

## References

### HLD
- docs/high-level-design.md (System Design → tmdb)

### LLD
- docs/intent/tmdb/tmdb-design.md

### EARS
- docs/intent/tmdb/tmdb-specs.md (15 specs: 13 implemented, 2 active gaps)

### Tests
- backend/src/datasources/tmdb.rs unit tests (:320-606) — wiremock-driven client behavior, body cap, 404/429 mapping, provider and network reductions, URL stripping (:579)
- backend/src/models/show.rs and movie.rs unit tests (image URL helpers)
- backend/src/error.rs unit tests (variant to status mapping)
- backend/tests/api.rs — 429 on movie detail (:748), 502 on search (:156), outbound `api_key` param (:98)

### Code
- backend/src/datasources/tmdb.rs
- backend/src/error.rs:37-46 (`From<reqwest::Error>`)
- backend/src/models/show.rs:1-15, backend/src/models/movie.rs:1-9 (image URL helpers)
- Consumers: backend/src/api/search.rs (accessors), backend/src/api/shows.rs, backend/src/api/movies.rs, backend/src/logic/resync.rs

## Architecture

**Purpose:** Talk to TMDB on the user's behalf without leaking the key, and hand the rest of the app small, stable shapes.

**Key Components:**
1. `TmdbClient` — three request methods plus accessors used only by search.
2. `json_within_cap` and the minimal deserialized shapes with empty-collection defaults.
3. Reductions — US providers, network names, directors, image URL helpers.
4. Error conversion — URL stripped at the `reqwest::Error` boundary; status mapping per method.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Client | TMDB-CLIENT-001 to 006 | 6 | 0 | 0 |
| Shape | TMDB-SHAPE-001 to 004 | 4 | 0 | 0 |
| Errors | TMDB-ERR-001 to 005 | 3 | 0 | 2 |

**Summary:** 13 of 15 active specs implemented; 2 gaps (`TMDB-ERR-002` friendly 429 on every call, `TMDB-ERR-003` friendly 5xx on every call).

## Key Findings

1. **Friendly rate-limit text exists on one method only** — `get_movie` (backend/src/datasources/tmdb.rs:112-117); `get_show`, `get_season`, and the search handler surface raw `TMDB returned 429`. Intended: `TMDB-ERR-002`, `TMDB-ERR-003`.
2. **Search bypasses the client** — backend/src/api/search.rs:65-76 uses `base_url()`, `http()`, `api_key()`; the key leaves this module there.
3. **`get_season` has no 404 mapping** (tmdb.rs:136-142); a missing season is a 502.
4. **Chunked responses are not size-capped** (tmdb.rs:10-13); only the timeout bounds them.
5. **No retry, backoff, User-Agent, or caching** anywhere in the client.
6. **`Config` derives `Debug` with the key in a plain `String`** (backend/src/config.rs:7-14); nothing formats it today.
7. **Credential naming mismatch in docs** — README.md:59 names the v4 "Read Access Token"; the code uses a v3 key (`api_key` query param).
8. **Three copies of the w185 image base** (models/show.rs:4, models/movie.rs:4, api/search.rs:11).

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` decisions (v3 key, 10 s timeout, US-only providers and tiers, no retries, season-404-as-upstream).
2. Implement `TMDB-ERR-002` and `TMDB-ERR-003` across `get_show`, `get_season`, and `get_movie`.

### Should Fix
3. Add a `search_multi` client method and retire the `api_key()` accessor (cascade to `search`).
4. Fix README.md:59 to name the v3 API key.

### Nice to Have
5. Map season 404 to not-found.
6. Consolidate the image base constants.
