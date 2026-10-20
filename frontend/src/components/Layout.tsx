import { useEffect, useRef } from "react";
import { Link, NavLink, Outlet, useLocation } from "react-router";
import { useNarrow } from "../hooks/useNarrow";

// @spec APP-SPA-002, APP-SPA-009, APP-SPA-010, APP-SPA-013, APP-SPA-014, APP-SPA-015
export default function Layout() {
	const narrow = useNarrow();
	const { pathname } = useLocation();
	const strip = useRef<HTMLElement>(null);

	// While narrow the links scroll sideways, so bring the active one into view
	// whenever the route changes or the viewport becomes narrow.
	// biome-ignore lint/correctness/useExhaustiveDependencies: pathname is the trigger, not an input; the active link is read from the DOM after the route commits
	useEffect(() => {
		if (!narrow) return;
		const active = strip.current?.querySelector<HTMLElement>("a.active");
		if (active && typeof active.scrollIntoView === "function") {
			active.scrollIntoView({
				inline: "center",
				block: "nearest",
				behavior: "auto",
			});
		}
	}, [narrow, pathname]);

	return (
		<div className="layout">
			<header className="topnav">
				<div className="topnav-brand-row">
					<Link to="/" className="brand">
						Showrunner
					</Link>
				</div>
				<nav className="topnav-links" ref={strip}>
					<NavLink to="/" end>
						Up Next
					</NavLink>
					<NavLink to="/watchlist">Watchlist</NavLink>
					<NavLink to="/movies">Movies</NavLink>
					<NavLink to="/search">Search</NavLink>
					<NavLink to="/calendar">Calendar</NavLink>
					<NavLink to="/history">History</NavLink>
					<NavLink to="/settings">Settings</NavLink>
				</nav>
			</header>
			<main className="main">
				<Outlet />
			</main>
		</div>
	);
}
