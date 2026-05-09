import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import Settings from './Settings';

vi.mock('../api/client', () => ({
  api: {
    health: vi.fn(),
    sync: vi.fn(),
    testNotification: vi.fn(),
  },
}));
import { api } from '../api/client';

const mockHealth = vi.mocked(api.health);
const mockSync = vi.mocked(api.sync);
const mockTest = vi.mocked(api.testNotification);

function renderPage() {
  return render(
    <MemoryRouter>
      <Settings />
    </MemoryRouter>,
  );
}

beforeEach(() => {
  mockHealth.mockReset();
  mockSync.mockReset();
  mockTest.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('Settings', () => {
  it('shows "no channels" message and disables test button when notifiers empty', async () => {
    mockHealth.mockResolvedValueOnce({
      status: 'ok',
      version: '0.1.0',
      database: true,
      notifiers: [],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText(/No notification channels configured/)).toBeInTheDocument(),
    );
    expect(screen.getByRole('button', { name: 'Send test notification' })).toBeDisabled();
  });

  it('renders configured channels and enables test button', async () => {
    mockHealth.mockResolvedValueOnce({
      status: 'ok',
      version: '0.1.0',
      database: true,
      notifiers: ['slack'],
    });
    renderPage();
    await waitFor(() => expect(screen.getByText('slack')).toBeInTheDocument());
    expect(screen.getByRole('button', { name: 'Send test notification' })).not.toBeDisabled();
  });

  it('runs sync and shows result', async () => {
    const user = userEvent.setup();
    mockHealth.mockResolvedValueOnce({
      status: 'ok',
      version: '0.1.0',
      database: true,
      notifiers: [],
    });
    mockSync.mockResolvedValueOnce({ shows_synced: 5, errors: [] });
    renderPage();
    await waitFor(() =>
      expect(screen.getByText(/No notification channels configured/)).toBeInTheDocument(),
    );

    await user.click(screen.getByRole('button', { name: 'Resync all shows from TMDB' }));
    await waitFor(() => expect(screen.getByText('Synced 5 show(s).')).toBeInTheDocument());
  });

  it('renders sync errors per show', async () => {
    const user = userEvent.setup();
    mockHealth.mockResolvedValueOnce({
      status: 'ok',
      version: '0.1.0',
      database: true,
      notifiers: [],
    });
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
    mockHealth.mockResolvedValueOnce({
      status: 'ok',
      version: '0.1.0',
      database: true,
      notifiers: [],
    });
    mockSync.mockRejectedValueOnce(new Error('boom'));
    renderPage();

    await user.click(screen.getByRole('button', { name: 'Resync all shows from TMDB' }));
    await waitFor(() => expect(screen.getByText('Error: boom')).toBeInTheDocument());
  });

  it('shows generic sync error for non-Error rejection', async () => {
    const user = userEvent.setup();
    mockHealth.mockResolvedValueOnce({
      status: 'ok',
      version: '0.1.0',
      database: true,
      notifiers: [],
    });
    mockSync.mockRejectedValueOnce('weird');
    renderPage();

    await user.click(screen.getByRole('button', { name: 'Resync all shows from TMDB' }));
    await waitFor(() =>
      expect(screen.getByText('Error: Sync failed')).toBeInTheDocument(),
    );
  });

  it('sends a test notification and renders per-channel results', async () => {
    const user = userEvent.setup();
    mockHealth.mockResolvedValueOnce({
      status: 'ok',
      version: '0.1.0',
      database: true,
      notifiers: ['slack'],
    });
    mockTest.mockResolvedValueOnce({
      results: [
        { channel: 'slack', ok: true, error: null },
        { channel: 'email', ok: false, error: 'auth failed' },
      ],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Send test notification' })).not.toBeDisabled(),
    );

    await user.click(screen.getByRole('button', { name: 'Send test notification' }));
    await waitFor(() => expect(screen.getByText('sent')).toBeInTheDocument());
    expect(screen.getByText('auth failed')).toBeInTheDocument();
  });

  it('falls back to "failed" when ok=false and no error provided', async () => {
    const user = userEvent.setup();
    mockHealth.mockResolvedValueOnce({
      status: 'ok',
      version: '0.1.0',
      database: true,
      notifiers: ['slack'],
    });
    mockTest.mockResolvedValueOnce({
      results: [{ channel: 'slack', ok: false, error: null }],
    });
    renderPage();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Send test notification' })).not.toBeDisabled(),
    );
    await user.click(screen.getByRole('button', { name: 'Send test notification' }));
    await waitFor(() => expect(screen.getByText('failed')).toBeInTheDocument());
  });

  it('shows error when test notification rejects', async () => {
    const user = userEvent.setup();
    mockHealth.mockResolvedValueOnce({
      status: 'ok',
      version: '0.1.0',
      database: true,
      notifiers: ['slack'],
    });
    mockTest.mockRejectedValueOnce(new Error('webhook'));
    renderPage();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Send test notification' })).not.toBeDisabled(),
    );
    await user.click(screen.getByRole('button', { name: 'Send test notification' }));
    await waitFor(() => expect(screen.getByText('Error: webhook')).toBeInTheDocument());
  });

  it('shows generic test error for non-Error rejection', async () => {
    const user = userEvent.setup();
    mockHealth.mockResolvedValueOnce({
      status: 'ok',
      version: '0.1.0',
      database: true,
      notifiers: ['slack'],
    });
    mockTest.mockRejectedValueOnce('boom');
    renderPage();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Send test notification' })).not.toBeDisabled(),
    );
    await user.click(screen.getByRole('button', { name: 'Send test notification' }));
    await waitFor(() =>
      expect(screen.getByText('Error: Test failed')).toBeInTheDocument(),
    );
  });

  it('swallows health fetch failure silently', async () => {
    mockHealth.mockRejectedValueOnce(new Error('500'));
    renderPage();
    // Health call is fire-and-forget on init — no error UI is shown.
    expect(screen.getByText('Loading…')).toBeInTheDocument();
  });
});
