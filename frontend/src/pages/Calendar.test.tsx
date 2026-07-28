import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import Calendar from './Calendar';
import type { CalendarEpisode } from '../types';

vi.mock('../api/client', () => ({
  api: { calendar: vi.fn() },
}));
import { api } from '../api/client';

const mockCalendar = vi.mocked(api.calendar);

function ep(overrides: Partial<CalendarEpisode> = {}): CalendarEpisode {
  return {
    show_tmdb_id: 1,
    show_name: 'Show',
    poster_url: '/p.jpg',
    season_number: 2,
    episode_number: 5,
    episode_name: 'Title',
    air_date: '2026-05-15',
    watched: false,
    ...overrides,
  };
}

function renderPage() {
  return render(
    <MemoryRouter>
      <Calendar />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  // Pin "today" to 2026-05-09.
  vi.useFakeTimers({ shouldAdvanceTime: true });
  vi.setSystemTime(new Date(2026, 4, 9, 12, 0, 0));
  mockCalendar.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('Calendar', () => {
  it('renders current month title and episode in correct cell', async () => {
    mockCalendar.mockResolvedValueOnce({ episodes: [ep()] });
    renderPage();
    expect(screen.getByText('May 2026')).toBeInTheDocument();
    await waitFor(() => expect(mockCalendar).toHaveBeenCalled());
    expect(screen.getByText('Show')).toBeInTheDocument();
    expect(screen.getByText('S02E05')).toBeInTheDocument();
  });

  it('queries with a 42-cell range covering the visible grid', async () => {
    mockCalendar.mockResolvedValueOnce({ episodes: [] });
    renderPage();
    await waitFor(() => expect(mockCalendar).toHaveBeenCalled());
    const [start, end] = mockCalendar.mock.calls[0]!;
    // First Sunday on/before May 1 2026 (a Friday) is April 26.
    expect(start).toBe('2026-04-26');
    // 42 cells later is June 6.
    expect(end).toBe('2026-06-06');
  });

  it('navigates to previous month', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockCalendar.mockResolvedValue({ episodes: [] });
    renderPage();
    await waitFor(() => expect(mockCalendar).toHaveBeenCalled());

    await user.click(screen.getByRole('button', { name: /Prev/ }));
    await waitFor(() => expect(screen.getByText('April 2026')).toBeInTheDocument());
  });

  it('navigates next and crosses year boundary forward', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockCalendar.mockResolvedValue({ episodes: [] });
    renderPage();
    await waitFor(() => expect(mockCalendar).toHaveBeenCalled());

    // Click Next 7 times: May→Dec then Dec→Jan 2027.
    for (let i = 0; i < 8; i++) {
      await user.click(screen.getByRole('button', { name: /Next/ }));
    }
    await waitFor(() => expect(screen.getByText('January 2027')).toBeInTheDocument());
  });

  it('navigates prev across year boundary backward', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockCalendar.mockResolvedValue({ episodes: [] });
    renderPage();
    await waitFor(() => expect(mockCalendar).toHaveBeenCalled());

    for (let i = 0; i < 5; i++) {
      await user.click(screen.getByRole('button', { name: /Prev/ }));
    }
    await waitFor(() => expect(screen.getByText('December 2025')).toBeInTheDocument());
  });

  it('Today button returns to current month', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    mockCalendar.mockResolvedValue({ episodes: [] });
    renderPage();
    await waitFor(() => expect(mockCalendar).toHaveBeenCalled());

    await user.click(screen.getByRole('button', { name: /Next/ }));
    await waitFor(() => expect(screen.getByText('June 2026')).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Today' }));
    await waitFor(() => expect(screen.getByText('May 2026')).toBeInTheDocument());
  });

  it('shows error when calendar fetch rejects', async () => {
    mockCalendar.mockRejectedValueOnce(new Error('range too big'));
    renderPage();
    await waitFor(() =>
      expect(screen.getByText('Error: range too big')).toBeInTheDocument(),
    );
  });

  it('shows generic error for non-Error rejection', async () => {
    mockCalendar.mockRejectedValueOnce('weird');
    renderPage();
    await waitFor(() =>
      expect(screen.getByText('Error: Load failed')).toBeInTheDocument(),
    );
  });

  it('renders watched episodes with the watched class and groups by date', async () => {
    mockCalendar.mockResolvedValueOnce({
      episodes: [
        ep({ show_tmdb_id: 1, show_name: 'A', air_date: '2026-05-05', watched: true }),
        ep({ show_tmdb_id: 2, show_name: 'B', air_date: '2026-05-05' }),
      ],
    });
    const { container } = renderPage();
    await waitFor(() => expect(screen.getByText('A')).toBeInTheDocument());
    expect(screen.getByText('B')).toBeInTheDocument();
    const watched = container.querySelector('.calendar-ep-watched');
    expect(watched).not.toBeNull();
  });

  it('handles missing episode poster and link wraps each episode', async () => {
    mockCalendar.mockResolvedValueOnce({
      episodes: [ep({ poster_url: null, episode_name: null })],
    });
    renderPage();
    await waitFor(() => expect(screen.getByText('Show')).toBeInTheDocument());
    const link = screen.getByRole('link');
    expect(link).toHaveAttribute('href', '/shows/1');
    // No image rendered when poster_url is null
    const img = within(link).queryByRole('img');
    expect(img).toBeNull();
  });
});
