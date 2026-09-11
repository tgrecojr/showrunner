-- Append-only record of every user "watched" / "unwatched" action, surfaced on
-- the History page so accidental marks can be spotted. Rows snapshot the title
-- and poster and carry no foreign keys: history must outlive the show or movie
-- it refers to (marking a movie watched deletes its row).

CREATE TABLE watch_log (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at    TEXT    NOT NULL,            -- UTC RFC3339
    media_type     TEXT    NOT NULL,            -- 'tv' | 'movie'
    action         TEXT    NOT NULL,            -- 'watched' | 'unwatched'
    scope          TEXT    NOT NULL,            -- 'episode' | 'season' | 'show' | 'through_episode' | 'movie'
    tmdb_id        INTEGER NOT NULL,            -- show or movie id
    title          TEXT    NOT NULL,            -- snapshot of show/movie name
    poster_path    TEXT,
    season_number  INTEGER,
    episode_number INTEGER,
    episode_name   TEXT,
    episode_count  INTEGER NOT NULL DEFAULT 1   -- episodes whose state changed
);

CREATE INDEX idx_watch_log_order ON watch_log(occurred_at DESC, id DESC);
