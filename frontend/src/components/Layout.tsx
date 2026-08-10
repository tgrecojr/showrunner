import { Link, NavLink, Outlet } from "react-router";

export default function Layout() {
	return (
		<div className="layout">
			<header className="topnav">
				<Link to="/" className="brand">
					Showrunner
				</Link>
				<nav>
					<NavLink to="/" end>
						Up Next
					</NavLink>
					<NavLink to="/watchlist">Watchlist</NavLink>
					<NavLink to="/movies">Movies</NavLink>
					<NavLink to="/search">Search</NavLink>
					<NavLink to="/calendar">Calendar</NavLink>
					<NavLink to="/settings">Settings</NavLink>
				</nav>
			</header>
			<main className="main">
				<Outlet />
			</main>
		</div>
	);
}
