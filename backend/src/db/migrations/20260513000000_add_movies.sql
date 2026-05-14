-- Movies watchlist: flat list, no seasons/episodes. Row existence == "on the
-- watchlist & unwatched". Marking a movie watched deletes it.

CREATE TABLE movies (
    tmdb_id        INTEGER PRIMARY KEY,
    name           TEXT    NOT NULL,
    overview       TEXT,
    poster_path    TEXT,
    backdrop_path  TEXT,
    release_date   TEXT,
    runtime        INTEGER,
    added_at       TEXT    NOT NULL DEFAULT CURRENT_TIMESTAMP
);
