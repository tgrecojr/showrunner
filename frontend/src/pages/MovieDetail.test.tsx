import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MovieDetail as MovieDetailType } from "../types";
import MovieDetail from "./MovieDetail";

vi.mock("../api/client", () => ({
	api: { getMovie: vi.fn(), deleteMovie: vi.fn(), markMovieWatched: vi.fn() },
}));

import { api } from "../api/client";

const mockGet = vi.mocked(api.getMovie);
const mockDelete = vi.mocked(api.deleteMovie);
const mockMarkWatched = vi.mocked(api.markMovieWatched);

function movie(overrides: Partial<MovieDetailType> = {}): MovieDetailType {
	return {
		tmdb_id: 27205,
		name: "Inception",
		overview: "A thief who steals corporate secrets…",
		poster_url: "/p.jpg",
		backdrop_url: "/b.jpg",
		release_date: "2010-07-16",
		runtime: 148,
		watch_providers: ["Netflix"],
		directors: ["Christopher Nolan"],
		cast: [
			{ name: "Leonardo DiCaprio", character: "Cobb", profile_url: "/leo.jpg" },
			{ name: "Joseph Gordon-Levitt", character: "Arthur", profile_url: null },
		],
		...overrides,
	};
}

function renderPage() {
	return render(
		<MemoryRouter initialEntries={["/movies/27205"]}>
			<Routes>
				<Route path="/movies/:tmdbId" element={<MovieDetail />} />
				<Route path="/movies" element={<div>Movies List</div>} />
			</Routes>
		</MemoryRouter>,
	);
}

beforeEach(() => {
	mockGet.mockReset();
	mockDelete.mockReset();
	mockMarkWatched.mockReset();
	vi.spyOn(window, "confirm").mockReturnValue(true);
});

afterEach(() => {
	vi.restoreAllMocks();
});

describe("MovieDetail", () => {
	// @spec MOVIES-UI-006, MOVIES-UI-007
	it("renders movie details, director, providers, and cast", async () => {
		mockGet.mockResolvedValueOnce(movie());
		renderPage();

		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);
		expect(screen.getByText("2010")).toBeInTheDocument();
		expect(screen.getByText("148 min")).toBeInTheDocument();
		expect(screen.getByText("Director:")).toBeInTheDocument();
		expect(screen.getByText("Christopher Nolan")).toBeInTheDocument();
		expect(screen.getByText("Watch on:")).toBeInTheDocument();
		expect(screen.getByText("Netflix")).toBeInTheDocument();
		expect(screen.getByText("Leonardo DiCaprio")).toBeInTheDocument();
		expect(screen.getByText("as Cobb")).toBeInTheDocument();
		expect(screen.getByText("No photo")).toBeInTheDocument();
	});

	// @spec APP-SPA-006
	it("shows the server's rate-limit sentence exactly as delivered", async () => {
		mockGet.mockRejectedValueOnce(
			new Error(
				"TMDB is rate-limiting requests right now. Please try again in a moment.",
			),
		);
		renderPage();

		await waitFor(() =>
			expect(
				screen.getByText(
					"Error: TMDB is rate-limiting requests right now. Please try again in a moment.",
				),
			).toBeInTheDocument(),
		);
	});

	// @spec MOVIES-UI-008
	it("Mark Watched calls markMovieWatched and navigates to /movies", async () => {
		const user = userEvent.setup();
		mockGet.mockResolvedValueOnce(movie());
		mockMarkWatched.mockResolvedValueOnce(undefined as never);
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);

		await user.click(screen.getByRole("button", { name: "Mark Watched" }));

		await waitFor(() => expect(mockMarkWatched).toHaveBeenCalledWith(27205));
		expect(mockDelete).not.toHaveBeenCalled();
		await waitFor(() =>
			expect(screen.getByText("Movies List")).toBeInTheDocument(),
		);
	});

	// @spec MOVIES-UI-008
	it("Remove calls deleteMovie and navigates to /movies", async () => {
		const user = userEvent.setup();
		mockGet.mockResolvedValueOnce(movie());
		mockDelete.mockResolvedValueOnce(undefined as never);
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);

		await user.click(screen.getByRole("button", { name: "Remove" }));

		await waitFor(() => expect(mockDelete).toHaveBeenCalledWith(27205));
		expect(mockMarkWatched).not.toHaveBeenCalled();
		await waitFor(() =>
			expect(screen.getByText("Movies List")).toBeInTheDocument(),
		);
	});

	// @spec MOVIES-UI-008
	it("shows banner error and stays on page when marking fails", async () => {
		const user = userEvent.setup();
		mockGet.mockResolvedValueOnce(movie());
		mockMarkWatched.mockRejectedValueOnce(new Error("boom"));
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Inception")).toBeInTheDocument(),
		);

		await user.click(screen.getByRole("button", { name: "Mark Watched" }));

		await waitFor(() =>
			expect(screen.getByText("Error: boom")).toBeInTheDocument(),
		);
		expect(screen.getByText("Inception")).toBeInTheDocument();
	});

	// @spec MOVIES-UI-005
	it("shows error for an invalid movie id in the URL", async () => {
		render(
			<MemoryRouter initialEntries={["/movies/not-a-number"]}>
				<Routes>
					<Route path="/movies/:tmdbId" element={<MovieDetail />} />
				</Routes>
			</MemoryRouter>,
		);
		expect(screen.getByText("Error: Invalid movie id")).toBeInTheDocument();
		expect(mockGet).not.toHaveBeenCalled();
	});

	// @spec APP-SPA-006
	it("does not rewrite a raw upstream status message", async () => {
		mockGet.mockRejectedValueOnce(new Error("TMDB returned 503"));
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("Error: TMDB returned 503")).toBeInTheDocument(),
		);
	});

	// @spec MOVIES-UI-006, MOVIES-UI-007
	it("renders an empty cast section when TMDB returns no cast", async () => {
		mockGet.mockResolvedValueOnce(
			movie({ cast: [], directors: [], watch_providers: [] }),
		);
		renderPage();
		await waitFor(() =>
			expect(
				screen.getByText("No cast information available."),
			).toBeInTheDocument(),
		);
		expect(screen.queryByText("Director:")).not.toBeInTheDocument();
		expect(screen.queryByText("Watch on:")).not.toBeInTheDocument();
	});

	// @spec MOVIES-UI-006
	it("handles missing poster, backdrop, runtime, and release date gracefully", async () => {
		mockGet.mockResolvedValueOnce(
			movie({
				poster_url: null,
				backdrop_url: null,
				runtime: null,
				release_date: null,
			}),
		);
		renderPage();
		await waitFor(() =>
			expect(screen.getByText("No poster")).toBeInTheDocument(),
		);
		expect(screen.queryByText("148 min")).not.toBeInTheDocument();
		expect(screen.queryByText("2010")).not.toBeInTheDocument();
	});
});
