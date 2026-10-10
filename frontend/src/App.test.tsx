import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

// Each page is exercised in its own test file. Here we just confirm the
// router wires the right path → component mapping.
vi.mock("./pages/Watchlist", () => ({
	default: () => <div>WatchlistPage</div>,
}));
vi.mock("./pages/UpNext", () => ({ default: () => <div>UpNextPage</div> }));
vi.mock("./pages/Search", () => ({ default: () => <div>SearchPage</div> }));
vi.mock("./pages/ShowDetail", () => ({ default: () => <div>DetailPage</div> }));
vi.mock("./pages/Calendar", () => ({ default: () => <div>CalendarPage</div> }));
vi.mock("./pages/History", () => ({ default: () => <div>HistoryPage</div> }));
vi.mock("./pages/Settings", () => ({ default: () => <div>SettingsPage</div> }));

afterEach(() => {
	window.history.replaceState({}, "", "/");
	vi.resetModules();
});

async function renderAt(path: string) {
	window.history.replaceState({}, "", path);
	// Dynamically import so BrowserRouter picks up the new pathname.
	const { default: App } = await import("./App");
	return render(<App />);
}

describe("App routing", () => {
	// @spec APP-SPA-001
	it("renders UpNext at /", async () => {
		await renderAt("/");
		await waitFor(() =>
			expect(screen.getByText("UpNextPage")).toBeInTheDocument(),
		);
	});

	// @spec APP-SPA-001
	it("renders Watchlist at /watchlist", async () => {
		await renderAt("/watchlist");
		await waitFor(() =>
			expect(screen.getByText("WatchlistPage")).toBeInTheDocument(),
		);
	});

	// @spec APP-SPA-001
	it("renders Search at /search", async () => {
		await renderAt("/search");
		await waitFor(() =>
			expect(screen.getByText("SearchPage")).toBeInTheDocument(),
		);
	});

	// @spec APP-SPA-001
	it("renders ShowDetail at /shows/:id", async () => {
		await renderAt("/shows/42");
		await waitFor(() =>
			expect(screen.getByText("DetailPage")).toBeInTheDocument(),
		);
	});

	// @spec APP-SPA-001
	it("renders Calendar at /calendar", async () => {
		await renderAt("/calendar");
		await waitFor(() =>
			expect(screen.getByText("CalendarPage")).toBeInTheDocument(),
		);
	});

	// @spec APP-SPA-001
	it("renders History at /history", async () => {
		await renderAt("/history");
		await waitFor(() =>
			expect(screen.getByText("HistoryPage")).toBeInTheDocument(),
		);
	});

	// @spec APP-SPA-001
	it("renders Settings at /settings", async () => {
		await renderAt("/settings");
		await waitFor(() =>
			expect(screen.getByText("SettingsPage")).toBeInTheDocument(),
		);
	});
});
