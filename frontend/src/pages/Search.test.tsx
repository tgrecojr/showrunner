import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import Search from './Search';
import type { SearchResult } from '../types';

vi.mock('../api/client', () => ({
  api: { search: vi.fn(), addShow: vi.fn() },
}));
import { api } from '../api/client';

const mockSearch = vi.mocked(api.search);
const mockAdd = vi.mocked(api.addShow);

function result(overrides: Partial<SearchResult> = {}): SearchResult {
  return {
    tmdb_id: 1,
    name: 'Show',
    overview: 'description',
    first_air_date: '2024-05-01',
    poster_url: '/p.jpg',
    already_tracked: false,
    ...overrides,
  };
}

function renderPage() {
  return render(
    <MemoryRouter>
      <Search />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  mockSearch.mockReset();
  mockAdd.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

async function flushDebounce() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(400);
  });
}

describe('Search', () => {
  it('debounces query and renders results', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockSearch.mockResolvedValueOnce({ results: [result({ tmdb_id: 99 })] });
    renderPage();

    await user.type(screen.getByLabelText('Search shows'), 'bear');
    expect(mockSearch).not.toHaveBeenCalled();

    await flushDebounce();
    await waitFor(() => expect(mockSearch).toHaveBeenCalledWith('bear'));
    await waitFor(() => expect(screen.getByText('Show')).toBeInTheDocument());
    expect(screen.getByText('(2024)')).toBeInTheDocument();
    expect(screen.getByText('description')).toBeInTheDocument();
  });

  it('clears results when query becomes empty', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockSearch.mockResolvedValueOnce({ results: [result()] });
    renderPage();

    const input = screen.getByLabelText('Search shows');
    await user.type(input, 'bear');
    await flushDebounce();
    await waitFor(() => expect(screen.getByText('Show')).toBeInTheDocument());

    await user.clear(input);
    await waitFor(() => expect(screen.queryByText('Show')).not.toBeInTheDocument());
  });

  it('shows no-matches message when results array is empty', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockSearch.mockResolvedValueOnce({ results: [] });
    renderPage();

    await user.type(screen.getByLabelText('Search shows'), 'xyzzy');
    await flushDebounce();
    await waitFor(() => expect(screen.getByText('No matches.')).toBeInTheDocument());
  });

  it('shows search error', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockSearch.mockRejectedValueOnce(new Error('500'));
    renderPage();

    await user.type(screen.getByLabelText('Search shows'), 'q');
    await flushDebounce();
    await waitFor(() => expect(screen.getByText('Error: 500')).toBeInTheDocument());
  });

  it('shows generic search error for non-Error rejection', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockSearch.mockRejectedValueOnce('boom');
    renderPage();

    await user.type(screen.getByLabelText('Search shows'), 'q');
    await flushDebounce();
    await waitFor(() =>
      expect(screen.getByText('Error: Search failed')).toBeInTheDocument(),
    );
  });

  it('marks already-tracked shows with a chip rather than an Add button', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockSearch.mockResolvedValueOnce({ results: [result({ already_tracked: true })] });
    renderPage();

    await user.type(screen.getByLabelText('Search shows'), 'q');
    await flushDebounce();
    await waitFor(() => expect(screen.getByText('On watchlist')).toBeInTheDocument());
    expect(screen.queryByRole('button', { name: 'Add' })).not.toBeInTheDocument();
  });

  it('adds a show, transitions to "On watchlist"', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockSearch.mockResolvedValueOnce({ results: [result({ tmdb_id: 7 })] });
    mockAdd.mockResolvedValueOnce({} as never);
    renderPage();

    await user.type(screen.getByLabelText('Search shows'), 'q');
    await flushDebounce();
    const addBtn = await screen.findByRole('button', { name: 'Add' });
    await user.click(addBtn);

    await waitFor(() => expect(mockAdd).toHaveBeenCalledWith(7));
    await waitFor(() => expect(screen.getByText('On watchlist')).toBeInTheDocument());
  });

  it('shows per-result error when add fails', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockSearch.mockResolvedValueOnce({ results: [result({ tmdb_id: 7 })] });
    mockAdd.mockRejectedValueOnce(new Error('duplicate'));
    renderPage();

    await user.type(screen.getByLabelText('Search shows'), 'q');
    await flushDebounce();
    const addBtn = await screen.findByRole('button', { name: 'Add' });
    await user.click(addBtn);

    await waitFor(() => expect(screen.getByText('duplicate')).toBeInTheDocument());
  });

  it('shows generic add error for non-Error rejection', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockSearch.mockResolvedValueOnce({ results: [result({ tmdb_id: 7 })] });
    mockAdd.mockRejectedValueOnce('weird');
    renderPage();

    await user.type(screen.getByLabelText('Search shows'), 'q');
    await flushDebounce();
    const addBtn = await screen.findByRole('button', { name: 'Add' });
    await user.click(addBtn);

    await waitFor(() => expect(screen.getByText('Add failed')).toBeInTheDocument());
  });

  it('renders placeholder card when poster is missing and falsy first_air_date', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockSearch.mockResolvedValueOnce({
      results: [result({ poster_url: null, first_air_date: null, overview: null })],
    });
    renderPage();

    await user.type(screen.getByLabelText('Search shows'), 'q');
    await flushDebounce();
    await waitFor(() => expect(screen.getByText('No poster')).toBeInTheDocument());
    expect(screen.queryByText(/\(202\d\)/)).not.toBeInTheDocument();
  });
});
