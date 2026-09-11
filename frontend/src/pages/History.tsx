import { useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router";
import { api } from "../api/client";
import type { WatchLogPage } from "../types";
import { describeEntry, plural } from "./watchLogText";

const PER_PAGE = 50;

function formatTime(iso: string): string {
	const d = new Date(iso);
	if (Number.isNaN(d.getTime())) return iso;
	return d.toLocaleString(undefined, {
		year: "numeric",
		month: "short",
		day: "numeric",
		hour: "numeric",
		minute: "2-digit",
	});
}

function parsePage(raw: string | null): number {
	const n = Number(raw);
	return Number.isInteger(n) && n >= 1 ? n : 1;
}

export default function History() {
	const [searchParams, setSearchParams] = useSearchParams();
	const page = parsePage(searchParams.get("page"));
	const [data, setData] = useState<WatchLogPage | null>(null);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		let cancelled = false;
		setError(null);
		api
			.watchLog(page, PER_PAGE)
			.then((res) => {
				if (!cancelled) setData(res);
			})
			.catch((err: unknown) => {
				if (!cancelled) {
					setError(err instanceof Error ? err.message : "Load failed");
				}
			});
		return () => {
			cancelled = true;
		};
	}, [page]);

	function goTo(next: number) {
		setSearchParams(next === 1 ? {} : { page: String(next) });
	}

	if (error && data === null)
		return (
			<div>
				<h1>Watch History</h1>
				<p className="status status-error">Error: {error}</p>
			</div>
		);
	if (data === null)
		return (
			<div>
				<h1>Watch History</h1>
				<p className="status">Loading…</p>
			</div>
		);

	if (data.total === 0) {
		return (
			<div>
				<h1>Watch History</h1>
				<p>
					Nothing logged yet. Marking an episode or movie watched will show up
					here.
				</p>
			</div>
		);
	}

	const totalPages = Math.max(1, Math.ceil(data.total / data.per_page));

	return (
		<div className="history-page">
			<h1>Watch History</h1>
			<p className="status">
				Every watched / unwatched action, newest first. Undo mistakes from the
				show or movie page.
			</p>
			{error && <p className="status status-error">Error: {error}</p>}

			<ul className="upnext-list">
				{data.entries.map((entry) => (
					<li
						key={entry.id}
						className={`upnext-row${
							entry.action === "unwatched" ? " history-row-unwatched" : ""
						}`}
					>
						{entry.poster_url ? (
							<img
								src={entry.poster_url}
								alt={`${entry.title} poster`}
								className="upnext-poster"
							/>
						) : (
							<div className="upnext-poster poster-placeholder">No poster</div>
						)}
						<div className="upnext-body">
							<h3>
								{entry.media_type === "tv" ? (
									<Link
										to={`/shows/${entry.tmdb_id}`}
										className="card-title-link"
									>
										{entry.title}
									</Link>
								) : (
									entry.title
								)}
							</h3>
							<p className="history-sentence">{describeEntry(entry)}</p>
						</div>
						<time className="history-time" dateTime={entry.occurred_at}>
							{formatTime(entry.occurred_at)}
						</time>
					</li>
				))}
			</ul>

			<div className="pager">
				<span className="status">
					Page {data.page} of {totalPages} ·{" "}
					{plural(data.total, "entry", "entries")}
				</span>
				<div className="pager-buttons">
					<button
						type="button"
						className="btn btn-sm"
						onClick={() => goTo(page - 1)}
						disabled={page <= 1}
					>
						Previous
					</button>
					<button
						type="button"
						className="btn btn-sm"
						onClick={() => goTo(page + 1)}
						disabled={page >= totalPages}
					>
						Next
					</button>
				</div>
			</div>
		</div>
	);
}
