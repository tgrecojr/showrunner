import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MovieWatchlistItem } from "../types";
import Movies from "./Movies";

vi.mock("../api/client", () => ({
	api: { listMovies: vi.fn(), deleteMovie: vi.fn(), markMovieWatched: vi.fn() },
}));

import { api } from "../api/client";

const mockListMovies = vi.mocked(api.listMovies);
const mockDeleteMovie = vi.mocked(api.deleteMovie);
const mockMarkWatched = vi.mocked(api.markMovieWatched);

function buildMovie(
	overrides: Partial<MovieWatchlistItem> = {},
): MovieWatchlistItem {
	return {
		tmdb_id: 1,
		name: "Inception",
		overview: "dreams",
		poster_url: "/p.jpg",
		backdrop_url: "/b.jpg",
		release_date: "2010-07-16",
		runtime: 148,
		added_at: "2026-05-13T00:00:00Z",
		...overrides,
	};
}

function renderPage() {
	return render(
		<MemoryRouter>
			<Movies />
		</MemoryRouter>,
	);
}

beforeEach(() => {
	mockListMovies.mockReset();
	mockDeleteMovie.mockReset();
	mockMarkWatched.mockReset();
	vi.spyOn(window, "confirm").mockReturnValue(true);
});

afterEach(() => {
	vi.restoreAllMocks();
});

describe("Movies", () => {
	// @spec MOVIES-UI-001, MOVIES-UI-002
	it("shows loading then renders movies with year", async () => {
		mockListMovies.mockResolvedValueOnce({
			movies: [
				buildMovie({ tmdb_id: 1, name: "Inception" }),
				buildMovie({
					tmdb_id: 2,
					name: "Dune",
					poster_url: null,
					release_date: null,
				}),
			],
		});
		renderPage();
		expect(screen.getByText("Loading…")).toBeInTheDocument();

		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);
		expect(screen.getByText("Dune")).toBeInTheDocument();
		expect(screen.getByText("(2010)")).toBeInTheDocument();
		expect(screen.getByAltText("Inception poster")).toHaveAttribute(
			"src",
			"/p.jpg",
		);
		expect(screen.getByText("No poster")).toBeInTheDocument();
	});

	// @spec MOVIES-UI-001
	it("poster links to the movie detail page", async () => {
		mockListMovies.mockResolvedValueOnce({
			movies: [buildMovie({ tmdb_id: 42, name: "Inception" })],
		});
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);

		expect(
			screen.getByRole("link", { name: "View details for Inception" }),
		).toHaveAttribute("href", "/movies/42");
	});

	// @spec MOVIES-UI-002
	it("shows empty state with link to Search", async () => {
		mockListMovies.mockResolvedValueOnce({ movies: [] });
		renderPage();
		await waitFor(() =>
			expect(screen.getByText(/No movies yet/)).toBeInTheDocument(),
		);
		expect(
			screen.getByRole("link", { name: "Search for a movie" }),
		).toHaveAttribute("href", "/search");
	});

	// @spec MOVIES-UI-002
	it("shows error state when load fails", async () => {
		mockListMovies.mockRejectedValueOnce(new Error("boom"));
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Error: boom")).toBeInTheDocument(),
		);
	});

	// @spec MOVIES-UI-003
	it("Mark Watched calls markMovieWatched and removes the movie", async () => {
		const user = userEvent.setup();
		mockListMovies.mockResolvedValueOnce({
			movies: [buildMovie({ tmdb_id: 1, name: "Inception" })],
		});
		mockMarkWatched.mockResolvedValueOnce(undefined as never);
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);

		await user.click(screen.getByRole("button", { name: "Mark Watched" }));

		await waitFor(() => expect(mockMarkWatched).toHaveBeenCalledWith(1));
		expect(mockDeleteMovie).not.toHaveBeenCalled();
		await waitFor(() =>
			expect(screen.queryByText("Inception")).not.toBeInTheDocument(),
		);
	});

	// @spec MOVIES-UI-003
	it("Remove triggers deleteMovie and removes the card", async () => {
		const user = userEvent.setup();
		mockListMovies.mockResolvedValueOnce({
			movies: [buildMovie({ tmdb_id: 5, name: "Dune" })],
		});
		mockDeleteMovie.mockResolvedValueOnce(undefined as never);
		renderPage();
		await waitFor(() => expect(screen.getByText("Dune")).toBeInTheDocument());

		await user.click(screen.getByRole("button", { name: "Remove" }));

		await waitFor(() => expect(mockDeleteMovie).toHaveBeenCalledWith(5));
		expect(mockMarkWatched).not.toHaveBeenCalled();
		await waitFor(() =>
			expect(screen.queryByText("Dune")).not.toBeInTheDocument(),
		);
	});

	// @spec MOVIES-UI-003
	it("does nothing if the user cancels the confirm", async () => {
		vi.spyOn(window, "confirm").mockReturnValue(false);
		const user = userEvent.setup();
		mockListMovies.mockResolvedValueOnce({
			movies: [buildMovie({ tmdb_id: 1, name: "Inception" })],
		});
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);

		await user.click(screen.getByRole("button", { name: "Mark Watched" }));

		expect(mockDeleteMovie).not.toHaveBeenCalled();
		expect(mockMarkWatched).not.toHaveBeenCalled();
		expect(screen.getByText("Inception")).toBeInTheDocument();
	});

	// @spec MOVIES-UI-010
	it("shows a banner, keeps the list, and re-enables the card when delete fails", async () => {
		const user = userEvent.setup();
		mockListMovies.mockResolvedValueOnce({
			movies: [
				buildMovie({ tmdb_id: 1, name: "Inception" }),
				buildMovie({ tmdb_id: 2, name: "Dune" }),
			],
		});
		mockDeleteMovie.mockRejectedValueOnce(new Error("500"));
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);

		await user.click(
			screen.getAllByRole("button", { name: "Remove" })[0] as HTMLElement,
		);

		await waitFor(() =>
			expect(screen.getByText("Error: 500")).toBeInTheDocument(),
		);
		expect(screen.getByText("Inception")).toBeInTheDocument();
		expect(screen.getByText("Dune")).toBeInTheDocument();
		for (const button of screen.getAllByRole("button")) {
			expect(button).toBeEnabled();
		}
	});

	// @spec MOVIES-UI-010
	it("clears the banner when the next attempt begins", async () => {
		const user = userEvent.setup();
		mockListMovies.mockResolvedValueOnce({
			movies: [buildMovie({ tmdb_id: 1, name: "Inception" })],
		});
		mockMarkWatched.mockRejectedValueOnce(new Error("boom"));
		mockMarkWatched.mockResolvedValueOnce(undefined as never);
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);

		await user.click(screen.getByRole("button", { name: "Mark Watched" }));
		await waitFor(() =>
			expect(screen.getByText("Error: boom")).toBeInTheDocument(),
		);

		await user.click(screen.getByRole("button", { name: "Mark Watched" }));
		await waitFor(() =>
			expect(screen.queryByText("Error: boom")).not.toBeInTheDocument(),
		);
		expect(screen.queryByText("Inception")).not.toBeInTheDocument();
	});

	// @spec MOVIES-UI-010
	it("uses 'Update failed' when the rejection carries no message", async () => {
		const user = userEvent.setup();
		mockListMovies.mockResolvedValueOnce({
			movies: [buildMovie({ tmdb_id: 1, name: "Inception" })],
		});
		mockMarkWatched.mockRejectedValueOnce("nope");
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);

		await user.click(screen.getByRole("button", { name: "Mark Watched" }));
		await waitFor(() =>
			expect(screen.getByText("Error: Update failed")).toBeInTheDocument(),
		);
		expect(screen.getByText("Inception")).toBeInTheDocument();
	});
});
