import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import Settings from './Settings';

vi.mock('../api/client', () => ({
  api: {
    sync: vi.fn(),
  },
}));
import { api } from '../api/client';

const mockSync = vi.mocked(api.sync);

function renderPage() {
  return render(
    <MemoryRouter>
      <Settings />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mockSync.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('Settings', () => {
  it('runs sync and shows result', async () => {
    const user = userEvent.setup();
    mockSync.mockResolvedValueOnce({ shows_synced: 5, errors: [] });
    renderPage();

    await user.click(screen.getByRole('button', { name: 'Resync all shows from TMDB' }));
    await waitFor(() => expect(screen.getByText('Synced 5 show(s).')).toBeInTheDocument());
  });

  it('renders sync errors per show', async () => {
    const user = userEvent.setup();
    mockSync.mockResolvedValueOnce({
      shows_synced: 1,
      errors: [{ tmdb_id: 7, message: 'tmdb 404' }],
    });
    renderPage();

    await user.click(screen.getByRole('button', { name: 'Resync all shows from TMDB' }));
    await waitFor(() =>
      expect(screen.getByText(/Synced 1 show\(s\). 1 failed./)).toBeInTheDocument(),
    );
    expect(screen.getByText(/Show #7: tmdb 404/)).toBeInTheDocument();
  });

  it('shows error when sync rejects', async () => {
    const user = userEvent.setup();
    mockSync.mockRejectedValueOnce(new Error('boom'));
    renderPage();

    await user.click(screen.getByRole('button', { name: 'Resync all shows from TMDB' }));
    await waitFor(() => expect(screen.getByText('Error: boom')).toBeInTheDocument());
  });

  it('shows generic sync error for non-Error rejection', async () => {
    const user = userEvent.setup();
    mockSync.mockRejectedValueOnce('weird');
    renderPage();

    await user.click(screen.getByRole('button', { name: 'Resync all shows from TMDB' }));
    await waitFor(() =>
      expect(screen.getByText('Error: Sync failed')).toBeInTheDocument(),
    );
  });
});
