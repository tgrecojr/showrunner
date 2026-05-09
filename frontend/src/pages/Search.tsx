import { useEffect, useState } from 'react';
import { api } from '../api/client';
import type { SearchResult } from '../types';

type AddState = 'idle' | 'pending' | 'added' | 'error';

export default function Search() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [addStates, setAddStates] = useState<Record<number, AddState>>({});
  const [addErrors, setAddErrors] = useState<Record<number, string>>({});

  useEffect(() => {
    const term = query.trim();
    if (term.length === 0) {
      setResults([]);
      setError(null);
      setLoading(false);
      return;
    }

    let cancelled = false;
    const handle = setTimeout(() => {
      setLoading(true);
      setError(null);
      api
        .search(term)
        .then((res) => {
          if (!cancelled) {
            setResults(res.results);
            setAddStates({});
            setAddErrors({});
          }
        })
        .catch((err: unknown) => {
          if (!cancelled) {
            setError(err instanceof Error ? err.message : 'Search failed');
          }
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    }, 350);

    return () => {
      cancelled = true;
      clearTimeout(handle);
    };
  }, [query]);

  async function handleAdd(tmdbId: number) {
    setAddStates((prev) => ({ ...prev, [tmdbId]: 'pending' }));
    setAddErrors((prev) => {
      const next = { ...prev };
      delete next[tmdbId];
      return next;
    });
    try {
      await api.addShow(tmdbId);
      setAddStates((prev) => ({ ...prev, [tmdbId]: 'added' }));
    } catch (err) {
      setAddStates((prev) => ({ ...prev, [tmdbId]: 'error' }));
      setAddErrors((prev) => ({
        ...prev,
        [tmdbId]: err instanceof Error ? err.message : 'Add failed',
      }));
    }
  }

  function buttonFor(s: SearchResult) {
    const state = addStates[s.tmdb_id] ?? 'idle';
    if (s.already_tracked || state === 'added') {
      return (
        <span className="status-pill status-pill-success">On watchlist</span>
      );
    }
    if (state === 'pending') {
      return (
        <button type="button" className="btn" disabled>
          Adding…
        </button>
      );
    }
    return (
      <button
        type="button"
        className="btn btn-primary"
        onClick={() => handleAdd(s.tmdb_id)}
      >
        Add
      </button>
    );
  }

  return (
    <div className="search-page">
      <h1>Search TV Shows</h1>
      <input
        type="search"
        autoFocus
        placeholder="The Bear, Severance, Game of Thrones..."
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        className="search-input"
        aria-label="Search shows"
      />

      {loading && <p className="status">Searching…</p>}
      {error && <p className="status status-error">Error: {error}</p>}
      {!loading && !error && query.trim() !== '' && results.length === 0 && (
        <p className="status">No matches.</p>
      )}

      <ul className="results-grid">
        {results.map((s) => (
          <li key={s.tmdb_id} className="result-card">
            {s.poster_url ? (
              <img
                src={s.poster_url}
                alt={`${s.name} poster`}
                className="poster"
              />
            ) : (
              <div className="poster poster-placeholder">No poster</div>
            )}
            <div className="result-body">
              <h3>
                {s.name}
                {s.first_air_date && (
                  <span className="year">
                    {' '}
                    ({s.first_air_date.slice(0, 4)})
                  </span>
                )}
              </h3>
              {s.overview && <p className="overview">{s.overview}</p>}
              <div className="card-actions">{buttonFor(s)}</div>
              {addErrors[s.tmdb_id] && (
                <p className="status status-error">
                  {addErrors[s.tmdb_id]}
                </p>
              )}
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
