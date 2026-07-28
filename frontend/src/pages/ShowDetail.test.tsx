import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes } from 'react-router';
import ShowDetail from './ShowDetail';
import type { ShowDetail as ShowDetailType } from '../types';

vi.mock('../api/client', () => ({
  api: {
    getShow: vi.fn(),
    deleteShow: vi.fn(),
    setEpisodeWatched: vi.fn(),
    bulkWatch: vi.fn(),
  },
}));
import { api } from '../api/client';

const mockGet = vi.mocked(api.getShow);
const mockDelete = vi.mocked(api.deleteShow);
const mockSet = vi.mocked(api.setEpisodeWatched);
const mockBulk = vi.mocked(api.bulkWatch);

function show(overrides: Partial<ShowDetailType> = {}): ShowDetailType {
  return {
    tmdb_id: 1,
    name: 'My Show',
    overview: 'A great show',
    poster_url: '/p.jpg',
    backdrop_url: '/b.jpg',
    status: 'Returning',
    in_production: true,
    first_air_date: '2022-01-01',
    last_air_date: '2024-01-01',
    watch_providers: ['Hulu'],
    seasons: [
      {
        season_number: 1,
        name: 'Season 1',
        overview: null,
        air_date: null,
        episode_count: 2,
        watched_count: 0,
        episodes: [
          {
            episode_number: 1,
            name: 'Pilot',
            overview: null,
            air_date: '2022-01-01',
            runtime: 30,
            watched: false,
          },
          {
            episode_number: 2,
            name: 'Second',
            overview: null,
            air_date: '2022-01-08',
            runtime: 30,
            watched: false,
          },
        ],
      },
    ],
    ...overrides,
  };
}

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/shows/:tmdbId" element={<ShowDetail />} />
        <Route path="/" element={<div>HomePage</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mockGet.mockReset();
  mockDelete.mockReset();
  mockSet.mockReset();
  mockBulk.mockReset();
  vi.spyOn(window, 'confirm').mockReturnValue(true);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('ShowDetail', () => {
  it('renders header with progress, providers, year range, and overview', async () => {
    mockGet.mockResolvedValueOnce(show());
    const { container } = renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());
    expect(container.querySelector('.progress-chip')).toHaveTextContent('0/2');
    expect(screen.getByText('Returning')).toBeInTheDocument();
    expect(screen.getByText('2022–2024')).toBeInTheDocument();
    expect(screen.getByText('A great show')).toBeInTheDocument();
    expect(screen.getByText(/Hulu/)).toBeInTheDocument();
  });

  it('omits year-range dash when first and last air dates are identical', async () => {
    mockGet.mockResolvedValueOnce(
      show({ first_air_date: '2024-01-01', last_air_date: '2024-01-01' }),
    );
    const { container } = renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());
    const yearSpan = container.querySelector('.year');
    expect(yearSpan).toHaveTextContent('2024');
    expect(yearSpan?.textContent ?? '').not.toContain('–');
  });

  it('shows placeholder poster when poster_url missing', async () => {
    mockGet.mockResolvedValueOnce(show({ poster_url: null }));
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('No poster')).toBeInTheDocument());
  });

  it('shows error when invalid id in URL', () => {
    renderAt('/shows/notanumber');
    expect(screen.getByText('Error: Invalid show id')).toBeInTheDocument();
  });

  it('shows API error when getShow rejects', async () => {
    mockGet.mockRejectedValueOnce(new Error('boom'));
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('Error: boom')).toBeInTheDocument());
  });

  it('shows generic load error for non-Error rejection', async () => {
    mockGet.mockRejectedValueOnce('weird');
    renderAt('/shows/1');
    await waitFor(() =>
      expect(screen.getByText('Error: Load failed')).toBeInTheDocument(),
    );
  });

  it('toggles a season and renders episode list', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(show());
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());

    expect(screen.queryByText('Pilot')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /Season 1/ }));
    expect(screen.getByText('Pilot')).toBeInTheDocument();
    expect(screen.getByText('S01E01')).toBeInTheDocument();

    // Closing collapses
    await user.click(screen.getByRole('button', { name: /Season 1/ }));
    expect(screen.queryByText('Pilot')).not.toBeInTheDocument();
  });

  it('checking an episode calls setEpisodeWatched and updates UI', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(show());
    mockSet.mockResolvedValueOnce(
      show({
        seasons: [
          {
            ...show().seasons[0],
            watched_count: 1,
            episodes: [
              { ...show().seasons[0].episodes[0], watched: true },
              show().seasons[0].episodes[1],
            ],
          },
        ],
      }),
    );
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());
    await user.click(screen.getByRole('button', { name: /Season 1/ }));

    const checkbox = screen.getByLabelText('Mark S1E1 watched');
    await user.click(checkbox);
    await waitFor(() => expect(mockSet).toHaveBeenCalledWith(1, 1, 1, true));
    await waitFor(() =>
      expect(document.querySelector('.progress-chip')?.textContent).toBe('1/2'),
    );
  });

  it('shows error when episode toggle fails', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(show());
    mockSet.mockRejectedValueOnce(new Error('failed'));
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());
    await user.click(screen.getByRole('button', { name: /Season 1/ }));
    await user.click(screen.getByLabelText('Mark S1E1 watched'));
    await waitFor(() => expect(screen.getByText('Error: failed')).toBeInTheDocument());
  });

  it('shows generic update error for non-Error rejection', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(show());
    mockSet.mockRejectedValueOnce('weird');
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());
    await user.click(screen.getByRole('button', { name: /Season 1/ }));
    await user.click(screen.getByLabelText('Mark S1E1 watched'));
    await waitFor(() =>
      expect(screen.getByText('Error: Update failed')).toBeInTheDocument(),
    );
  });

  it('Mark all watched calls bulkWatch with type=all and watched=true', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(show());
    mockBulk.mockResolvedValueOnce(show({}));
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());
    await user.click(screen.getByRole('button', { name: 'Mark all watched' }));
    await waitFor(() =>
      expect(mockBulk).toHaveBeenCalledWith(1, { type: 'all' }, true),
    );
  });

  it('Toggles to "Mark all unwatched" when fully watched', async () => {
    const user = userEvent.setup();
    const fullyWatched = show({
      seasons: [
        {
          ...show().seasons[0],
          watched_count: 2,
          episodes: show().seasons[0].episodes.map((e) => ({ ...e, watched: true })),
        },
      ],
    });
    mockGet.mockResolvedValueOnce(fullyWatched);
    mockBulk.mockResolvedValueOnce(show({}));
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());
    await user.click(screen.getByRole('button', { name: 'Mark all unwatched' }));
    await waitFor(() =>
      expect(mockBulk).toHaveBeenCalledWith(1, { type: 'all' }, false),
    );
  });

  it('Mark season scope calls bulkWatch with season type', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(show());
    mockBulk.mockResolvedValueOnce(show({}));
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());

    const seasonWatchBtn = screen.getAllByRole('button', { name: 'Mark watched' })[0];
    await user.click(seasonWatchBtn);
    await waitFor(() =>
      expect(mockBulk).toHaveBeenCalledWith(
        1,
        { type: 'season', season_number: 1 },
        true,
      ),
    );
  });

  it('Mark through-episode bulk-watches up to that episode', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(show());
    mockBulk.mockResolvedValueOnce(show({}));
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());
    await user.click(screen.getByRole('button', { name: /Season 1/ }));

    const throughBtns = screen.getAllByRole('button', { name: 'Mark through here' });
    await user.click(throughBtns[1]!);
    await waitFor(() =>
      expect(mockBulk).toHaveBeenCalledWith(
        1,
        { type: 'through_episode', season_number: 1, episode_number: 2 },
        true,
      ),
    );
  });

  it('Disables season action when episode_count is 0', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(
      show({
        seasons: [
          {
            season_number: 1,
            name: 'S1',
            overview: null,
            air_date: null,
            episode_count: 0,
            watched_count: 0,
            episodes: [],
          },
        ],
      }),
    );
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Mark watched' })).toBeDisabled();
    // Bulk button never called
    await user.click(screen.getByRole('button', { name: 'Mark all watched' }));
    await waitFor(() => expect(mockBulk).toHaveBeenCalledTimes(1));
  });

  it('Removes show after confirm and navigates home', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(show());
    mockDelete.mockResolvedValueOnce(undefined);
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Remove from watchlist' }));
    await waitFor(() => expect(mockDelete).toHaveBeenCalledWith(1));
    await waitFor(() => expect(screen.getByText('HomePage')).toBeInTheDocument());
  });

  it('Aborts removal when user cancels confirm()', async () => {
    const user = userEvent.setup();
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    mockGet.mockResolvedValueOnce(show());
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Remove from watchlist' }));
    expect(mockDelete).not.toHaveBeenCalled();
  });

  it('Shows error and stays on page when delete fails', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(show());
    mockDelete.mockRejectedValueOnce(new Error('cannot delete'));
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Remove from watchlist' }));
    await waitFor(() =>
      expect(screen.getByText('Error: cannot delete')).toBeInTheDocument(),
    );
    expect(screen.queryByText('HomePage')).not.toBeInTheDocument();
  });

  it('Shows generic delete error for non-Error rejection', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(show());
    mockDelete.mockRejectedValueOnce('weird');
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Remove from watchlist' }));
    await waitFor(() =>
      expect(screen.getByText('Error: Delete failed')).toBeInTheDocument(),
    );
  });

  it('Renders "No seasons available yet" when seasons array is empty', async () => {
    mockGet.mockResolvedValueOnce(show({ seasons: [] }));
    renderAt('/shows/1');
    await waitFor(() =>
      expect(screen.getByText('No seasons available yet.')).toBeInTheDocument(),
    );
  });

  it('Falls back to "Season N" label when season name missing', async () => {
    mockGet.mockResolvedValueOnce(
      show({
        seasons: [
          {
            season_number: 3,
            name: null,
            overview: null,
            air_date: null,
            episode_count: 0,
            watched_count: 0,
            episodes: [],
          },
        ],
      }),
    );
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('Season 3')).toBeInTheDocument());
  });

  it('Falls back to em-dash when episode name missing', async () => {
    const user = userEvent.setup();
    mockGet.mockResolvedValueOnce(
      show({
        seasons: [
          {
            season_number: 1,
            name: 'S1',
            overview: null,
            air_date: null,
            episode_count: 1,
            watched_count: 0,
            episodes: [
              {
                episode_number: 1,
                name: null,
                overview: null,
                air_date: null,
                runtime: null,
                watched: false,
              },
            ],
          },
        ],
      }),
    );
    renderAt('/shows/1');
    await waitFor(() => expect(screen.getByText('My Show')).toBeInTheDocument());
    await user.click(screen.getByRole('button', { name: /S1/ }));
    expect(screen.getByText('—')).toBeInTheDocument();
  });
});
