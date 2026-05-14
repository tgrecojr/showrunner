import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import Movies from './Movies';
import type { MovieWatchlistItem } from '../types';

vi.mock('../api/client', () => ({
  api: { listMovies: vi.fn(), deleteMovie: vi.fn() },
}));
import { api } from '../api/client';

const mockListMovies = vi.mocked(api.listMovies);
const mockDeleteMovie = vi.mocked(api.deleteMovie);

function buildMovie(overrides: Partial<MovieWatchlistItem> = {}): MovieWatchlistItem {
  return {
    tmdb_id: 1,
    name: 'Inception',
    overview: 'dreams',
    poster_url: '/p.jpg',
    backdrop_url: '/b.jpg',
    release_date: '2010-07-16',
    runtime: 148,
    added_at: '2026-05-13T00:00:00Z',
    ...overrides,
  };
}

function renderPage() {
  return render(
    <MemoryRouter>
      <Movies />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mockListMovies.mockReset();
  mockDeleteMovie.mockReset();
  vi.spyOn(window, 'confirm').mockReturnValue(true);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('Movies', () => {
  it('shows loading then renders movies with year', async () => {
    mockListMovies.mockResolvedValueOnce({
      movies: [
        buildMovie({ tmdb_id: 1, name: 'Inception' }),
        buildMovie({
          tmdb_id: 2,
          name: 'Dune',
          poster_url: null,
          release_date: null,
        }),
      ],
    });
    renderPage();
    expect(screen.getByText('Loading…')).toBeInTheDocument();

    await waitFor(() => expect(screen.getByText('Inception')).toBeInTheDocument());
    expect(screen.getByText('Dune')).toBeInTheDocument();
    expect(screen.getByText('(2010)')).toBeInTheDocument();
    expect(screen.getByAltText('Inception poster')).toHaveAttribute('src', '/p.jpg');
    expect(screen.getByText('No poster')).toBeInTheDocument();
  });

  it('poster links to the movie detail page', async () => {
    mockListMovies.mockResolvedValueOnce({
      movies: [buildMovie({ tmdb_id: 42, name: 'Inception' })],
    });
    renderPage();
    await waitFor(() => expect(screen.getByText('Inception')).toBeInTheDocument());

    expect(
      screen.getByRole('link', { name: 'View details for Inception' }),
    ).toHaveAttribute('href', '/movies/42');
  });

  it('shows empty state with link to Search', async () => {
    mockListMovies.mockResolvedValueOnce({ movies: [] });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText(/No movies yet/)).toBeInTheDocument(),
    );
    expect(
      screen.getByRole('link', { name: 'Search for a movie' }),
    ).toHaveAttribute('href', '/search');
  });

  it('shows error state when load fails', async () => {
    mockListMovies.mockRejectedValueOnce(new Error('boom'));
    renderPage();
    await waitFor(() => expect(screen.getByText('Error: boom')).toBeInTheDocument());
  });

  it('Mark Watched removes the movie from the list', async () => {
    const user = userEvent.setup();
    mockListMovies.mockResolvedValueOnce({
      movies: [buildMovie({ tmdb_id: 1, name: 'Inception' })],
    });
    mockDeleteMovie.mockResolvedValueOnce(undefined as never);
    renderPage();
    await waitFor(() => expect(screen.getByText('Inception')).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Mark Watched' }));

    await waitFor(() => expect(mockDeleteMovie).toHaveBeenCalledWith(1));
    await waitFor(() =>
      expect(screen.queryByText('Inception')).not.toBeInTheDocument(),
    );
  });

  it('Remove triggers deleteMovie and removes the card', async () => {
    const user = userEvent.setup();
    mockListMovies.mockResolvedValueOnce({
      movies: [buildMovie({ tmdb_id: 5, name: 'Dune' })],
    });
    mockDeleteMovie.mockResolvedValueOnce(undefined as never);
    renderPage();
    await waitFor(() => expect(screen.getByText('Dune')).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Remove' }));

    await waitFor(() => expect(mockDeleteMovie).toHaveBeenCalledWith(5));
    await waitFor(() => expect(screen.queryByText('Dune')).not.toBeInTheDocument());
  });

  it('does nothing if the user cancels the confirm', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    const user = userEvent.setup();
    mockListMovies.mockResolvedValueOnce({
      movies: [buildMovie({ tmdb_id: 1, name: 'Inception' })],
    });
    renderPage();
    await waitFor(() => expect(screen.getByText('Inception')).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Mark Watched' }));

    expect(mockDeleteMovie).not.toHaveBeenCalled();
    expect(screen.getByText('Inception')).toBeInTheDocument();
  });

  it('shows error and keeps card when delete fails', async () => {
    const user = userEvent.setup();
    mockListMovies.mockResolvedValueOnce({
      movies: [buildMovie({ tmdb_id: 1, name: 'Inception' })],
    });
    mockDeleteMovie.mockRejectedValueOnce(new Error('500'));
    renderPage();
    await waitFor(() => expect(screen.getByText('Inception')).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Remove' }));

    await waitFor(() => expect(screen.getByText('Error: 500')).toBeInTheDocument());
  });
});
