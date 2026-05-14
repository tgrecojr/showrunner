import { useEffect, useState } from 'react';
import { api } from '../api/client';
import type { SearchResult } from '../types';

type AddState = 'idle' | 'pending' | 'added' | 'error';

export default function Search() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [addStates, setAddStates] = useState<Record<string, AddState>>({});
  const [addErrors, setAddErrors] = useState<Record<string, string>>({});

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

  // Keyed by media_type:tmdb_id so a movie and TV show with the same TMDB id
  // (unlikely but possible) don't share state.
  const stateKey = (s: SearchResult) => `${s.media_type}:${s.tmdb_id}`;

  async function handleAdd(s: SearchResult) {
    const key = stateKey(s);
    setAddStates((prev) => ({ ...prev, [key]: 'pending' }));
    setAddErrors((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
    try {
      if (s.media_type === 'movie') {
        await api.addMovie(s.tmdb_id);
      } else {
        await api.addShow(s.tmdb_id);
      }
      setAddStates((prev) => ({ ...prev, [key]: 'added' }));
    } catch (err) {
      setAddStates((prev) => ({ ...prev, [key]: 'error' }));
      setAddErrors((prev) => ({
        ...prev,
        [key]: err instanceof Error ? err.message : 'Add failed',
      }));
    }
  }

  function buttonFor(s: SearchResult) {
    const state = addStates[stateKey(s)] ?? 'idle';
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
        onClick={() => handleAdd(s)}
      >
        Add
      </button>
    );
  }

  return (
    <div className="search-page">
      <h1>Search</h1>
      <input
        type="search"
        autoFocus
        placeholder="Search movies & TV shows..."
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        className="search-input"
        aria-label="Search movies and TV shows"
      />

      {loading && <p className="status">Searching…</p>}
      {error && <p className="status status-error">Error: {error}</p>}
      {!loading && !error && query.trim() !== '' && results.length === 0 && (
        <p className="status">No matches.</p>
      )}

      <ul className="results-grid">
        {results.map((s) => {
          const key = stateKey(s);
          return (
            <li key={key} className="result-card">
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
                  {s.date && (
                    <span className="year"> ({s.date.slice(0, 4)})</span>
                  )}
                </h3>
                <div className="meta-row">
                  <span className="status-pill">
                    {s.media_type === 'movie' ? 'Movie' : 'TV'}
                  </span>
                </div>
                {s.overview && <p className="overview">{s.overview}</p>}
                <div className="card-actions">{buttonFor(s)}</div>
                {addErrors[key] && (
                  <p className="status status-error">{addErrors[key]}</p>
                )}
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
