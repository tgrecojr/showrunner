import { useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { api } from '../api/client';
import type { BulkWatchScope } from '../api/client';
import type { ShowDetail as ShowDetailType } from '../types';

export default function ShowDetail() {
  const { tmdbId } = useParams<{ tmdbId: string }>();
  const navigate = useNavigate();
  const id = Number(tmdbId);

  const [show, setShow] = useState<ShowDetailType | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [openSeasons, setOpenSeasons] = useState<Set<number>>(new Set());
  const [removing, setRemoving] = useState(false);
  const [mutating, setMutating] = useState(false);

  useEffect(() => {
    if (!Number.isFinite(id)) {
      setError('Invalid show id');
      return;
    }
    let cancelled = false;
    api
      .getShow(id)
      .then((res) => {
        if (!cancelled) setShow(res);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Load failed');
        }
      });
    return () => {
      cancelled = true;
    };
  }, [id]);

  function toggleSeason(n: number) {
    setOpenSeasons((prev) => {
      const next = new Set(prev);
      if (next.has(n)) next.delete(n);
      else next.add(n);
      return next;
    });
  }

  async function handleRemove() {
    if (!show) return;
    if (!confirm(`Remove "${show.name}" from your watchlist?`)) return;
    setRemoving(true);
    try {
      await api.deleteShow(show.tmdb_id);
      navigate('/');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Delete failed');
      setRemoving(false);
    }
  }

  async function runMutation(promise: Promise<ShowDetailType>) {
    setMutating(true);
    setError(null);
    try {
      const next = await promise;
      setShow(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Update failed');
    } finally {
      setMutating(false);
    }
  }

  function toggleEpisode(seasonNumber: number, episodeNumber: number, watched: boolean) {
    if (!show) return;
    void runMutation(
      api.setEpisodeWatched(show.tmdb_id, seasonNumber, episodeNumber, watched),
    );
  }

  function bulk(scope: BulkWatchScope, watched: boolean) {
    if (!show) return;
    void runMutation(api.bulkWatch(show.tmdb_id, scope, watched));
  }

  if (error && !show)
    return <p className="status status-error">Error: {error}</p>;
  if (!show) return <p className="status">Loading…</p>;

  const totalWatched = show.seasons.reduce(
    (acc, s) => acc + s.watched_count,
    0,
  );
  const totalEpisodes = show.seasons.reduce(
    (acc, s) => acc + s.episode_count,
    0,
  );
  const allWatched = totalWatched > 0 && totalWatched === totalEpisodes;

  return (
    <div className="show-detail">
      {error && (
        <p className="status status-error">Error: {error}</p>
      )}

      <header className="show-header">
        {show.poster_url ? (
          <img
            src={show.poster_url}
            alt={`${show.name} poster`}
            className="poster poster-lg"
          />
        ) : (
          <div className="poster poster-lg poster-placeholder">No poster</div>
        )}
        <div className="show-meta">
          <h1>{show.name}</h1>
          <div className="meta-row">
            <span className="progress-chip">
              {totalWatched}/{totalEpisodes}
            </span>
            {show.status && <span className="status-pill">{show.status}</span>}
            {show.first_air_date && (
              <span className="year">
                {show.first_air_date.slice(0, 4)}
                {show.last_air_date && show.last_air_date !== show.first_air_date
                  ? `–${show.last_air_date.slice(0, 4)}`
                  : ''}
              </span>
            )}
          </div>
          {show.watch_providers.length > 0 && (
            <p className="providers">
              <strong>Watch on:</strong> {show.watch_providers.join(', ')}
            </p>
          )}
          {show.overview && <p>{show.overview}</p>}
          <div className="action-row">
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => bulk({ type: 'all' }, !allWatched)}
              disabled={mutating}
            >
              {allWatched ? 'Mark all unwatched' : 'Mark all watched'}
            </button>
            <button
              type="button"
              className="btn btn-danger"
              onClick={handleRemove}
              disabled={removing || mutating}
            >
              {removing ? 'Removing…' : 'Remove from watchlist'}
            </button>
          </div>
        </div>
      </header>

      <section className="seasons">
        <h2>Seasons</h2>
        {show.seasons.length === 0 && <p>No seasons available yet.</p>}
        {show.seasons.map((season) => {
          const open = openSeasons.has(season.season_number);
          const seasonAllWatched =
            season.watched_count === season.episode_count &&
            season.episode_count > 0;
          return (
            <div key={season.season_number} className="season-block">
              <div className="season-header">
                <button
                  type="button"
                  className="season-toggle"
                  onClick={() => toggleSeason(season.season_number)}
                  aria-expanded={open}
                >
                  <span className="caret">{open ? '▾' : '▸'}</span>
                  <span className="season-name">
                    {season.name ?? `Season ${season.season_number}`}
                  </span>
                  <span className="season-progress">
                    {season.watched_count}/{season.episode_count}
                  </span>
                </button>
                <button
                  type="button"
                  className="btn btn-sm"
                  onClick={() =>
                    bulk(
                      {
                        type: 'season',
                        season_number: season.season_number,
                      },
                      !seasonAllWatched,
                    )
                  }
                  disabled={mutating || season.episode_count === 0}
                >
                  {seasonAllWatched ? 'Mark unwatched' : 'Mark watched'}
                </button>
              </div>
              {open && (
                <ul className="episode-list">
                  {season.episodes.map((ep) => (
                    <li key={ep.episode_number} className="episode-row">
                      <label className="episode-label">
                        <input
                          type="checkbox"
                          checked={ep.watched}
                          disabled={mutating}
                          onChange={(e) =>
                            toggleEpisode(
                              season.season_number,
                              ep.episode_number,
                              e.target.checked,
                            )
                          }
                          aria-label={`Mark S${season.season_number}E${ep.episode_number} watched`}
                        />
                        <span className="ep-num">
                          S{String(season.season_number).padStart(2, '0')}E
                          {String(ep.episode_number).padStart(2, '0')}
                        </span>
                        <span className="ep-name">{ep.name ?? '—'}</span>
                        {ep.air_date && (
                          <span className="ep-date">{ep.air_date}</span>
                        )}
                        <button
                          type="button"
                          className="ep-through-btn"
                          onClick={(e) => {
                            e.preventDefault();
                            bulk(
                              {
                                type: 'through_episode',
                                season_number: season.season_number,
                                episode_number: ep.episode_number,
                              },
                              true,
                            );
                          }}
                          disabled={mutating}
                          title="Mark all episodes from start through this one as watched"
                        >
                          Mark through here
                        </button>
                      </label>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          );
        })}
      </section>
    </div>
  );
}
