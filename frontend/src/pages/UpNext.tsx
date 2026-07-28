import { useEffect, useState } from 'react';
import { Link } from 'react-router';
import { api } from '../api/client';
import type { UpNextItem } from '../types';

function pad(n: number): string {
  return String(n).padStart(2, '0');
}

export default function UpNext() {
  const [items, setItems] = useState<UpNextItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [advancing, setAdvancing] = useState<number | null>(null);

  async function load() {
    setError(null);
    try {
      const res = await api.upNext();
      setItems(res.items);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Load failed');
    }
  }

  useEffect(() => {
    let cancelled = false;
    api
      .upNext()
      .then((res) => {
        if (!cancelled) setItems(res.items);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Load failed');
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function markWatched(item: UpNextItem) {
    setAdvancing(item.show_tmdb_id);
    setError(null);
    try {
      await api.setEpisodeWatched(
        item.show_tmdb_id,
        item.season_number,
        item.episode_number,
        true,
      );
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Update failed');
    } finally {
      setAdvancing(null);
    }
  }

  if (error && items === null)
    return <p className="status status-error">Error: {error}</p>;
  if (items === null) return <p className="status">Loading…</p>;

  if (items.length === 0) {
    return (
      <div>
        <h1>Up Next</h1>
        <p>
          You're all caught up. Add more shows from{' '}
          <Link to="/search">Search</Link> or check back when new episodes air.
        </p>
      </div>
    );
  }

  return (
    <div className="upnext-page">
      <h1>Up Next</h1>
      <p className="status">
        {items.length} show{items.length === 1 ? '' : 's'} with unwatched aired
        episodes. Sorted by oldest unwatched first.
      </p>
      {error && <p className="status status-error">Error: {error}</p>}

      <ul className="upnext-list">
        {items.map((item) => (
          <li key={item.show_tmdb_id} className="upnext-row">
            <Link to={`/shows/${item.show_tmdb_id}`} className="upnext-poster-link">
              {item.poster_url ? (
                <img
                  src={item.poster_url}
                  alt={`${item.show_name} poster`}
                  className="upnext-poster"
                />
              ) : (
                <div className="upnext-poster poster-placeholder">No poster</div>
              )}
            </Link>
            <div className="upnext-body">
              <h3>
                <Link
                  to={`/shows/${item.show_tmdb_id}`}
                  className="card-title-link"
                >
                  {item.show_name}
                </Link>
              </h3>
              <div className="meta-row">
                <span className="ep-num">
                  S{pad(item.season_number)}E{pad(item.episode_number)}
                </span>
                {item.episode_name && (
                  <span className="upnext-ep-name">{item.episode_name}</span>
                )}
              </div>
              {item.episode_overview && (
                <p className="upnext-ep-overview">{item.episode_overview}</p>
              )}
              <div className="meta-row">
                <span className="status-pill">
                  {item.remaining} remaining
                </span>
                <span className="ep-date">aired {item.air_date}</span>
              </div>
            </div>
            <div className="upnext-action">
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => markWatched(item)}
                disabled={advancing === item.show_tmdb_id}
              >
                {advancing === item.show_tmdb_id ? 'Marking…' : 'Mark watched'}
              </button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
