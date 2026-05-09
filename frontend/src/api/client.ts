import type {
  CalendarResponse,
  HealthResponse,
  SearchResponse,
  ShowDetail,
  SyncResponse,
  TestNotificationResponse,
  UpNextResponse,
  WatchlistItem,
  WatchlistResponse,
} from '../types';

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api/v1${path}`, {
    headers: { 'Content-Type': 'application/json' },
    ...init,
  });
  if (!res.ok) {
    const body = await res.text();
    let message = body;
    try {
      const parsed = JSON.parse(body) as { error?: string };
      if (parsed.error) message = parsed.error;
    } catch {
      // body wasn't JSON; use raw text
    }
    throw new Error(`API ${res.status}: ${message}`);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}

export const api = {
  health: () => request<HealthResponse>('/health'),
  search: (q: string) =>
    request<SearchResponse>(`/search?q=${encodeURIComponent(q)}`),
  listShows: () => request<WatchlistResponse>('/shows'),
  upNext: () => request<UpNextResponse>('/up-next'),
  addShow: (tmdb_id: number) =>
    request<WatchlistItem>('/shows', {
      method: 'POST',
      body: JSON.stringify({ tmdb_id }),
    }),
  getShow: (tmdb_id: number) =>
    request<ShowDetail>(`/shows/${tmdb_id}`),
  deleteShow: (tmdb_id: number) =>
    request<void>(`/shows/${tmdb_id}`, { method: 'DELETE' }),
  setEpisodeWatched: (
    tmdb_id: number,
    season_number: number,
    episode_number: number,
    watched: boolean,
  ) =>
    request<ShowDetail>(
      `/episodes/${tmdb_id}/${season_number}/${episode_number}`,
      {
        method: 'PATCH',
        body: JSON.stringify({ watched }),
      },
    ),
  bulkWatch: (tmdb_id: number, scope: BulkWatchScope, watched: boolean) =>
    request<ShowDetail>(`/shows/${tmdb_id}/bulk-watch`, {
      method: 'POST',
      body: JSON.stringify({ scope, watched }),
    }),
  calendar: (start: string, end: string) =>
    request<CalendarResponse>(
      `/calendar?start=${start}&end=${end}`,
    ),
  sync: () => request<SyncResponse>('/sync', { method: 'POST' }),
  testNotification: (message?: string) =>
    request<TestNotificationResponse>('/notifications/test', {
      method: 'POST',
      body: JSON.stringify({ message: message ?? null }),
    }),
};

export type BulkWatchScope =
  | { type: 'all' }
  | { type: 'season'; season_number: number }
  | {
      type: 'through_episode';
      season_number: number;
      episode_number: number;
    };
