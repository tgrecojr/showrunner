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
		networks: ["NBC", "Peacock"],
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
	// @spec AIRING-UPNEXT-006
	it("renders items with padded SxxExx code, episode name, remaining, and air date", async () => {
		mockUpNext.mockResolvedValueOnce({ items: [item()] });
		renderPage();
		await waitFor(() => expect(screen.getByText("Show 1")).toBeInTheDocument());
		expect(screen.getByText("S02E05")).toBeInTheDocument();
		expect(screen.getByText("The Episode")).toBeInTheDocument();
		expect(screen.getByText("3 remaining")).toBeInTheDocument();
		expect(screen.getByText("aired 2026-04-01")).toBeInTheDocument();
	});

	// @spec AIRING-UPNEXT-007
	it("accents the remaining pill when more than one aired episode is unwatched", async () => {
		mockUpNext.mockResolvedValueOnce({ items: [item({ remaining: 2 })] });
		renderPage();
		const pill = await screen.findByText("2 remaining");
		expect(pill).toHaveClass("status-pill", "status-pill-accent");
		expect(pill).toHaveAttribute("title", "2 aired episodes not yet watched");
	});

	// @spec AIRING-UPNEXT-007
	it("keeps the remaining pill neutral when only one episode is unwatched", async () => {
		mockUpNext.mockResolvedValueOnce({ items: [item({ remaining: 1 })] });
		renderPage();
		const pill = await screen.findByText("1 remaining");
		expect(pill).toHaveClass("status-pill");
		expect(pill).not.toHaveClass("status-pill-accent");
		expect(pill).toHaveAttribute("title", "1 aired episode not yet watched");
	});

	// @spec AIRING-UPNEXT-006
	it("renders a pill per network, and none when the list is empty", async () => {
		mockUpNext.mockResolvedValueOnce({
			items: [
				item(),
				item({ show_tmdb_id: 2, show_name: "Show 2", networks: [] }),
			],
		});
		const { container } = renderPage();
		await waitFor(() => expect(screen.getByText("Show 2")).toBeInTheDocument());
		expect(screen.getByText("NBC")).toBeInTheDocument();
		expect(screen.getByText("Peacock")).toBeInTheDocument();
		expect(container.querySelectorAll(".network-pill")).toHaveLength(2);
	});

	// @spec AIRING-UPNEXT-006
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

	// @spec AIRING-UPNEXT-008
	it('uses singular "show" when only one item', async () => {
		mockUpNext.mockResolvedValueOnce({ items: [item()] });
		renderPage();
		await waitFor(() =>
			expect(screen.getByText(/1 show with unwatched/)).toBeInTheDocument(),
		);
	});

	// @spec AIRING-UPNEXT-008
	it('uses plural "shows" when multiple items', async () => {
		mockUpNext.mockResolvedValueOnce({
			items: [item({ show_tmdb_id: 1 }), item({ show_tmdb_id: 2 })],
		});
		renderPage();
		await waitFor(() =>
			expect(screen.getByText(/2 shows with unwatched/)).toBeInTheDocument(),
		);
	});

	// @spec AIRING-UPNEXT-009
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

	// @spec AIRING-UPNEXT-010, APP-SPA-006
	it("shows error when initial load rejects", async () => {
		mockUpNext.mockRejectedValueOnce(new Error("nope"));
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Error: nope")).toBeInTheDocument(),
		);
	});

	// @spec AIRING-UPNEXT-010
	it("shows generic error when rejection is not an Error", async () => {
		mockUpNext.mockRejectedValueOnce("boom");
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Error: Load failed")).toBeInTheDocument(),
		);
	});

	// @spec AIRING-UPNEXT-011
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

	// @spec AIRING-UPNEXT-012, APP-SPA-006
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

	// @spec AIRING-UPNEXT-012
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
