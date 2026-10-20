import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { installMatchMedia } from "../test/matchMedia";
import Layout from "./Layout";

function renderAt(path: string) {
	return render(
		<MemoryRouter initialEntries={[path]}>
			<Routes>
				<Route element={<Layout />}>
					<Route path="/" element={<div>HomeRouteContent</div>} />
					<Route path="/watchlist" element={<div>WatchlistRouteContent</div>} />
					<Route path="/calendar" element={<div>CalendarRouteContent</div>} />
					<Route path="/settings" element={<div>SettingsRouteContent</div>} />
					<Route path="/shows/:tmdbId" element={<div>ShowRouteContent</div>} />
				</Route>
			</Routes>
		</MemoryRouter>,
	);
}

afterEach(() => {
	vi.restoreAllMocks();
});

describe("Layout", () => {
	// @spec APP-SPA-002
	it("renders nav links and brand", () => {
		renderAt("/");
		expect(screen.getByRole("link", { name: "Showrunner" })).toHaveAttribute(
			"href",
			"/",
		);
		expect(screen.getByRole("link", { name: "Watchlist" })).toBeInTheDocument();
		expect(screen.getByRole("link", { name: "Up Next" })).toBeInTheDocument();
		expect(screen.getByRole("link", { name: "Search" })).toBeInTheDocument();
		expect(screen.getByRole("link", { name: "Calendar" })).toBeInTheDocument();
		expect(screen.getByRole("link", { name: "History" })).toHaveAttribute(
			"href",
			"/history",
		);
		expect(screen.getByRole("link", { name: "Settings" })).toBeInTheDocument();
	});

	// @spec APP-SPA-002
	it("marks active nav link via NavLink", () => {
		renderAt("/watchlist");
		const watchlistLink = screen.getByRole("link", { name: "Watchlist" });
		expect(watchlistLink).toHaveClass("active");
	});

	it("renders the active route content via Outlet", () => {
		renderAt("/calendar");
		expect(screen.getByText("CalendarRouteContent")).toBeInTheDocument();
	});

	// @spec APP-SPA-009
	it("keeps the brand in its own row and the seven links as the strip's only children", () => {
		renderAt("/");
		const header = screen.getByRole("banner");
		const brandRow = header.querySelector(".topnav-brand-row");
		expect(brandRow).not.toBeNull();
		expect(
			within(brandRow as HTMLElement).getByRole("link", { name: "Showrunner" }),
		).toBeInTheDocument();

		const strip = screen.getByRole("navigation");
		expect(strip).toHaveClass("topnav-links");
		// The trailing spacer is a pseudo-element, so the DOM children are exactly the links.
		expect(strip.children).toHaveLength(7);
		for (const child of Array.from(strip.children)) {
			expect(child.tagName).toBe("A");
		}
	});

	describe("active link scrolling", () => {
		const scrollOptions = {
			inline: "center",
			block: "nearest",
			behavior: "auto",
		};

		// @spec APP-SPA-010
		it("scrolls the active link into view on first render while narrow", () => {
			installMatchMedia(true);
			const scroll = vi.spyOn(Element.prototype, "scrollIntoView");
			renderAt("/settings");
			expect(scroll).toHaveBeenCalledTimes(1);
			expect(scroll.mock.instances[0]).toBe(
				screen.getByRole("link", { name: "Settings" }),
			);
			expect(scroll).toHaveBeenCalledWith(scrollOptions);
		});

		// @spec APP-SPA-010
		it("scrolls again when the pathname changes", async () => {
			installMatchMedia(true);
			const scroll = vi.spyOn(Element.prototype, "scrollIntoView");
			const user = userEvent.setup();
			renderAt("/");
			expect(scroll).toHaveBeenCalledTimes(1);
			await user.click(screen.getByRole("link", { name: "Calendar" }));
			expect(scroll).toHaveBeenCalledTimes(2);
			expect(scroll.mock.instances[1]).toBe(
				screen.getByRole("link", { name: "Calendar" }),
			);
		});

		// @spec APP-SPA-010
		it("does not scroll while wide, then scrolls when the viewport becomes narrow", () => {
			const mm = installMatchMedia(false);
			const scroll = vi.spyOn(Element.prototype, "scrollIntoView");
			renderAt("/watchlist");
			expect(scroll).not.toHaveBeenCalled();
			act(() => mm.set(true));
			expect(scroll).toHaveBeenCalledTimes(1);
			expect(scroll.mock.instances[0]).toBe(
				screen.getByRole("link", { name: "Watchlist" }),
			);
			act(() => mm.set(false));
			act(() => mm.set(true));
			expect(scroll).toHaveBeenCalledTimes(2);
		});

		// @spec APP-SPA-010
		it("does nothing on a route with no active link", () => {
			installMatchMedia(true);
			const scroll = vi.spyOn(Element.prototype, "scrollIntoView");
			renderAt("/shows/42");
			expect(screen.getByText("ShowRouteContent")).toBeInTheDocument();
			expect(scroll).not.toHaveBeenCalled();
		});

		// @spec APP-SPA-010
		it("does nothing when the browser lacks scrollIntoView", () => {
			installMatchMedia(true);
			const original = Element.prototype.scrollIntoView;
			// @ts-expect-error simulating a browser without the method
			Element.prototype.scrollIntoView = undefined;
			try {
				expect(() => renderAt("/settings")).not.toThrow();
			} finally {
				Element.prototype.scrollIntoView = original;
			}
		});
	});
});
