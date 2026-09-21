-- Networks a show airs on (TMDB `networks`: broadcast/cable channels and
-- streaming originals alike), stored as a JSON array of names. Surfaced on the
-- Up Next page so it's obvious where to go watch. NULL until the show is next
-- resynced.

ALTER TABLE shows ADD COLUMN networks_json TEXT;
