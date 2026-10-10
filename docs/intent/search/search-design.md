---
parent: high-level-design
prefix: SEARCH
---

# Search

## Context and Design Philosophy

Search is how a title enters Showrunner. The user types a name, the backend asks TMDB's multi-search on their behalf, and each result comes back tagged with whether it is already on the show watchlist or the movie list so the page can offer either an **Add** button or an **On watchlist** pill. The TMDB API key lives only in the server's environment; the browser never calls TMDB directly (SECURITY.md:28).

The segment owns the `GET /api/v1/search` handler and the Search page. It does not own what happens after Add: the add-show and add-movie endpoints, and the tracked-id lookups used to compute `already_tracked`, belong to `shows` and `movies` and are consumed across the boundary.

## Backend request flow

`search_shows` (backend/src/api/search.rs:54-143):

1. **Validate** — `q` is trimmed; an empty result is a 400 `InvalidData("query parameter `q` is required")` (:58-63). There is no length cap.
2. **Call TMDB** — the request is assembled in the handler from `TmdbClient` accessors (`base_url()`, `http()`, `api_key()`) rather than a client method (:65-76). Query params: `api_key`, `query`, `include_adult=false`. This is the only TMDB call in the codebase that is not a `TmdbClient` method.
3. **Reject non-2xx** — any non-success status becomes `Upstream("TMDB returned <status>")`, which the perimeter maps to HTTP 502 (:78-83). A 429 is not special-cased here, unlike `TmdbClient::get_movie` (backend/src/datasources/tmdb.rs:112-117).
4. **Parse within cap** — the body goes through `tmdb::json_within_cap` (:85), which refuses bodies whose `Content-Length` exceeds 16 MiB. That rule is owned by `tmdb`.
5. **Partition ids** — results with `media_type` `tv` or `movie` are collected into two id lists; everything else (persons, missing type) is ignored (:87-95).
6. **Look up tracked ids** — two IN-list queries, `tracked_tmdb_ids_in` against `shows` and `tracked_movie_tmdb_ids_in` against `movies` (:97-104). Both short-circuit to no query when their list is empty (backend/src/db/queries.rs:413, :428).
7. **Shape results** — TV uses `name` + `first_air_date`; movies use `title` + `release_date`. Empty strings for `overview`, `date`, and `poster_path` become `null`; `poster_url` is prefixed with the w185 poster base (:106-140). TMDB's order is preserved; there is no ranking, deduplication, or pagination, so only TMDB's first page is ever returned.

### Wire shape

| Field | Type | Source |
|---|---|---|
| `media_type` | `"tv"` \| `"movie"` | TMDB discriminator |
| `tmdb_id` | integer | TMDB `id` |
| `name` | string | `name` (tv) or `title` (movie), empty string if absent |
| `overview` | string \| null | empty → null |
| `date` | string \| null | `first_air_date` (tv) or `release_date` (movie), empty → null |
| `poster_url` | string \| null | `https://image.tmdb.org/t/p/w185` + `poster_path` |
| `already_tracked` | boolean | id present in `shows` (tv) or `movies` (movie) |

## Search page

`Search` (frontend/src/pages/Search.tsx):

- **Input** — a `type="search"` field, auto-focused on mount with an explicit lint exemption, labelled "Search movies and TV shows" (:109-118).
- **Debounce** — the effect trims the query; a blank query clears results, error, and loading immediately with no request (:16-22). A non-blank query starts a 350 ms timer; when it fires, `loading` is set and `api.search` runs (:25-45). Cleanup clears the timer and sets a `cancelled` flag so a late response is ignored; there is no `AbortController`, so the HTTP request itself is not cancelled (:47-50).
- **Result arrival** — a successful response replaces `results` and resets all per-card add state (:30-35). Previous results remain on screen while a new search is in flight; "Searching…" renders above them (:120, :126).
- **Status lines** — "Searching…" while loading; `Error: <message>` on failure ("Search failed" when the rejection is not an `Error`); "No matches." when a non-blank query produced zero results and nothing is loading or errored (:120-124).
- **Cards** — poster or "No poster" placeholder, name with `(YYYY)` from the first four characters of `date`, a `Movie`/`TV` pill, overview when present, then the action area (:130-157).
- **Add flow** — per-card state keyed `media_type:tmdb_id` (:53-55). Clicking **Add** sets `pending` ("Adding…", disabled), calls `addMovie` for movies and `addShow` for everything else, then `added` on success or `error` with the message shown beneath the actions ("Add failed" for non-`Error` rejections) (:57-79, :88-103, :154-156). A card whose server flag `already_tracked` is true, or whose local state is `added`, renders the **On watchlist** pill instead of a button (:83-87). On `error` the Add button returns so the user can retry.
- **No navigation** — result cards do not link to detail pages; adding keeps the user on Search.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| Where the TMDB call happens | Backend proxy; browser calls `/api/v1/search` | Browser calls TMDB directly with a public key | The API key must never reach the browser (SECURITY.md:28, README.md:50). |
| TMDB endpoint | One `search/multi` request | Separate `search/tv` and `search/movie` requests merged client- or server-side | `[inferred]` One round trip and TMDB's own cross-type ranking; the cost is filtering out `person` results. |
| Request assembly | Inline in the handler via `TmdbClient` accessors | A `TmdbClient::search_multi` method like `get_show`/`get_movie` | `[inferred]` No rationale in code or comments; appears historical. The accessors exist only for this caller. |
| `already_tracked` computation | Server-side, two IN-list queries per response | Client compares results against a cached watchlist | `[inferred]` Keeps the page stateless and correct across tabs; the lists are bounded by TMDB's page size. |
| Adult content | `include_adult=false` hard-coded | Configurable | `[inferred]` Personal homelab tracker; no setting surface exists. |
| Empty-string normalization | `overview`, `date`, `poster_path` empty → `null` | Pass TMDB strings through | `[inferred]` Lets the page use truthiness checks; TMDB uses `""` for unknown dates. |
| Result paging | First TMDB page only, TMDB order | Expose `page`, or fetch several pages | `[inferred]` A name search rarely needs more than the top 20; no UI for paging. |
| Add interaction | Stay on Search with per-card state | Navigate to the new detail page after add | `[inferred]` Supports adding several titles from one query. |
| Debounce | 350 ms after the last keystroke, no abort | Submit button; abortable fetch | `[inferred]` Balances TMDB call volume against responsiveness; a stale response is ignored via the `cancelled` flag. |

## Open Questions & Future Decisions

### Resolved
1. ✅ **Friendly upstream errors are app-wide and mapped on the server.** A user-readable message whenever TMDB rate-limits or fails upstream is intended for every page. The `tmdb` client owns the mapping (`TMDB-ERR-002`, `TMDB-ERR-003`); search carries `SEARCH-API-010` only because its request is assembled outside the client, and the spec is satisfied by moving the request into a client method (deferred question 1).

### Deferred
1. **Move the request into `TmdbClient`.** Would let `tmdb` own every outbound call and retire the `api_key()` accessor.
2. **Cap `q` length.** Currently bounded only by URI limits.
3. **Link results to detail pages.** Cards are not links today; is that deliberate?
4. **Abort in-flight searches** with `AbortController` instead of only ignoring late results.
5. **Add-error styling.** Per-card add errors omit the `Error:` prefix every other page uses (Search.tsx:155). Intentional, or drift?
6. **Poster base constant** duplicated at search.rs:11 rather than reusing `models::show::poster_url`.

## References

- backend/src/api/search.rs
- frontend/src/pages/Search.tsx
- backend/tests/api.rs:91-170 (search tests)
- frontend/src/pages/Search.test.tsx
- Consumed: `shows` (add-show endpoint, `tracked_tmdb_ids_in`), `movies` (add-movie endpoint, `tracked_movie_tmdb_ids_in`), `tmdb` (`json_within_cap`, accessors)
- SECURITY.md:28 — key never reaches the browser
