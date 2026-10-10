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
- Consumers: backend/src/api/search.rs (`search_multi`), backend/src/api/shows.rs, backend/src/api/movies.rs, backend/src/logic/resync.rs

## Architecture

**Purpose:** Talk to TMDB on the user's behalf without leaking the key, and hand the rest of the app small, stable shapes.

**Key Components:**
1. `TmdbClient` — four request methods (show, movie, season, search) over one private `get` that is the only place the key is used.
2. `json_within_cap` and the minimal deserialized shapes with empty-collection defaults.
3. Reductions — US providers, network names, directors, image URL helpers.
4. Error conversion — URL stripped at the `reqwest::Error` boundary; one shared status mapper (429 and 5xx sentences, raw status otherwise) after each method's 404 rule.

## Spec Coverage

| Category | Spec IDs | Implemented | Deferred | Gaps |
|----------|----------|-------------|----------|------|
| Client | TMDB-CLIENT-001 to 007 | 7 | 0 | 0 |
| Shape | TMDB-SHAPE-001 to 004 | 4 | 0 | 0 |
| Errors | TMDB-ERR-001 to 005 | 5 | 0 | 0 |

**Summary:** 16 of 16 active specs implemented; no gaps.

## Key Findings

1. **`get_season` has no 404 mapping** (tmdb.rs:136-142); a missing season is a 502.
2. **Chunked responses are not size-capped** (tmdb.rs:10-13); only the timeout bounds them.
3. **No retry, backoff, User-Agent, or caching** anywhere in the client.
4. **`Config` derives `Debug` with the key in a plain `String`** (backend/src/config.rs:7-14); nothing formats it today.
5. **Credential naming mismatch in docs** — README.md:59 names the v4 "Read Access Token"; the code uses a v3 key (`api_key` query param).
6. **Three copies of the w185 image base** (models/show.rs, models/movie.rs, api/search.rs).

## Work Required

### Must Fix
1. Confirm or refute the `[inferred]` decisions (v3 key, 10 s timeout, US-only providers and tiers, no retries, season-404-as-upstream).

### Should Fix
2. Fix README.md:59 to name the v3 API key.

### Nice to Have
3. Map season 404 to not-found.
4. Consolidate the image base constants.
