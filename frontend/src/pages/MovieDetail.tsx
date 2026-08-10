import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router";
import { api } from "../api/client";
import type { MovieDetail as MovieDetailType } from "../types";

function friendlyError(raw: string): string {
	const stripped = raw.replace(/^API \d+:\s*/, "");
	if (/rate.?limit/i.test(stripped) || /TMDB returned 429/i.test(stripped)) {
		return "We couldn't load this movie's details right now — TMDB is rate-limiting requests. Please try again in a moment.";
	}
	if (/TMDB returned 5\d\d/i.test(stripped) || /Upstream/i.test(stripped)) {
		return "We couldn't load this movie's details from TMDB right now. Please try again shortly.";
	}
	return stripped;
}

export default function MovieDetail() {
	const { tmdbId } = useParams<{ tmdbId: string }>();
	const navigate = useNavigate();
	const id = Number(tmdbId);

	const [movie, setMovie] = useState<MovieDetailType | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [pending, setPending] = useState(false);

	useEffect(() => {
		if (!Number.isFinite(id)) {
			setError("Invalid movie id");
			return;
		}
		let cancelled = false;
		api
			.getMovie(id)
			.then((res) => {
				if (!cancelled) setMovie(res);
			})
			.catch((err: unknown) => {
				if (!cancelled) {
					setError(
						friendlyError(err instanceof Error ? err.message : "Load failed"),
					);
				}
			});
		return () => {
			cancelled = true;
		};
	}, [id]);

	async function remove(message: string) {
		if (!movie) return;
		if (!confirm(message)) return;
		setPending(true);
		try {
			await api.deleteMovie(movie.tmdb_id);
			navigate("/movies");
		} catch (err) {
			setError(
				friendlyError(err instanceof Error ? err.message : "Update failed"),
			);
			setPending(false);
		}
	}

	if (error && !movie)
		return <p className="status status-error">Error: {error}</p>;
	if (!movie) return <p className="status">Loading…</p>;

	const year = movie.release_date ? movie.release_date.slice(0, 4) : null;
	const backdropStyle = movie.backdrop_url
		? { backgroundImage: `url("${encodeURI(movie.backdrop_url)}")` }
		: undefined;

	return (
		<div className="movie-detail">
			{error && <p className="status status-error">Error: {error}</p>}

			<div className="movie-backdrop" style={backdropStyle}>
				<header className="show-header">
					{movie.poster_url ? (
						<img
							src={movie.poster_url}
							alt={`${movie.name} poster`}
							className="poster poster-lg"
						/>
					) : (
						<div className="poster poster-lg poster-placeholder">No poster</div>
					)}
					<div className="show-meta">
						<h1>{movie.name}</h1>
						<div className="meta-row">
							{year && <span className="year">{year}</span>}
							{movie.runtime && (
								<span className="status-pill">{movie.runtime} min</span>
							)}
						</div>
						{movie.directors.length > 0 && (
							<p className="providers">
								<strong>
									{movie.directors.length === 1 ? "Director:" : "Directors:"}
								</strong>{" "}
								{movie.directors.join(", ")}
							</p>
						)}
						{movie.watch_providers.length > 0 && (
							<p className="providers">
								<strong>Watch on:</strong> {movie.watch_providers.join(", ")}
							</p>
						)}
						{movie.overview && <p>{movie.overview}</p>}
						<div className="action-row">
							<button
								type="button"
								className="btn btn-primary"
								disabled={pending}
								onClick={() => remove(`Mark "${movie.name}" as watched?`)}
							>
								Mark Watched
							</button>
							<button
								type="button"
								className="btn btn-danger"
								disabled={pending}
								onClick={() => remove(`Remove "${movie.name}" from your list?`)}
							>
								Remove
							</button>
						</div>
					</div>
				</header>
			</div>

			<section className="cast-section">
				<h2>Cast</h2>
				{movie.cast.length === 0 ? (
					<p>No cast information available.</p>
				) : (
					<ul className="cast-grid">
						{movie.cast.map((member) => (
							<li
								key={`${member.name}-${member.character ?? ""}`}
								className="cast-card"
							>
								{member.profile_url ? (
									<img
										src={member.profile_url}
										alt={member.name}
										className="cast-photo"
									/>
								) : (
									<div className="cast-photo cast-photo-placeholder">
										No photo
									</div>
								)}
								<p className="cast-name">{member.name}</p>
								{member.character && (
									<p className="cast-character">as {member.character}</p>
								)}
							</li>
						))}
					</ul>
				)}
			</section>
		</div>
	);
}
