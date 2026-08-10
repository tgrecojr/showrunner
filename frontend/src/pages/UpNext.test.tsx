import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { UpNextItem } from "../types";
import UpNext from "./UpNext";

vi.mock("../api/client", () => ({
	api: { upNext: vi.fn(), setEpisodeWatched: vi.fn() },
}));

import { api } from "../api/client";

const mockUpNext = vi.mocked(api.upNext);
const mockSet = vi.mocked(api.setEpisodeWatched);

function item(overrides: Partial<UpNextItem> = {}): UpNextItem {
	return {
		show_tmdb_id: 1,
		show_name: "Show 1",
		poster_url: "/p.jpg",
		season_number: 2,
		episode_number: 5,
		episode_name: "The Episode",
		air_date: "2026-04-01",
		remaining: 3,
		...overrides,
	};
}

function renderPage() {
	return render(
		<MemoryRouter>
			<UpNext />
		</MemoryRouter>,
	);
}

beforeEach(() => {
	mockUpNext.mockReset();
	mockSet.mockReset();
});

afterEach(() => {
	vi.restoreAllMocks();
});

describe("UpNext", () => {
	it("renders items with padded SxxExx code, episode name, remaining, and air date", async () => {
		mockUpNext.mockResolvedValueOnce({ items: [item()] });
		renderPage();
		await waitFor(() => expect(screen.getByText("Show 1")).toBeInTheDocument());
		expect(screen.getByText("S02E05")).toBeInTheDocument();
		expect(screen.getByText("The Episode")).toBeInTheDocument();
		expect(screen.getByText("3 remaining")).toBeInTheDocument();
		expect(screen.getByText("aired 2026-04-01")).toBeInTheDocument();
	});

	it("handles missing poster and missing episode name", async () => {
		mockUpNext.mockResolvedValueOnce({
			items: [item({ poster_url: null, episode_name: null })],
		});
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("No poster")).toBeInTheDocument(),
		);
		expect(screen.queryByText("The Episode")).not.toBeInTheDocument();
	});

	it('uses singular "show" when only one item', async () => {
		mockUpNext.mockResolvedValueOnce({ items: [item()] });
		renderPage();
		await waitFor(() =>
			expect(screen.getByText(/1 show with unwatched/)).toBeInTheDocument(),
		);
	});

	it('uses plural "shows" when multiple items', async () => {
		mockUpNext.mockResolvedValueOnce({
			items: [item({ show_tmdb_id: 1 }), item({ show_tmdb_id: 2 })],
		});
		renderPage();
		await waitFor(() =>
			expect(screen.getByText(/2 shows with unwatched/)).toBeInTheDocument(),
		);
	});

	it("shows caught-up empty state", async () => {
		mockUpNext.mockResolvedValueOnce({ items: [] });
		renderPage();
		await waitFor(() =>
			expect(screen.getByText(/all caught up/)).toBeInTheDocument(),
		);
		expect(screen.getByRole("link", { name: "Search" })).toHaveAttribute(
			"href",
			"/search",
		);
	});

	it("shows error when initial load rejects", async () => {
		mockUpNext.mockRejectedValueOnce(new Error("nope"));
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Error: nope")).toBeInTheDocument(),
		);
	});

	it("shows generic error when rejection is not an Error", async () => {
		mockUpNext.mockRejectedValueOnce("boom");
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Error: Load failed")).toBeInTheDocument(),
		);
	});

	it("mark watched: calls API, reloads list, removes item when caught up", async () => {
		const user = userEvent.setup();
		mockUpNext.mockResolvedValueOnce({ items: [item()] });
		mockSet.mockResolvedValueOnce({} as never);
		mockUpNext.mockResolvedValueOnce({ items: [] });

		renderPage();
		const button = await screen.findByRole("button", { name: "Mark watched" });
		await user.click(button);

		await waitFor(() => expect(mockSet).toHaveBeenCalledWith(1, 2, 5, true));
		await waitFor(() =>
			expect(screen.getByText(/all caught up/)).toBeInTheDocument(),
		);
	});

	it("mark watched: surfaces an error from the API call", async () => {
		const user = userEvent.setup();
		mockUpNext.mockResolvedValueOnce({ items: [item()] });
		mockSet.mockRejectedValueOnce(new Error("upstream"));

		renderPage();
		const button = await screen.findByRole("button", { name: "Mark watched" });
		await user.click(button);

		await waitFor(() =>
			expect(screen.getByText("Error: upstream")).toBeInTheDocument(),
		);
	});

	it("mark watched: handles non-Error rejection", async () => {
		const user = userEvent.setup();
		mockUpNext.mockResolvedValueOnce({ items: [item()] });
		mockSet.mockRejectedValueOnce("weird");

		renderPage();
		const button = await screen.findByRole("button", { name: "Mark watched" });
		await user.click(button);

		await waitFor(() =>
			expect(screen.getByText("Error: Update failed")).toBeInTheDocument(),
		);
	});
});
