import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { WatchLogEntry, WatchLogPage } from "../types";
import History from "./History";
import { describeEntry } from "./watchLogText";

vi.mock("../api/client", () => ({
	api: { watchLog: vi.fn() },
}));

import { api } from "../api/client";

const mockWatchLog = vi.mocked(api.watchLog);

function entry(overrides: Partial<WatchLogEntry> = {}): WatchLogEntry {
	return {
		id: 1,
		occurred_at: "2026-09-11T14:05:00+00:00",
		media_type: "tv",
		action: "watched",
		scope: "episode",
		tmdb_id: 42,
		title: "Severance",
		poster_url: "/sev.jpg",
		season_number: 2,
		episode_number: 5,
		episode_name: "Trojan's Horse",
		episode_count: 1,
		...overrides,
	};
}

function page(
	entries: WatchLogEntry[],
	overrides: Partial<WatchLogPage> = {},
): WatchLogPage {
	return {
		entries,
		page: 1,
		per_page: 50,
		total: entries.length,
		...overrides,
	};
}

function renderAt(path = "/history") {
	return render(
		<MemoryRouter initialEntries={[path]}>
			<History />
		</MemoryRouter>,
	);
}

beforeEach(() => {
	mockWatchLog.mockReset();
});

describe("describeEntry", () => {
	it("builds a sentence per scope and action", () => {
		expect(describeEntry(entry())).toBe(
			'Marked S02E05 "Trojan\'s Horse" watched',
		);
		expect(
			describeEntry(entry({ episode_name: null, action: "unwatched" })),
		).toBe("Marked S02E05 unwatched");
		expect(
			describeEntry(
				entry({ scope: "season", episode_count: 8, episode_number: null }),
			),
		).toBe("Marked Season 2 watched · 8 episodes");
		expect(describeEntry(entry({ scope: "show", episode_count: 1 }))).toBe(
			"Marked all episodes watched · 1 episode",
		);
		expect(
			describeEntry(entry({ scope: "through_episode", episode_count: 13 })),
		).toBe("Marked through S02E05 watched · 13 episodes");
		expect(
			describeEntry(
				entry({ scope: "movie", media_type: "movie", title: "Inception" }),
			),
		).toBe("Marked watched");
	});
});

describe("History", () => {
	it("shows loading then renders entries newest-first with links for shows", async () => {
		mockWatchLog.mockResolvedValueOnce(
			page([
				entry({
					id: 2,
					scope: "movie",
					media_type: "movie",
					title: "Inception",
					tmdb_id: 27205,
					poster_url: null,
				}),
				entry({ id: 1 }),
			]),
		);
		renderAt();
		expect(screen.getByText("Loading…")).toBeInTheDocument();

		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);
		expect(mockWatchLog).toHaveBeenCalledWith(1, 50);

		const items = screen.getAllByRole("listitem");
		expect(items).toHaveLength(2);
		expect(items[0]).toHaveTextContent("Inception");
		expect(items[1]).toHaveTextContent("Severance");

		expect(screen.getByRole("link", { name: "Severance" })).toHaveAttribute(
			"href",
			"/shows/42",
		);
		expect(screen.queryByRole("link", { name: "Inception" })).toBeNull();
		expect(screen.getByText("No poster")).toBeInTheDocument();
		expect(
			screen.getByText('Marked S02E05 "Trojan\'s Horse" watched'),
		).toBeInTheDocument();
		expect(screen.getByText("Page 1 of 1 · 2 entries")).toBeInTheDocument();
	});

	it("renders unwatched rows muted", async () => {
		mockWatchLog.mockResolvedValueOnce(page([entry({ action: "unwatched" })]));
		renderAt();
		await waitFor(() =>
			expect(screen.getByText("Severance")).toBeInTheDocument(),
		);
		expect(screen.getByRole("listitem")).toHaveClass("history-row-unwatched");
	});

	it("shows empty state", async () => {
		mockWatchLog.mockResolvedValueOnce(page([]));
		renderAt();
		await waitFor(() =>
			expect(screen.getByText(/Nothing logged yet/)).toBeInTheDocument(),
		);
	});

	it("shows error state when load fails", async () => {
		mockWatchLog.mockRejectedValueOnce(new Error("boom"));
		renderAt();
		await waitFor(() =>
			expect(screen.getByText("Error: boom")).toBeInTheDocument(),
		);
	});

	it("reads the page from the URL and pages with Previous/Next", async () => {
		const user = userEvent.setup();
		mockWatchLog.mockResolvedValueOnce(
			page([entry({ id: 51 })], { page: 2, per_page: 50, total: 120 }),
		);
		renderAt("/history?page=2");

		await waitFor(() =>
			expect(screen.getByText("Page 2 of 3 · 120 entries")).toBeInTheDocument(),
		);
		expect(mockWatchLog).toHaveBeenCalledWith(2, 50);
		expect(screen.getByRole("button", { name: "Previous" })).toBeEnabled();
		expect(screen.getByRole("button", { name: "Next" })).toBeEnabled();

		mockWatchLog.mockResolvedValueOnce(
			page([entry({ id: 101 })], { page: 3, per_page: 50, total: 120 }),
		);
		await user.click(screen.getByRole("button", { name: "Next" }));
		await waitFor(() => expect(mockWatchLog).toHaveBeenCalledWith(3, 50));
		await waitFor(() =>
			expect(screen.getByText("Page 3 of 3 · 120 entries")).toBeInTheDocument(),
		);
		expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
	});

	it("disables Previous on the first page and ignores a bad page param", async () => {
		mockWatchLog.mockResolvedValueOnce(page([entry()]));
		renderAt("/history?page=abc");
		await waitFor(() =>
			expect(screen.getByText("Severance")).toBeInTheDocument(),
		);
		expect(mockWatchLog).toHaveBeenCalledWith(1, 50);
		expect(screen.getByRole("button", { name: "Previous" })).toBeDisabled();
		expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
	});
});
