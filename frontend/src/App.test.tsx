import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Each page is exercised in its own test file. Here we just confirm the
// router wires the right path → component mapping.
vi.mock("./pages/Watchlist", () => ({
	default: () => <div>WatchlistPage</div>,
}));
vi.mock("./pages/UpNext", () => ({ default: () => <div>UpNextPage</div> }));
vi.mock("./pages/Search", () => ({ default: () => <div>SearchPage</div> }));
vi.mock("./pages/ShowDetail", () => ({ default: () => <div>DetailPage</div> }));
vi.mock("./pages/Calendar", () => ({ default: () => <div>CalendarPage</div> }));
vi.mock("./pages/Settings", () => ({ default: () => <div>SettingsPage</div> }));

let originalLocation: Location;

beforeEach(() => {
	originalLocation = window.location;
});

afterEach(() => {
	Object.defineProperty(window, "location", {
		value: originalLocation,
		writable: true,
		configurable: true,
	});
	vi.resetModules();
});

async function renderAt(path: string) {
	Object.defineProperty(window, "location", {
		value: { ...originalLocation, pathname: path, search: "", hash: "" },
		writable: true,
		configurable: true,
	});
	// Dynamically import so BrowserRouter picks up the new pathname.
	const { default: App } = await import("./App");
	return render(<App />);
}

describe("App routing", () => {
	it("renders UpNext at /", async () => {
		await renderAt("/");
		await waitFor(() =>
			expect(screen.getByText("UpNextPage")).toBeInTheDocument(),
		);
	});

	it("renders Watchlist at /watchlist", async () => {
		await renderAt("/watchlist");
		await waitFor(() =>
			expect(screen.getByText("WatchlistPage")).toBeInTheDocument(),
		);
	});

	it("renders Search at /search", async () => {
		await renderAt("/search");
		await waitFor(() =>
			expect(screen.getByText("SearchPage")).toBeInTheDocument(),
		);
	});

	it("renders ShowDetail at /shows/:id", async () => {
		await renderAt("/shows/42");
		await waitFor(() =>
			expect(screen.getByText("DetailPage")).toBeInTheDocument(),
		);
	});

	it("renders Calendar at /calendar", async () => {
		await renderAt("/calendar");
		await waitFor(() =>
			expect(screen.getByText("CalendarPage")).toBeInTheDocument(),
		);
	});

	it("renders Settings at /settings", async () => {
		await renderAt("/settings");
		await waitFor(() =>
			expect(screen.getByText("SettingsPage")).toBeInTheDocument(),
		);
	});
});
