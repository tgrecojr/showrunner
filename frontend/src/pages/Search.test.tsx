import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SearchResult } from "../types";
import Search from "./Search";

vi.mock("../api/client", () => ({
	api: { search: vi.fn(), addShow: vi.fn(), addMovie: vi.fn() },
}));

import { api } from "../api/client";

const mockSearch = vi.mocked(api.search);
const mockAddShow = vi.mocked(api.addShow);
const mockAddMovie = vi.mocked(api.addMovie);

function tvResult(overrides: Partial<SearchResult> = {}): SearchResult {
	return {
		media_type: "tv",
		tmdb_id: 1,
		name: "Show",
		overview: "description",
		date: "2024-05-01",
		poster_url: "/p.jpg",
		already_tracked: false,
		...overrides,
	};
}

function movieResult(overrides: Partial<SearchResult> = {}): SearchResult {
	return {
		media_type: "movie",
		tmdb_id: 100,
		name: "Movie Title",
		overview: "movie description",
		date: "2010-07-16",
		poster_url: "/m.jpg",
		already_tracked: false,
		...overrides,
	};
}

function renderPage() {
	return render(
		<MemoryRouter>
			<Search />
		</MemoryRouter>,
	);
}

const SEARCH_LABEL = "Search movies and TV shows";

beforeEach(() => {
	vi.useFakeTimers({ shouldAdvanceTime: true });
	mockSearch.mockReset();
	mockAddShow.mockReset();
	mockAddMovie.mockReset();
});

afterEach(() => {
	vi.useRealTimers();
	vi.restoreAllMocks();
});

async function flushDebounce() {
	await act(async () => {
		await vi.advanceTimersByTimeAsync(400);
	});
}

describe("Search", () => {
	// @spec SEARCH-UI-001
	it("labels the query input and focuses it on mount", () => {
		renderPage();
		const input = screen.getByLabelText(SEARCH_LABEL);
		expect(input).toHaveAttribute("type", "search");
		expect(input).toHaveFocus();
	});

	// @spec SEARCH-UI-004
	it("shows Searching… while a request is pending and keeps the previous results", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({
			results: [tvResult({ name: "First" })],
		});
		let settle: (v: { results: SearchResult[] }) => void = () => {};
		mockSearch.mockReturnValueOnce(
			new Promise((resolve) => {
				settle = resolve;
			}),
		);
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "bear");
		await flushDebounce();
		await waitFor(() => expect(screen.getByText("First")).toBeInTheDocument());
		expect(screen.queryByText("Searching…")).not.toBeInTheDocument();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "s");
		await flushDebounce();
		await waitFor(() =>
			expect(screen.getByText("Searching…")).toBeInTheDocument(),
		);
		expect(screen.getByText("First")).toBeInTheDocument();

		settle({ results: [tvResult({ name: "Second" })] });
		await waitFor(() => expect(screen.getByText("Second")).toBeInTheDocument());
		expect(screen.queryByText("Searching…")).not.toBeInTheDocument();
	});

	// @spec SEARCH-UI-010
	it("disables the clicked card's Add as Adding… while other cards stay enabled", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({
			results: [
				tvResult({ tmdb_id: 1, name: "One" }),
				tvResult({ tmdb_id: 2, name: "Two" }),
			],
		});
		let settle: () => void = () => {};
		mockAddShow.mockReturnValueOnce(
			new Promise((resolve) => {
				settle = () => resolve({} as never);
			}),
		);
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "q");
		await flushDebounce();
		const oneCard = (await screen.findByText("One")).closest(
			"li",
		) as HTMLElement;
		const twoCard = screen.getByText("Two").closest("li") as HTMLElement;

		await user.click(within(oneCard).getByRole("button", { name: "Add" }));

		await waitFor(() =>
			expect(
				within(oneCard).getByRole("button", { name: "Adding…" }),
			).toBeDisabled(),
		);
		expect(within(twoCard).getByRole("button", { name: "Add" })).toBeEnabled();

		settle();
		await waitFor(() =>
			expect(within(oneCard).getByText("On watchlist")).toBeInTheDocument(),
		);
	});

	// @spec SEARCH-UI-012
	it("keys add state by media type and id, and resets it when new results arrive", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({
			results: [
				tvResult({ tmdb_id: 7, name: "Same Id TV" }),
				movieResult({ tmdb_id: 7, name: "Same Id Movie" }),
			],
		});
		mockSearch.mockResolvedValueOnce({
			results: [tvResult({ tmdb_id: 7, name: "Same Id TV" })],
		});
		mockAddShow.mockResolvedValueOnce({} as never);
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "same");
		await flushDebounce();
		const tvCard = (await screen.findByText("Same Id TV")).closest(
			"li",
		) as HTMLElement;
		const movieCard = screen
			.getByText("Same Id Movie")
			.closest("li") as HTMLElement;

		await user.click(within(tvCard).getByRole("button", { name: "Add" }));
		await waitFor(() =>
			expect(within(tvCard).getByText("On watchlist")).toBeInTheDocument(),
		);
		expect(
			within(movieCard).getByRole("button", { name: "Add" }),
		).toBeEnabled();
		expect(mockAddMovie).not.toHaveBeenCalled();

		await user.type(screen.getByLabelText(SEARCH_LABEL), " id");
		await flushDebounce();
		await waitFor(() => expect(mockSearch).toHaveBeenCalledTimes(2));
		const refreshedCard = (await screen.findByText("Same Id TV")).closest(
			"li",
		) as HTMLElement;
		await waitFor(() =>
			expect(
				within(refreshedCard).getByRole("button", { name: "Add" }),
			).toBeEnabled(),
		);
		expect(screen.queryByText("On watchlist")).not.toBeInTheDocument();
	});

	// @spec SEARCH-UI-002, SEARCH-UI-007
	it("debounces query and renders results", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({ results: [tvResult({ tmdb_id: 99 })] });
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "bear");
		expect(mockSearch).not.toHaveBeenCalled();

		await flushDebounce();
		await waitFor(() => expect(mockSearch).toHaveBeenCalledWith("bear"));
		await waitFor(() => expect(screen.getByText("Show")).toBeInTheDocument());
		expect(screen.getByText("(2024)")).toBeInTheDocument();
		expect(screen.getByText("description")).toBeInTheDocument();
	});

	// @spec SEARCH-UI-003
	it("clears results when query becomes empty", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({ results: [tvResult()] });
		renderPage();

		const input = screen.getByLabelText(SEARCH_LABEL);
		await user.type(input, "bear");
		await flushDebounce();
		await waitFor(() => expect(screen.getByText("Show")).toBeInTheDocument());

		await user.clear(input);
		await waitFor(() =>
			expect(screen.queryByText("Show")).not.toBeInTheDocument(),
		);
	});

	// @spec SEARCH-UI-005
	it("shows no-matches message when results array is empty", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({ results: [] });
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "xyzzy");
		await flushDebounce();
		await waitFor(() =>
			expect(screen.getByText("No matches.")).toBeInTheDocument(),
		);
	});

	// @spec SEARCH-UI-006
	it("shows search error", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockRejectedValueOnce(new Error("500"));
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "q");
		await flushDebounce();
		await waitFor(() =>
			expect(screen.getByText("Error: 500")).toBeInTheDocument(),
		);
	});

	// @spec SEARCH-UI-006
	it("shows generic search error for non-Error rejection", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockRejectedValueOnce("boom");
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "q");
		await flushDebounce();
		await waitFor(() =>
			expect(screen.getByText("Error: Search failed")).toBeInTheDocument(),
		);
	});

	// @spec SEARCH-UI-008
	it("marks already-tracked items with a chip rather than an Add button", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({
			results: [tvResult({ already_tracked: true })],
		});
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "q");
		await flushDebounce();
		await waitFor(() =>
			expect(screen.getByText("On watchlist")).toBeInTheDocument(),
		);
		expect(
			screen.queryByRole("button", { name: "Add" }),
		).not.toBeInTheDocument();
	});

	// @spec SEARCH-UI-008, SEARCH-UI-009
	it('adds a TV show, transitions to "On watchlist"', async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({ results: [tvResult({ tmdb_id: 7 })] });
		mockAddShow.mockResolvedValueOnce({} as never);
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "q");
		await flushDebounce();
		const addBtn = await screen.findByRole("button", { name: "Add" });
		await user.click(addBtn);

		await waitFor(() => expect(mockAddShow).toHaveBeenCalledWith(7));
		expect(mockAddMovie).not.toHaveBeenCalled();
		await waitFor(() =>
			expect(screen.getByText("On watchlist")).toBeInTheDocument(),
		);
	});

	// @spec SEARCH-UI-008, SEARCH-UI-009
	it("adds a movie via addMovie, not addShow", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({
			results: [movieResult({ tmdb_id: 27205 })],
		});
		mockAddMovie.mockResolvedValueOnce({} as never);
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "q");
		await flushDebounce();
		const addBtn = await screen.findByRole("button", { name: "Add" });
		await user.click(addBtn);

		await waitFor(() => expect(mockAddMovie).toHaveBeenCalledWith(27205));
		expect(mockAddShow).not.toHaveBeenCalled();
		await waitFor(() =>
			expect(screen.getByText("On watchlist")).toBeInTheDocument(),
		);
	});

	// @spec SEARCH-UI-007
	it("renders media type badges for both kinds", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({
			results: [tvResult({ tmdb_id: 1 }), movieResult({ tmdb_id: 2 })],
		});
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "q");
		await flushDebounce();
		await waitFor(() => expect(screen.getByText("Show")).toBeInTheDocument());
		expect(screen.getByText("TV")).toBeInTheDocument();
		expect(screen.getByText("Movie")).toBeInTheDocument();
	});

	// @spec SEARCH-UI-011
	it("shows per-result error when add fails", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({ results: [tvResult({ tmdb_id: 7 })] });
		mockAddShow.mockRejectedValueOnce(new Error("duplicate"));
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "q");
		await flushDebounce();
		const addBtn = await screen.findByRole("button", { name: "Add" });
		await user.click(addBtn);

		await waitFor(() =>
			expect(screen.getByText("duplicate")).toBeInTheDocument(),
		);
	});

	// @spec SEARCH-UI-011
	it("shows generic add error for non-Error rejection", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({ results: [tvResult({ tmdb_id: 7 })] });
		mockAddShow.mockRejectedValueOnce("weird");
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "q");
		await flushDebounce();
		const addBtn = await screen.findByRole("button", { name: "Add" });
		await user.click(addBtn);

		await waitFor(() =>
			expect(screen.getByText("Add failed")).toBeInTheDocument(),
		);
	});

	// @spec SEARCH-UI-007
	it("renders placeholder card when poster is missing and date is null", async () => {
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		mockSearch.mockResolvedValueOnce({
			results: [tvResult({ poster_url: null, date: null, overview: null })],
		});
		renderPage();

		await user.type(screen.getByLabelText(SEARCH_LABEL), "q");
		await flushDebounce();
		await waitFor(() =>
			expect(screen.getByText("No poster")).toBeInTheDocument(),
		);
		expect(screen.queryByText(/\(202\d\)/)).not.toBeInTheDocument();
	});
});
