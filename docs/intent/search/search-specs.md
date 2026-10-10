# Search — EARS specs

Prefix `SEARCH`. Facets: `API` (the `GET /api/v1/search` handler) and `UI` (the Search page). Statuses: `[x]` observed working in code and tests, `[ ]` specified but not implemented, `[D]` deferred.

## API

- [x] **SEARCH-API-001**: When `GET /api/v1/search` is called with a `q` whose trimmed value is non-empty, the system shall request TMDB `/search/multi` with `query` set to the trimmed value and `include_adult=false`.
- [x] **SEARCH-API-002**: If `q` is absent or blank after trimming, then the system shall respond 400 with the error `query parameter `q` is required` and make no TMDB request.
- [x] **SEARCH-API-003**: The system shall send the TMDB API key only on the server-side request to TMDB, and the search response body shall not contain it.
- [x] **SEARCH-API-004**: The system shall include in the search response only TMDB results whose `media_type` is `tv` or `movie`, dropping `person` and any other or missing type.
- [x] **SEARCH-API-005**: For a `tv` result the system shall take `name` from TMDB `name` and `date` from `first_air_date`; for a `movie` result it shall take `name` from `title` and `date` from `release_date`.
- [x] **SEARCH-API-006**: The system shall set `already_tracked` to true for a `tv` result whose id is on the show watchlist and for a `movie` result whose id is on the movie list, checking each media type against its own table.
- [x] **SEARCH-API-007**: The system shall return `overview`, `date`, and `poster_url` as `null` when TMDB supplies an empty string, and shall build `poster_url` by prefixing `poster_path` with the w185 TMDB image base.
- [x] **SEARCH-API-008**: If TMDB responds to a search with a non-2xx status other than 429 or 5xx, then the system shall respond 502 with the error `TMDB returned <status>`.
- [x] **SEARCH-API-009**: The system shall return search results in TMDB's order, from TMDB's first result page only, without ranking, deduplication, or pagination of its own.
- [x] **SEARCH-API-010**: The system shall make the search request through the TMDB client, so that a 429 or 5xx from TMDB surfaces as HTTP 502 carrying the user-readable sentence defined by TMDB-ERR-002 or TMDB-ERR-003 rather than the raw status text.

## UI

- [x] **SEARCH-UI-001**: The Search page shall focus the search input on load and expose it with the accessible label "Search movies and TV shows".
- [x] **SEARCH-UI-002**: When the trimmed query is non-empty, the Search page shall wait 350 ms after the last change before requesting results, cancelling any earlier pending timer.
- [x] **SEARCH-UI-003**: When the query becomes blank, the Search page shall clear results and any error immediately without making a request.
- [x] **SEARCH-UI-004**: While a search request is in flight, the Search page shall show "Searching…" and keep the previous results visible.
- [x] **SEARCH-UI-005**: When a search for a non-blank query completes with zero results, the Search page shall show "No matches."
- [x] **SEARCH-UI-006**: If the search request fails, then the Search page shall show `Error: <message>`, using "Search failed" when the rejection carries no message.
- [x] **SEARCH-UI-007**: The Search page shall render each result as a card with its poster (or a "No poster" placeholder), its name followed by the year taken from the first four characters of `date` when present, a `Movie` or `TV` pill, and the overview when present.
- [x] **SEARCH-UI-008**: While a result is flagged `already_tracked` by the server or was added during the current result set, the Search page shall show an "On watchlist" pill in place of the Add button.
- [x] **SEARCH-UI-009**: When the user clicks Add on a `movie` result, the Search page shall call the add-movie endpoint; on a `tv` result it shall call the add-show endpoint.
- [x] **SEARCH-UI-010**: While an add request is pending, the Search page shall replace that card's Add button with a disabled "Adding…" button, leaving other cards interactive.
- [x] **SEARCH-UI-011**: If an add request fails, then the Search page shall show the failure message beneath that card's actions ("Add failed" when the rejection carries no message) and restore the Add button.
- [x] **SEARCH-UI-012**: The Search page shall key per-result add state by `media_type:tmdb_id` and reset all add state whenever a new result set arrives.
