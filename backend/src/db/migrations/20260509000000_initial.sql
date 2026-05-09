-- Showrunner initial schema

CREATE TABLE shows (
    tmdb_id              INTEGER PRIMARY KEY,
    name                 TEXT NOT NULL,
    overview             TEXT,
    poster_path          TEXT,
    backdrop_path        TEXT,
    status               TEXT,
    first_air_date       TEXT,
    last_air_date        TEXT,
    in_production        INTEGER NOT NULL DEFAULT 0,
    watch_providers_json TEXT,
    notify_new_episodes  INTEGER NOT NULL DEFAULT 1,
    added_at             TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_synced_at       TEXT
);

CREATE TABLE seasons (
    show_tmdb_id  INTEGER NOT NULL,
    season_number INTEGER NOT NULL,
    name          TEXT,
    overview      TEXT,
    air_date      TEXT,
    episode_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (show_tmdb_id, season_number),
    FOREIGN KEY (show_tmdb_id) REFERENCES shows(tmdb_id) ON DELETE CASCADE
);

CREATE TABLE episodes (
    show_tmdb_id   INTEGER NOT NULL,
    season_number  INTEGER NOT NULL,
    episode_number INTEGER NOT NULL,
    tmdb_id        INTEGER,
    name           TEXT,
    overview       TEXT,
    air_date       TEXT,
    runtime        INTEGER,
    watched        INTEGER NOT NULL DEFAULT 0,
    watched_at     TEXT,
    PRIMARY KEY (show_tmdb_id, season_number, episode_number),
    FOREIGN KEY (show_tmdb_id, season_number)
        REFERENCES seasons(show_tmdb_id, season_number) ON DELETE CASCADE
);

CREATE INDEX idx_episodes_air_date ON episodes(air_date);
CREATE INDEX idx_episodes_show ON episodes(show_tmdb_id);

CREATE TABLE notification_log (
    show_tmdb_id   INTEGER NOT NULL,
    season_number  INTEGER NOT NULL,
    episode_number INTEGER NOT NULL,
    channel        TEXT NOT NULL,
    sent_at        TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (show_tmdb_id, season_number, episode_number, channel),
    FOREIGN KEY (show_tmdb_id) REFERENCES shows(tmdb_id) ON DELETE CASCADE
);
