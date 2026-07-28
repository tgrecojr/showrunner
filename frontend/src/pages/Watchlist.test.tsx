import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import Watchlist from './Watchlist';
import type { WatchlistItem } from '../types';

vi.mock('../api/client', () => ({
  api: { listShows: vi.fn() },
}));
import { api } from '../api/client';

const mockListShows = vi.mocked(api.listShows);

function buildItem(overrides: Partial<WatchlistItem> = {}): WatchlistItem {
  return {
    tmdb_id: 1,
    name: 'Show',
    poster_url: '/p.jpg',
    status: 'Returning',
    in_production: true,
    watched_count: 5,
    aired_count: 10,
    total_count: 12,
    next_episode_air_date: null,
    ...overrides,
  };
}

function renderPage() {
  return render(
    <MemoryRouter>
      <Watchlist />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mockListShows.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('Watchlist', () => {
  it('shows loading then renders shows with progress chip', async () => {
    mockListShows.mockResolvedValueOnce({
      shows: [
        buildItem({ tmdb_id: 1, name: 'Alpha', watched_count: 2, aired_count: 5 }),
        buildItem({
          tmdb_id: 2,
          name: 'Beta',
          poster_url: null,
          status: null,
          next_episode_air_date: '2026-06-01',
        }),
      ],
    });
    renderPage();
    expect(screen.getByText('Loading…')).toBeInTheDocument();

    await waitFor(() => expect(screen.getByText('Alpha')).toBeInTheDocument());
    expect(screen.getByText('Beta')).toBeInTheDocument();
    expect(screen.getByText('2/5')).toBeInTheDocument();
    expect(screen.getByText('Returning')).toBeInTheDocument();
    expect(screen.getByText('Next airs 2026-06-01')).toBeInTheDocument();

    // Both poster paths handled
    expect(screen.getByAltText('Alpha poster')).toHaveAttribute('src', '/p.jpg');
    expect(screen.getByText('No poster')).toBeInTheDocument();
  });

  it('shows empty state with link to Search when watchlist is empty', async () => {
    mockListShows.mockResolvedValueOnce({ shows: [] });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText(/Nothing tracked yet/)).toBeInTheDocument(),
    );
    expect(screen.getByRole('link', { name: 'Search for a show' })).toHaveAttribute(
      'href',
      '/search',
    );
  });

  it('shows error state when API call rejects', async () => {
    mockListShows.mockRejectedValueOnce(new Error('boom'));
    renderPage();
    await waitFor(() => expect(screen.getByText('Error: boom')).toBeInTheDocument());
  });

  it('shows generic error when rejection is not an Error', async () => {
    mockListShows.mockRejectedValueOnce('weird');
    renderPage();
    await waitFor(() =>
      expect(screen.getByText('Error: Load failed')).toBeInTheDocument(),
    );
  });
});
