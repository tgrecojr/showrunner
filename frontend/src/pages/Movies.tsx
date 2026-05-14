import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../api/client';
import type { MovieWatchlistItem } from '../types';

export default function Movies() {
  const [movies, setMovies] = useState<MovieWatchlistItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<Record<number, boolean>>({});

  useEffect(() => {
    let cancelled = false;
    api
      .listMovies()
      .then((res) => {
        if (!cancelled) setMovies(res.movies);
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

  async function removeMovie(tmdbId: number, message: string) {
    if (!confirm(message)) return;
    setPending((prev) => ({ ...prev, [tmdbId]: true }));
    try {
      await api.deleteMovie(tmdbId);
      setMovies((prev) =>
        prev ? prev.filter((m) => m.tmdb_id !== tmdbId) : prev,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Update failed');
      setPending((prev) => {
        const next = { ...prev };
        delete next[tmdbId];
        return next;
      });
    }
  }

  if (error) {
    return (
      <div>
        <h1>Movies</h1>
        <p className="status status-error">Error: {error}</p>
      </div>
    );
  }

  if (movies === null) {
    return (
      <div>
        <h1>Movies</h1>
        <p className="status">Loading…</p>
      </div>
    );
  }

  if (movies.length === 0) {
    return (
      <div>
        <h1>Movies</h1>
        <p>
          No movies yet. <Link to="/search">Search for a movie</Link> to add
          one.
        </p>
      </div>
    );
  }

  return (
    <div>
      <h1>Movies</h1>
      <ul className="results-grid">
        {movies.map((m) => {
          const isPending = !!pending[m.tmdb_id];
          const year = m.release_date ? m.release_date.slice(0, 4) : null;
          return (
            <li key={m.tmdb_id} className="result-card">
              {m.poster_url ? (
                <img
                  src={m.poster_url}
                  alt={`${m.name} poster`}
                  className="poster"
                />
              ) : (
                <div className="poster poster-placeholder">No poster</div>
              )}
              <div className="result-body">
                <h3>
                  {m.name}
                  {year && <span className="year"> ({year})</span>}
                </h3>
                <div className="action-row">
                  <button
                    type="button"
                    className="btn btn-primary btn-sm"
                    disabled={isPending}
                    onClick={() =>
                      removeMovie(
                        m.tmdb_id,
                        `Mark "${m.name}" as watched?`,
                      )
                    }
                  >
                    Mark Watched
                  </button>
                  <button
                    type="button"
                    className="btn btn-danger btn-sm"
                    disabled={isPending}
                    onClick={() =>
                      removeMovie(
                        m.tmdb_id,
                        `Remove "${m.name}" from your list?`,
                      )
                    }
                  >
                    Remove
                  </button>
                </div>
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
