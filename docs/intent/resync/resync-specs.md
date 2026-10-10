# Resync — EARS specs

Prefix `RESYNC`. Facets: `SHOW` (what one show's resync does), `RUN` (a full run and its bounds), `TRIGGER` (cron and manual endpoint), `UI` (Settings page).

## Show

- [x] **RESYNC-SHOW-001**: When a show is resynced, the system shall fetch it from TMDB and update its name, `in_production`, US watch providers, network names, and `last_synced_at` (current UTC time), and update its overview, poster path, backdrop path, status, first air date, and last air date, storing an empty string in any of those six as NULL.
- [x] **RESYNC-SHOW-002**: When a show is resynced, the system shall fetch every season with `season_number > 0` and insert or update its name, overview, air date, and `episode_count` (the number of episodes fetched); season 0 shall be neither fetched nor stored.
- [x] **RESYNC-SHOW-003**: When a show is resynced, the system shall insert each fetched episode that is new and update `tmdb_id`, `name`, `overview`, `air_date`, and `runtime` on each that exists, and shall never modify `watched` or `watched_at` on an existing episode.
- [x] **RESYNC-SHOW-004**: Resync shall never delete a season or episode row, including ones TMDB no longer returns.
- [x] **RESYNC-SHOW-005**: Resync shall never create, modify, or delete movie rows.

## Run

- [x] **RESYNC-RUN-001**: A full resync shall resync at most 100 eligible shows per run, one show at a time with at most one TMDB request in flight.
- [x] **RESYNC-RUN-002**: When more than 100 shows are eligible, a full resync shall log a warning carrying the eligible total, the limit, and the number skipped.
- [x] **RESYNC-RUN-003**: If resyncing one show fails, then the run shall record `{ tmdb_id, message }` for that show using the error's client-safe message and continue with the remaining shows; the run shall fail as a whole only if the tracked-show list cannot be read.
- [x] **RESYNC-RUN-004**: A full resync shall report `shows_synced` (count of shows that completed) and `errors` (one entry per failed show).
- [x] **RESYNC-RUN-005**: A full resync shall order eligible shows by `last_synced_at` ascending with NULL first and then by name, and take the first 100, so that when more eligible shows exist than the per-run ceiling, consecutive runs rotate through every eligible show.
- [x] **RESYNC-RUN-006**: A full resync shall treat a show as eligible when its stored TMDB `status` is neither `Ended` nor `Canceled` (a NULL or any other status being eligible), or when its `last_synced_at` is more than 30 days before the current time; the per-run ceiling and the rotation in RESYNC-RUN-005 apply to the eligible set only.

## Trigger

- [x] **RESYNC-TRIGGER-001**: On startup, the system shall schedule one job from the six-field `RESYNC_CRON` expression (default `0 0 6 * * *`), evaluated in the configured `TIMEZONE`, that performs a full resync and logs its report.
- [x] **RESYNC-TRIGGER-002**: If `RESYNC_CRON` cannot be parsed, then startup shall fail with a configuration error whose message names `RESYNC_CRON`.
- [x] **RESYNC-TRIGGER-003**: When `POST /api/v1/sync` is called and no manual sync has started on this server instance within the last 60 seconds, the system shall perform a full resync and respond 200 with `{ shows_synced, errors: [{ tmdb_id, message }] }`, including when every show failed.
- [x] **RESYNC-TRIGGER-004**: If `POST /api/v1/sync` is called within 60 seconds of a previous manual sync starting on the same server instance, then the system shall respond 429 with the error `a sync ran less than 60s ago; try again shortly` and start no resync.
- [x] **RESYNC-TRIGGER-005**: The manual-sync cooldown shall be tracked per application state instance, not process-globally, and the check-and-record shall be atomic so concurrent callers cannot both pass.
- [x] **RESYNC-TRIGGER-006**: Each `message` in the `POST /api/v1/sync` response shall be the client-safe form of the error, collapsing database, HTTP, IO, and JSON errors to `An internal error occurred`.

## UI

- [x] **RESYNC-UI-001**: The Settings page shall show a "TMDB sync" section stating that a run refreshes shows still airing or expected to return with ended shows checked about monthly, that watched state is preserved, and that the job also runs on the configured cron schedule, with a button labelled "Resync all shows from TMDB".
- [x] **RESYNC-UI-002**: When the user clicks the resync button, the Settings page shall call `POST /api/v1/sync`, label the button "Syncing…" and disable it while in flight, and clear any previous result and error.
- [x] **RESYNC-UI-003**: When a manual sync completes, the Settings page shall show `Synced <n> show(s).`, append ` <m> failed.` when `errors` is non-empty, and list each error as `Show #<tmdb_id>: <message>`.
- [x] **RESYNC-UI-004**: If the manual sync request fails, then the Settings page shall show `Error: <message>` ("Sync failed" when the rejection carries no message).
