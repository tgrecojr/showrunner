import { useEffect, useState } from "react";
import { Link } from "react-router";
import { api } from "../api/client";
import type { WatchlistItem } from "../types";

export default function Watchlist() {
	const [shows, setShows] = useState<WatchlistItem[] | null>(null);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		let cancelled = false;
		api
			.listShows()
			.then((res) => {
				if (!cancelled) setShows(res.shows);
			})
			.catch((err: unknown) => {
				if (!cancelled) {
					setError(err instanceof Error ? err.message : "Load failed");
				}
			});
		return () => {
			cancelled = true;
		};
	}, []);

	if (error) {
		return (
			<div>
				<h1>Watchlist</h1>
				<p className="status status-error">Error: {error}</p>
			</div>
		);
	}

	if (shows === null) {
		return (
			<div>
				<h1>Watchlist</h1>
				<p className="status">Loading…</p>
			</div>
		);
	}

	if (shows.length === 0) {
		return (
			<div>
				<h1>Watchlist</h1>
				<p>
					Nothing tracked yet. <Link to="/search">Search for a show</Link> to
					get started.
				</p>
			</div>
		);
	}

	return (
		<div>
			<h1>Watchlist</h1>
			<ul className="results-grid">
				{shows.map((s) => (
					<li key={s.tmdb_id} className="result-card">
						<Link to={`/shows/${s.tmdb_id}`} className="card-poster-link">
							{s.poster_url ? (
								<img
									src={s.poster_url}
									alt={`${s.name} poster`}
									className="poster"
								/>
							) : (
								<div className="poster poster-placeholder">No poster</div>
							)}
						</Link>
						<div className="result-body">
							<h3>
								<Link to={`/shows/${s.tmdb_id}`} className="card-title-link">
									{s.name}
								</Link>
							</h3>
							<div className="meta-row">
								<span className="progress-chip">
									{s.watched_count}/{s.aired_count}
								</span>
								{s.status && <span className="status-pill">{s.status}</span>}
							</div>
							{s.next_episode_air_date && (
								<p className="next-air">Next airs {s.next_episode_air_date}</p>
							)}
						</div>
					</li>
				))}
			</ul>
		</div>
	);
}
