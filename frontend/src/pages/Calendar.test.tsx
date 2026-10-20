import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { installMatchMedia } from "../test/matchMedia";
import type { CalendarEpisode } from "../types";
import Calendar from "./Calendar";

vi.mock("../api/client", () => ({
	api: { calendar: vi.fn() },
}));

import { api } from "../api/client";

const mockCalendar = vi.mocked(api.calendar);

function ep(overrides: Partial<CalendarEpisode> = {}): CalendarEpisode {
	return {
		show_tmdb_id: 1,
		show_name: "Show",
		poster_url: "/p.jpg",
		season_number: 2,
		episode_number: 5,
		episode_name: "Title",
		air_date: "2026-05-15",
		watched: false,
		...overrides,
	};
}

function renderPage() {
	return render(
		<MemoryRouter>
			<Calendar />
		</MemoryRouter>,
	);
}

beforeEach(() => {
	// Pin "today" to 2026-05-09.
	vi.useFakeTimers({ shouldAdvanceTime: true });
	vi.setSystemTime(new Date(2026, 4, 9, 12, 0, 0));
	mockCalendar.mockReset();
});

afterEach(() => {
	vi.useRealTimers();
	vi.restoreAllMocks();
});

describe("Calendar", () => {
	// @spec AIRING-CAL-006, AIRING-CAL-009
	it("renders current month title and episode in correct cell", async () => {
		mockCalendar.mockResolvedValueOnce({
			today: "2026-05-09",
			episodes: [ep()],
		});
		renderPage();
		expect(screen.getByText("May 2026")).toBeInTheDocument();
		await waitFor(() => expect(mockCalendar).toHaveBeenCalled());
		expect(screen.getByText("Show")).toBeInTheDocument();
		expect(screen.getByText("S02E05")).toBeInTheDocument();
	});

	// @spec AIRING-CAL-011
	it("marks the server's today, not the browser's, and the other-month cells", async () => {
		// The browser clock is pinned to May 9; the server (in its TIMEZONE)
		// says it is already May 10, and the server wins once it has answered.
		mockCalendar.mockResolvedValueOnce({ today: "2026-05-10", episodes: [] });
		const { container } = renderPage();
		await waitFor(() => expect(mockCalendar).toHaveBeenCalled());
		await waitFor(() =>
			expect(
				within(
					container.querySelector(".calendar-cell-today") as HTMLElement,
				).getByText("10"),
			).toBeInTheDocument(),
		);
		expect(container.querySelectorAll(".calendar-cell-today")).toHaveLength(1);

		const cells = container.querySelectorAll(".calendar-cell");
		expect(cells).toHaveLength(42);
		const today = container.querySelector(".calendar-cell-today");
		expect(today).not.toHaveClass("calendar-cell-other");

		// April 26–30 lead in (5 cells) and June 1–6 trail out (6 cells).
		const other = container.querySelectorAll(".calendar-cell-other");
		expect(other).toHaveLength(11);
		expect(cells[0]).toHaveClass("calendar-cell-other");
		expect(cells[41]).toHaveClass("calendar-cell-other");
		expect(cells[5]).not.toHaveClass("calendar-cell-other");
	});

	// @spec AIRING-CAL-006, AIRING-CAL-007
	it("queries with a 42-cell range covering the visible grid", async () => {
		mockCalendar.mockResolvedValueOnce({ today: "2026-05-09", episodes: [] });
		renderPage();
		await waitFor(() => expect(mockCalendar).toHaveBeenCalled());
		const [start, end] = mockCalendar.mock.calls[0]!;
		// First Sunday on/before May 1 2026 (a Friday) is April 26.
		expect(start).toBe("2026-04-26");
		// 42 cells later is June 6.
		expect(end).toBe("2026-06-06");
	});

	// @spec AIRING-CAL-008
	it("navigates to previous month", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockCalendar.mockResolvedValue({ today: "2026-05-09", episodes: [] });
		renderPage();
		await waitFor(() => expect(mockCalendar).toHaveBeenCalled());

		await user.click(screen.getByRole("button", { name: /Prev/ }));
		await waitFor(() =>
			expect(screen.getByText("April 2026")).toBeInTheDocument(),
		);
	});

	// @spec AIRING-CAL-008
	it("navigates next and crosses year boundary forward", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockCalendar.mockResolvedValue({ today: "2026-05-09", episodes: [] });
		renderPage();
		await waitFor(() => expect(mockCalendar).toHaveBeenCalled());

		// Click Next 7 times: May→Dec then Dec→Jan 2027.
		for (let i = 0; i < 8; i++) {
			await user.click(screen.getByRole("button", { name: /Next/ }));
		}
		await waitFor(() =>
			expect(screen.getByText("January 2027")).toBeInTheDocument(),
		);
	});

	// @spec AIRING-CAL-008
	it("navigates prev across year boundary backward", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockCalendar.mockResolvedValue({ today: "2026-05-09", episodes: [] });
		renderPage();
		await waitFor(() => expect(mockCalendar).toHaveBeenCalled());

		for (let i = 0; i < 5; i++) {
			await user.click(screen.getByRole("button", { name: /Prev/ }));
		}
		await waitFor(() =>
			expect(screen.getByText("December 2025")).toBeInTheDocument(),
		);
	});

	// @spec AIRING-CAL-008
	it("Today button returns to current month", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockCalendar.mockResolvedValue({ today: "2026-05-09", episodes: [] });
		renderPage();
		await waitFor(() => expect(mockCalendar).toHaveBeenCalled());

		await user.click(screen.getByRole("button", { name: /Next/ }));
		await waitFor(() =>
			expect(screen.getByText("June 2026")).toBeInTheDocument(),
		);

		await user.click(screen.getByRole("button", { name: "Today" }));
		await waitFor(() =>
			expect(screen.getByText("May 2026")).toBeInTheDocument(),
		);
	});

	// @spec AIRING-CAL-012, APP-SPA-006
	it("shows error when calendar fetch rejects", async () => {
		mockCalendar.mockRejectedValueOnce(new Error("range too big"));
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Error: range too big")).toBeInTheDocument(),
		);
	});

	// @spec AIRING-CAL-012
	it("shows generic error for non-Error rejection", async () => {
		mockCalendar.mockRejectedValueOnce("weird");
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Error: Load failed")).toBeInTheDocument(),
		);
	});

	// @spec AIRING-CAL-010
	it("renders watched episodes with the watched class and groups by date", async () => {
		mockCalendar.mockResolvedValueOnce({
			today: "2026-05-09",
			episodes: [
				ep({
					show_tmdb_id: 1,
					show_name: "A",
					air_date: "2026-05-05",
					watched: true,
				}),
				ep({ show_tmdb_id: 2, show_name: "B", air_date: "2026-05-05" }),
			],
		});
		const { container } = renderPage();
		await waitFor(() => expect(screen.getByText("A")).toBeInTheDocument());
		expect(screen.getByText("B")).toBeInTheDocument();
		const watched = container.querySelector(".calendar-ep-watched");
		expect(watched).not.toBeNull();
	});

	// @spec AIRING-CAL-009
	it("handles missing episode poster and link wraps each episode", async () => {
		mockCalendar.mockResolvedValueOnce({
			today: "2026-05-09",
			episodes: [ep({ poster_url: null, episode_name: null })],
		});
		renderPage();
		await waitFor(() => expect(screen.getByText("Show")).toBeInTheDocument());
		const link = screen.getByRole("link");
		expect(link).toHaveAttribute("href", "/shows/1");
		// No image rendered when poster_url is null
		const img = within(link).queryByRole("img");
		expect(img).toBeNull();
	});
});

describe("Calendar at narrow width (agenda list)", () => {
	const may = [
		ep({
			show_tmdb_id: 2,
			show_name: "Beta",
			air_date: "2026-05-15",
			episode_name: "Second on the day",
		}),
		ep({
			show_tmdb_id: 1,
			show_name: "Alpha",
			air_date: "2026-05-15",
			watched: true,
		}),
		ep({ show_tmdb_id: 3, show_name: "Gamma", air_date: "2026-05-05" }),
		// Padding days the 42-cell fetch returns but the agenda must not list.
		ep({ show_tmdb_id: 4, show_name: "AprilShow", air_date: "2026-04-28" }),
		ep({ show_tmdb_id: 5, show_name: "JuneShow", air_date: "2026-06-02" }),
	];

	// @spec AIRING-CAL-014, AIRING-CAL-006
	it("renders the agenda instead of the grid, keeping heading and buttons", async () => {
		installMatchMedia(true);
		mockCalendar.mockResolvedValueOnce({ today: "2026-05-09", episodes: may });
		const { container } = renderPage();
		await waitFor(() => expect(screen.getByText("Gamma")).toBeInTheDocument());
		expect(screen.getByText("May 2026")).toBeInTheDocument();
		expect(screen.getByRole("button", { name: /Prev/ })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Today" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: /Next/ })).toBeInTheDocument();
		expect(container.querySelector(".calendar-agenda")).not.toBeNull();
		expect(container.querySelector(".calendar-grid")).toBeNull();
		expect(container.querySelector(".calendar-weekday")).toBeNull();
	});

	// @spec AIRING-CAL-014, AIRING-CAL-007
	it("switching between grid and agenda does not refetch", async () => {
		const mm = installMatchMedia(true);
		mockCalendar.mockResolvedValue({ today: "2026-05-09", episodes: may });
		const { container } = renderPage();
		await waitFor(() => expect(screen.getByText("Gamma")).toBeInTheDocument());
		expect(mockCalendar).toHaveBeenCalledTimes(1);

		act(() => mm.set(false));
		await waitFor(() =>
			expect(container.querySelector(".calendar-grid")).not.toBeNull(),
		);
		expect(container.querySelector(".calendar-agenda")).toBeNull();
		expect(container.querySelectorAll(".calendar-cell")).toHaveLength(42);

		act(() => mm.set(true));
		await waitFor(() =>
			expect(container.querySelector(".calendar-agenda")).not.toBeNull(),
		);
		expect(container.querySelector(".calendar-grid")).toBeNull();
		expect(mockCalendar).toHaveBeenCalledTimes(1);
	});

	// @spec AIRING-CAL-015
	it("groups in-month airing days in date order with unpadded weekday headings and API order inside", async () => {
		installMatchMedia(true);
		mockCalendar.mockResolvedValueOnce({ today: "2026-05-09", episodes: may });
		const { container } = renderPage();
		await waitFor(() => expect(screen.getByText("Gamma")).toBeInTheDocument());

		const groups = container.querySelectorAll(".calendar-agenda-day");
		expect(groups).toHaveLength(2);
		const headings = Array.from(groups).map(
			(g) => g.querySelector(".calendar-agenda-date")?.textContent,
		);
		expect(headings).toEqual(["Tue 5", "Fri 15"]);

		const fifteenth = within(groups[1] as HTMLElement);
		const names = fifteenth
			.getAllByRole("link")
			.map((a) => a.querySelector(".calendar-ep-show")?.textContent);
		expect(names).toEqual(["Beta", "Alpha"]);

		expect(screen.queryByText("AprilShow")).toBeNull();
		expect(screen.queryByText("JuneShow")).toBeNull();
	});

	// @spec AIRING-CAL-015
	it("derives the weekday from the date string, not from a UTC parse", async () => {
		installMatchMedia(true);
		// 2026-05-01 is a Friday. A UTC parse rendered in a negative-offset zone
		// would land on Thursday April 30.
		mockCalendar.mockResolvedValueOnce({
			today: "2026-05-09",
			episodes: [ep({ air_date: "2026-05-01" })],
		});
		const { container } = renderPage();
		await waitFor(() =>
			expect(container.querySelector(".calendar-agenda-date")).not.toBeNull(),
		);
		expect(container.querySelector(".calendar-agenda-date")?.textContent).toBe(
			"Fri 1",
		);
	});

	// @spec AIRING-CAL-016
	it("reuses the grid chip markup and adds the visible episode name", async () => {
		installMatchMedia(true);
		mockCalendar.mockResolvedValueOnce({ today: "2026-05-09", episodes: may });
		const { container } = renderPage();
		await waitFor(() => expect(screen.getByText("Beta")).toBeInTheDocument());

		const items = container.querySelectorAll(".calendar-agenda .calendar-ep");
		expect(items).toHaveLength(3);

		const beta = Array.from(items).find((li) =>
			li.textContent?.includes("Beta"),
		) as HTMLElement;
		const link = within(beta).getByRole("link");
		expect(link).toHaveClass("calendar-ep-link");
		expect(link).toHaveAttribute("href", "/shows/2");
		expect(link).toHaveAttribute("title", "Second on the day");
		expect(beta.querySelector(".calendar-ep-code")?.textContent).toBe("S02E05");
		expect(beta.querySelector(".calendar-ep-poster")).toHaveAttribute(
			"src",
			"/p.jpg",
		);
		expect(beta.querySelector(".calendar-ep-name")?.textContent).toBe(
			"Second on the day",
		);
		expect(beta).not.toHaveClass("calendar-ep-watched");

		const alpha = Array.from(items).find((li) =>
			li.textContent?.includes("Alpha"),
		) as HTMLElement;
		expect(alpha).toHaveClass("calendar-ep-watched");
	});

	// @spec AIRING-CAL-016
	it("omits the visible name element when the episode has no name", async () => {
		installMatchMedia(true);
		mockCalendar.mockResolvedValueOnce({
			today: "2026-05-09",
			episodes: [ep({ episode_name: null })],
		});
		const { container } = renderPage();
		await waitFor(() => expect(screen.getByText("Show")).toBeInTheDocument());
		expect(container.querySelector(".calendar-ep-name")).toBeNull();
	});

	// @spec AIRING-CAL-017
	it("marks the group for the server's today, and none when today has no episodes", async () => {
		installMatchMedia(true);
		mockCalendar.mockResolvedValueOnce({ today: "2026-05-15", episodes: may });
		const { container, unmount } = renderPage();
		await waitFor(() => expect(screen.getByText("Beta")).toBeInTheDocument());
		const todayGroups = container.querySelectorAll(".calendar-agenda-today");
		expect(todayGroups).toHaveLength(1);
		expect(
			todayGroups[0]?.querySelector(".calendar-agenda-date")?.textContent,
		).toBe("Fri 15");
		unmount();

		mockCalendar.mockResolvedValueOnce({ today: "2026-05-09", episodes: may });
		const second = renderPage();
		await waitFor(() => expect(screen.getByText("Beta")).toBeInTheDocument());
		expect(second.container.querySelector(".calendar-agenda-today")).toBeNull();
	});

	// @spec AIRING-CAL-018, AIRING-CAL-012
	it("shows the empty-month text only once the fetch has settled with no episodes", async () => {
		installMatchMedia(true);
		let resolve: (v: { today: string; episodes: CalendarEpisode[] }) => void =
			() => {};
		mockCalendar.mockReturnValueOnce(
			new Promise((r) => {
				resolve = r;
			}),
		);
		renderPage();
		expect(screen.getByText("Loading…")).toBeInTheDocument();
		expect(screen.queryByText("Nothing airing in May 2026.")).toBeNull();

		await act(async () => {
			resolve({ today: "2026-05-09", episodes: [] });
		});
		await waitFor(() =>
			expect(
				screen.getByText("Nothing airing in May 2026."),
			).toBeInTheDocument(),
		);
		expect(screen.queryByText("Loading…")).toBeNull();
	});

	// @spec AIRING-CAL-018
	it("treats a month whose only episodes fall on padding days as empty", async () => {
		installMatchMedia(true);
		mockCalendar.mockResolvedValueOnce({
			today: "2026-05-09",
			episodes: [ep({ air_date: "2026-04-28" })],
		});
		renderPage();
		await waitFor(() =>
			expect(
				screen.getByText("Nothing airing in May 2026."),
			).toBeInTheDocument(),
		);
	});

	// @spec AIRING-CAL-012, APP-SPA-006
	it("shows the error above the agenda and keeps the month view", async () => {
		installMatchMedia(true);
		mockCalendar.mockRejectedValueOnce(new Error("range too big"));
		const { container } = renderPage();
		await waitFor(() =>
			expect(screen.getByText("Error: range too big")).toBeInTheDocument(),
		);
		expect(container.querySelector(".calendar-page")).not.toBeNull();
		expect(container.querySelector(".calendar-grid")).toBeNull();
	});
});
