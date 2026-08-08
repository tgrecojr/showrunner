import { useState } from 'react';
import { api } from '../api/client';
import type { SyncResponse } from '../types';

export default function Settings() {
  const [syncing, setSyncing] = useState(false);
  const [syncResult, setSyncResult] = useState<SyncResponse | null>(null);
  const [syncError, setSyncError] = useState<string | null>(null);

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

  return (
    <div className="settings-page">
      <h1>Settings</h1>

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
