import { useEffect, useState } from 'react';
import { api } from '../api/client';
import type {
  HealthResponse,
  SyncResponse,
  TestNotificationResponse,
} from '../types';

export default function Settings() {
  const [health, setHealth] = useState<HealthResponse | null>(null);

  const [syncing, setSyncing] = useState(false);
  const [syncResult, setSyncResult] = useState<SyncResponse | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);

  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<TestNotificationResponse | null>(
    null,
  );
  const [testError, setTestError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    api
      .health()
      .then((h) => {
        if (!cancelled) setHealth(h);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  async function runSync() {
    setSyncing(true);
    setSyncError(null);
    setSyncResult(null);
    try {
      setSyncResult(await api.sync());
    } catch (err) {
      setSyncError(err instanceof Error ? err.message : 'Sync failed');
    } finally {
      setSyncing(false);
    }
  }

  async function sendTest() {
    setTesting(true);
    setTestError(null);
    setTestResult(null);
    try {
      setTestResult(await api.testNotification());
    } catch (err) {
      setTestError(err instanceof Error ? err.message : 'Test failed');
    } finally {
      setTesting(false);
    }
  }

  return (
    <div className="settings-page">
      <h1>Settings</h1>

      <section className="settings-section">
        <h2>Notifications</h2>
        {health === null ? (
          <p className="status">Loading…</p>
        ) : health.notifiers.length === 0 ? (
          <p>
            No notification channels configured. Set <code>SLACK_WEBHOOK_URL</code>{' '}
            in your environment to enable Slack notifications.
          </p>
        ) : (
          <p>
            Configured channels:{' '}
            {health.notifiers.map((n) => (
              <span key={n} className="status-pill">{n}</span>
            ))}
          </p>
        )}

        <button
          type="button"
          className="btn btn-primary"
          onClick={sendTest}
          disabled={testing || (health?.notifiers.length ?? 0) === 0}
        >
          {testing ? 'Sending…' : 'Send test notification'}
        </button>

        {testError && <p className="status status-error">Error: {testError}</p>}
        {testResult && (
          <ul className="settings-result-list">
            {testResult.results.map((r) => (
              <li key={r.channel}>
                <strong>{r.channel}:</strong>{' '}
                {r.ok ? (
                  <span className="status-pill status-pill-success">sent</span>
                ) : (
                  <span className="status status-error">{r.error ?? 'failed'}</span>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="settings-section">
        <h2>TMDB sync</h2>
        <p>
          Refreshes every show's seasons and episodes from TMDB. Watched state
          is preserved. Runs automatically on the configured cron schedule.
        </p>
        <button
          type="button"
          className="btn btn-primary"
          onClick={runSync}
          disabled={syncing}
        >
          {syncing ? 'Syncing…' : 'Resync all shows from TMDB'}
        </button>

        {syncError && <p className="status status-error">Error: {syncError}</p>}
        {syncResult && (
          <div className="settings-result-list">
            <p>
              Synced {syncResult.shows_synced} show(s).
              {syncResult.errors.length > 0 &&
                ` ${syncResult.errors.length} failed.`}
            </p>
            {syncResult.errors.length > 0 && (
              <ul>
                {syncResult.errors.map((e) => (
                  <li key={e.tmdb_id}>
                    Show #{e.tmdb_id}: {e.message}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </section>
    </div>
  );
}
