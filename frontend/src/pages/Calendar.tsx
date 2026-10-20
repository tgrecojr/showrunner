import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router";
import { api } from "../api/client";
import { useNarrow } from "../hooks/useNarrow";
import type { CalendarEpisode } from "../types";

const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS = [
	"January",
	"February",
	"March",
	"April",
	"May",
	"June",
	"July",
	"August",
	"September",
	"October",
	"November",
	"December",
];

function pad(n: number): string {
	return String(n).padStart(2, "0");
}

function isoDate(d: Date): string {
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

interface VisibleRange {
	cells: Date[]; // 42 cells (6 weeks × 7 days)
	startIso: string; // first cell ISO
	endIso: string; // last cell ISO
}

// @spec AIRING-CAL-006, AIRING-CAL-007
function buildVisibleRange(year: number, month: number): VisibleRange {
	const firstOfMonth = new Date(year, month, 1);
	const startWeekday = firstOfMonth.getDay(); // 0 = Sun

	const gridStart = new Date(year, month, 1 - startWeekday);
	const cells: Date[] = [];
	for (let i = 0; i < 42; i++) {
		const d = new Date(gridStart);
		d.setDate(gridStart.getDate() + i);
		cells.push(d);
	}
	return {
		cells,
		startIso: isoDate(cells[0]!),
		endIso: isoDate(cells[cells.length - 1]!),
	};
}

interface AgendaGroup {
	iso: string;
	label: string; // "Fri 15"
	episodes: CalendarEpisode[];
}

// One group per in-month date that has episodes, ascending. The fetched range
// covers padding days of the neighbouring months; those are left out here.
// @spec AIRING-CAL-015
function buildAgenda(
	byDate: Map<string, CalendarEpisode[]>,
	year: number,
	month: number,
): AgendaGroup[] {
	const prefix = `${year}-${pad(month + 1)}-`;
	return [...byDate.keys()]
		.filter((iso) => iso.startsWith(prefix))
		.sort()
		.map((iso) => {
			const day = Number(iso.slice(8, 10));
			// Built from the date's own parts so the weekday never shifts with the
			// browser's UTC offset.
			const weekday = WEEKDAYS[new Date(year, month, day).getDay()];
			return {
				iso,
				label: `${weekday} ${day}`,
				episodes: byDate.get(iso) ?? [],
			};
		});
}

function EpisodeChip({
	ep,
	showName,
}: {
	ep: CalendarEpisode;
	showName: boolean;
}) {
	return (
		<li
			className={ep.watched ? "calendar-ep calendar-ep-watched" : "calendar-ep"}
		>
			<Link
				to={`/shows/${ep.show_tmdb_id}`}
				title={ep.episode_name ?? ""}
				className="calendar-ep-link"
			>
				{ep.poster_url && (
					<img src={ep.poster_url} alt="" className="calendar-ep-poster" />
				)}
				<span className="calendar-ep-text">
					<span className="calendar-ep-show">{ep.show_name}</span>
					<span className="calendar-ep-code">
						S{pad(ep.season_number)}E{pad(ep.episode_number)}
					</span>
					{showName && ep.episode_name && (
						<span className="calendar-ep-name">{ep.episode_name}</span>
					)}
				</span>
			</Link>
		</li>
	);
}

// @spec AIRING-CAL-006, AIRING-CAL-007, AIRING-CAL-008, AIRING-CAL-009, AIRING-CAL-010, AIRING-CAL-011, AIRING-CAL-012, AIRING-CAL-013, AIRING-CAL-014, AIRING-CAL-015, AIRING-CAL-016, AIRING-CAL-017, AIRING-CAL-018, APP-SPA-006
export default function Calendar() {
	const narrow = useNarrow();
	const today = useMemo(() => new Date(), []);
	const [year, setYear] = useState(today.getFullYear());
	const [month, setMonth] = useState(today.getMonth());

	const [episodes, setEpisodes] = useState<CalendarEpisode[]>([]);
	// The server's today in its TIMEZONE, once a response has carried it; the
	// browser-local date stands in only until then.
	const [serverToday, setServerToday] = useState<string | null>(null);
	// Start date of the range whose fetch last settled (succeeded or failed).
	// A fetch is in flight exactly while this differs from the displayed range.
	const [settledFor, setSettledFor] = useState<string | null>(null);
	const [error, setError] = useState<string | null>(null);

	const range = useMemo(() => buildVisibleRange(year, month), [year, month]);
	const todayIso = serverToday ?? isoDate(today);
	const loading = settledFor !== range.startIso;

	useEffect(() => {
		let cancelled = false;
		setError(null);
		api
			.calendar(range.startIso, range.endIso)
			.then((res) => {
				if (!cancelled) {
					setEpisodes(res.episodes);
					setServerToday(res.today);
				}
			})
			.catch((err: unknown) => {
				if (!cancelled) {
					setError(err instanceof Error ? err.message : "Load failed");
				}
			})
			.finally(() => {
				if (!cancelled) setSettledFor(range.startIso);
			});
		return () => {
			cancelled = true;
		};
	}, [range.startIso, range.endIso]);

	const episodesByDate = useMemo(() => {
		const map = new Map<string, CalendarEpisode[]>();
		for (const ep of episodes) {
			const list = map.get(ep.air_date) ?? [];
			list.push(ep);
			map.set(ep.air_date, list);
		}
		return map;
	}, [episodes]);

	const agenda = useMemo(
		() => buildAgenda(episodesByDate, year, month),
		[episodesByDate, year, month],
	);

	function goPrev() {
		if (month === 0) {
			setYear(year - 1);
			setMonth(11);
		} else {
			setMonth(month - 1);
		}
	}

	function goNext() {
		if (month === 11) {
			setYear(year + 1);
			setMonth(0);
		} else {
			setMonth(month + 1);
		}
	}

	function goToday() {
		if (serverToday) {
			const [y, m] = serverToday.split("-").map(Number);
			setYear(y as number);
			setMonth((m as number) - 1);
			return;
		}
		setYear(today.getFullYear());
		setMonth(today.getMonth());
	}

	function renderGrid() {
		return (
			<div className="calendar-grid">
				{WEEKDAYS.map((d) => (
					<div key={d} className="calendar-weekday">
						{d}
					</div>
				))}
				{range.cells.map((date) => {
					const iso = isoDate(date);
					const inMonth = date.getMonth() === month;
					const isToday = iso === todayIso;
					const dayEpisodes = episodesByDate.get(iso) ?? [];
					return (
						<div
							key={iso}
							className={[
								"calendar-cell",
								inMonth ? "" : "calendar-cell-other",
								isToday ? "calendar-cell-today" : "",
							]
								.filter(Boolean)
								.join(" ")}
						>
							<div className="calendar-day-num">{date.getDate()}</div>
							<ul className="calendar-eps">
								{dayEpisodes.map((ep) => (
									<EpisodeChip
										key={`${ep.show_tmdb_id}-${ep.season_number}-${ep.episode_number}`}
										ep={ep}
										showName={false}
									/>
								))}
							</ul>
						</div>
					);
				})}
			</div>
		);
	}

	function renderAgenda() {
		if (agenda.length === 0 && !loading) {
			return (
				<p className="status">{`Nothing airing in ${MONTHS[month]} ${year}.`}</p>
			);
		}
		return (
			<ul className="calendar-agenda">
				{agenda.map((group) => (
					<li
						key={group.iso}
						className={
							group.iso === todayIso
								? "calendar-agenda-day calendar-agenda-today"
								: "calendar-agenda-day"
						}
					>
						<h2 className="calendar-agenda-date">{group.label}</h2>
						<ul className="calendar-eps">
							{group.episodes.map((ep) => (
								<EpisodeChip
									key={`${ep.show_tmdb_id}-${ep.season_number}-${ep.episode_number}`}
									ep={ep}
									showName
								/>
							))}
						</ul>
					</li>
				))}
			</ul>
		);
	}

	return (
		<div className="calendar-page">
			<div className="calendar-toolbar">
				<h1>
					{MONTHS[month]} {year}
				</h1>
				<div className="calendar-nav">
					<button type="button" className="btn btn-sm" onClick={goPrev}>
						‹ Prev
					</button>
					<button type="button" className="btn btn-sm" onClick={goToday}>
						Today
					</button>
					<button type="button" className="btn btn-sm" onClick={goNext}>
						Next ›
					</button>
				</div>
			</div>

			{error && <p className="status status-error">Error: {error}</p>}

			{narrow ? renderAgenda() : renderGrid()}

			{loading && <p className="status">Loading…</p>}
		</div>
	);
}
