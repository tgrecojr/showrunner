import type { WatchLogEntry } from "../types";

function pad(n: number): string {
	return String(n).padStart(2, "0");
}

function epCode(entry: WatchLogEntry): string {
	return `S${pad(entry.season_number ?? 0)}E${pad(entry.episode_number ?? 0)}`;
}

export function plural(n: number, one: string, many: string): string {
	return `${n} ${n === 1 ? one : many}`;
}

/** One human sentence per log row, e.g. `Marked S02E05 "Title" watched`. */
export function describeEntry(entry: WatchLogEntry): string {
	const verb = entry.action === "watched" ? "watched" : "unwatched";
	const count = ` · ${plural(entry.episode_count, "episode", "episodes")}`;
	switch (entry.scope) {
		case "episode": {
			const name = entry.episode_name ? ` "${entry.episode_name}"` : "";
			return `Marked ${epCode(entry)}${name} ${verb}`;
		}
		case "season":
			return `Marked Season ${entry.season_number ?? "?"} ${verb}${count}`;
		case "show":
			return `Marked all episodes ${verb}${count}`;
		case "through_episode":
			return `Marked through ${epCode(entry)} ${verb}${count}`;
		case "movie":
			return `Marked ${verb}`;
		default:
			return `Marked ${verb}`;
	}
}
