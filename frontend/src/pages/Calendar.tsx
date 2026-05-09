import { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../api/client';
import type { CalendarEpisode } from '../types';

const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const MONTHS = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];

function pad(n: number): string {
  return String(n).padStart(2, '0');
}

function isoDate(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

interface VisibleRange {
  cells: Date[];      // 42 cells (6 weeks × 7 days)
  startIso: string;   // first cell ISO
  endIso: string;     // last cell ISO
}

function buildVisibleRange(year: number, month: number): VisibleRange {
  const firstOfMonth = new Date(year, month, 1);
  const startWeekday = firstOfMonth.getDay(); // 0 = Sun

  const gridStart = new Date(year, month, 1 - startWeekday);
  const cells: Date[] = [];
  for (let i = 0; i < 42; i++) {
    const d = new Date(gridStart);
    d.setDate(gridStart.getDate() + i);
    cells.push(d);
  }
  return {
    cells,
    startIso: isoDate(cells[0]!),
    endIso: isoDate(cells[cells.length - 1]!),
  };
}

export default function Calendar() {
  const today = useMemo(() => new Date(), []);
  const [year, setYear] = useState(today.getFullYear());
  const [month, setMonth] = useState(today.getMonth());

  const [episodes, setEpisodes] = useState<CalendarEpisode[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const range = useMemo(() => buildVisibleRange(year, month), [year, month]);
  const todayIso = useMemo(() => isoDate(today), [today]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    api
      .calendar(range.startIso, range.endIso)
      .then((res) => {
        if (!cancelled) setEpisodes(res.episodes);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Load failed');
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [range.startIso, range.endIso]);

  const episodesByDate = useMemo(() => {
    const map = new Map<string, CalendarEpisode[]>();
    for (const ep of episodes) {
      const list = map.get(ep.air_date) ?? [];
      list.push(ep);
      map.set(ep.air_date, list);
    }
    return map;
  }, [episodes]);

  function goPrev() {
    if (month === 0) {
      setYear(year - 1);
      setMonth(11);
    } else {
      setMonth(month - 1);
    }
  }

  function goNext() {
    if (month === 11) {
      setYear(year + 1);
      setMonth(0);
    } else {
      setMonth(month + 1);
    }
  }

  function goToday() {
    setYear(today.getFullYear());
    setMonth(today.getMonth());
  }

  return (
    <div className="calendar-page">
      <div className="calendar-toolbar">
        <h1>{MONTHS[month]} {year}</h1>
        <div className="calendar-nav">
          <button type="button" className="btn btn-sm" onClick={goPrev}>
            ‹ Prev
          </button>
          <button type="button" className="btn btn-sm" onClick={goToday}>
            Today
          </button>
          <button type="button" className="btn btn-sm" onClick={goNext}>
            Next ›
          </button>
        </div>
      </div>

      {error && <p className="status status-error">Error: {error}</p>}

      <div className="calendar-grid">
        {WEEKDAYS.map((d) => (
          <div key={d} className="calendar-weekday">{d}</div>
        ))}
        {range.cells.map((date, i) => {
          const iso = isoDate(date);
          const inMonth = date.getMonth() === month;
          const isToday = iso === todayIso;
          const dayEpisodes = episodesByDate.get(iso) ?? [];
          return (
            <div
              key={i}
              className={[
                'calendar-cell',
                inMonth ? '' : 'calendar-cell-other',
                isToday ? 'calendar-cell-today' : '',
              ]
                .filter(Boolean)
                .join(' ')}
            >
              <div className="calendar-day-num">{date.getDate()}</div>
              <ul className="calendar-eps">
                {dayEpisodes.map((ep) => (
                  <li
                    key={`${ep.show_tmdb_id}-${ep.season_number}-${ep.episode_number}`}
                    className={`calendar-ep ${ep.watched ? 'calendar-ep-watched' : ''}`}
                  >
                    <Link
                      to={`/shows/${ep.show_tmdb_id}`}
                      title={ep.episode_name ?? ''}
                      className="calendar-ep-link"
                    >
                      {ep.poster_url && (
                        <img
                          src={ep.poster_url}
                          alt=""
                          className="calendar-ep-poster"
                        />
                      )}
                      <span className="calendar-ep-text">
                        <span className="calendar-ep-show">{ep.show_name}</span>
                        <span className="calendar-ep-code">
                          S{pad(ep.season_number)}E{pad(ep.episode_number)}
                        </span>
                      </span>
                    </Link>
                  </li>
                ))}
              </ul>
            </div>
          );
        })}
      </div>

      {loading && <p className="status">Loading…</p>}
    </div>
  );
}
