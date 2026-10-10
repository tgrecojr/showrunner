---
parent: high-level-design
prefix: SEARCH
---

# Search

## Context and Design Philosophy

Search is how a title enters Showrunner. The user types a name, the backend asks TMDB's multi-search on their behalf, and each result comes back tagged with whether it is already on the show watchlist or the movie list so the page can offer either an **Add** button or an **On watchlist** pill. The TMDB API key lives only in the server's environment; the browser never calls TMDB directly (SECURITY.md, "What the app does protect").

The segment owns the `GET /api/v1/search` handler and the Search page. It does not own what happens after Add: the add-show and add-movie endpoints, and the tracked-id lookups used to compute `already_tracked`, belong to `shows` and `movies` and are consumed across the boundary.

## Backend request flow

`search_shows` (backend/src/api/search.rs):

1. **Validate** — `q` is trimmed; an empty result is a 400 `AppError::InvalidData("query parameter `q` is required")`. There is no length cap.
2. **Call TMDB** — `TmdbClient::search_multi(query)` sends `GET /search/multi` with `query` and `include_adult=false`. The handler never sees the key or the raw HTTP response.
3. **Upstream failures** — the client's status mapper (`map_status`) applies (`tmdb`): 429 and 5xx become the fixed user-readable sentences, any other non-2xx becomes `TMDB returned <status>`; all surface as HTTP 502. The body cap (`json_within_cap`, 16 MiB) is also the client's.
4. **Partition ids** — results with `media_type` `tv` or `movie` are collected into two id lists (`tv_ids`, `movie_ids`); everything else (persons, missing type) is ignored.
5. **Look up tracked ids** — two IN-list queries, `tracked_tmdb_ids_in` against `shows` and `tracked_movie_tmdb_ids_in` against `movies` (backend/src/db/queries.rs). Both short-circuit to no query when their list is empty (the `ids.is_empty()` guard in each).
6. **Shape results** — the `filter_map` in `search_shows`: TV uses `name` + `first_air_date`; movies use `title` + `release_date`. Empty strings for `overview`, `date`, and `poster_path` become `null`; `poster_url` is prefixed with the w185 poster base (`POSTER_BASE`). TMDB's order is preserved; there is no ranking, deduplication, or pagination, so only TMDB's first page is ever returned.

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

- **Input** — a `type="search"` field, auto-focused on mount with an explicit lint exemption, labelled "Search movies and TV shows" (the `<input>` in `Search`'s JSX).
- **Debounce** — the search `useEffect` trims the query; a blank query clears results, error, and loading immediately with no request. A non-blank query starts a 350 ms `setTimeout`; when it fires, `loading` is set and `api.search` runs. The effect's cleanup clears the timer and sets a `cancelled` flag so a late response is ignored; there is no `AbortController`, so the HTTP request itself is not cancelled.
- **Result arrival** — a successful response (the `.then` in the search `useEffect`) replaces `results` and resets all per-card add state (`addStates`, `addErrors`). Previous results remain on screen while a new search is in flight; "Searching…" renders above the `results-grid` list.
- **Status lines** — "Searching…" while loading; `Error: <message>` on failure ("Search failed" when the rejection is not an `Error`); "No matches." when a non-blank query produced zero results and nothing is loading or errored (the `status` paragraphs in `Search`'s JSX).
- **Cards** — poster or "No poster" placeholder, name with `(YYYY)` from the first four characters of `date`, a `Movie`/`TV` pill, overview when present, then the action area (the `result-card` list item in `Search`'s JSX).
- **Add flow** — per-card state keyed `media_type:tmdb_id` (`stateKey`). Clicking **Add** (`handleAdd`) sets `pending` ("Adding…", disabled, in `buttonFor`), calls `addMovie` for movies and `addShow` for everything else, then `added` on success or `error` with the message shown beneath the actions ("Add failed" for non-`Error` rejections; the `addErrors[key]` paragraph). A card whose server flag `already_tracked` is true, or whose local state is `added`, renders the **On watchlist** pill instead of a button (`buttonFor`). On `error` the Add button returns so the user can retry.
- **No navigation** — result cards do not link to detail pages; adding keeps the user on Search.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|----------|--------|------------------------|-----------|
| Where the TMDB call happens | Backend proxy; browser calls `/api/v1/search` | Browser calls TMDB directly with a public key | The API key must never reach the browser (SECURITY.md, "What the app does protect"; README.md feature list). |
| TMDB endpoint | One `search/multi` request | Separate `search/tv` and `search/movie` requests merged client- or server-side | `[inferred]` One round trip and TMDB's own cross-type ranking; the cost is filtering out `person` results. |
| `already_tracked` computation | Server-side, two IN-list queries per response | Client compares results against a cached watchlist | `[inferred]` Keeps the page stateless and correct across tabs; the lists are bounded by TMDB's page size. |
| Adult content | `include_adult=false` hard-coded | Configurable | `[inferred]` Personal homelab tracker; no setting surface exists. |
| Empty-string normalization | `overview`, `date`, `poster_path` empty → `null` | Pass TMDB strings through | `[inferred]` Lets the page use truthiness checks; TMDB uses `""` for unknown dates. |
| Result paging | First TMDB page only, TMDB order | Expose `page`, or fetch several pages | `[inferred]` A name search rarely needs more than the top 20; no UI for paging. |
| Add interaction | Stay on Search with per-card state | Navigate to the new detail page after add | `[inferred]` Supports adding several titles from one query. |
| Debounce | 350 ms after the last keystroke, no abort | Submit button; abortable fetch | `[inferred]` Balances TMDB call volume against responsiveness; a stale response is ignored via the `cancelled` flag. |

## Open Questions & Future Decisions

### Resolved
*(none yet)*

### Deferred
1. **Cap `q` length.** Currently bounded only by URI limits.
2. **Link results to detail pages.** Cards are not links today; is that deliberate?
3. **Abort in-flight searches** with `AbortController` instead of only ignoring late results.
4. **Add-error styling.** Per-card add errors omit the `Error:` prefix every other page uses (the `addErrors[key]` paragraph in Search.tsx). Intentional, or drift?
5. **Poster base constant** duplicated in search.rs rather than reusing `models::show::poster_url`.

## References

- backend/src/api/search.rs
- frontend/src/pages/Search.tsx
- backend/tests/api.rs (`search_returns_mixed_results_with_already_tracked_flag`, `search_rejects_blank_query`, `search_returns_502_with_unavailable_message_on_tmdb_5xx`, `search_returns_502_with_rate_limit_message_on_tmdb_429`, `search_returns_502_with_raw_status_on_other_tmdb_errors`)
- frontend/src/pages/Search.test.tsx
- Consumed: `shows` (add-show endpoint, `tracked_tmdb_ids_in`), `movies` (add-movie endpoint, `tracked_movie_tmdb_ids_in`), `tmdb` (`search_multi`, status mapping, body cap)
- SECURITY.md, "What the app does protect" — key never reaches the browser
