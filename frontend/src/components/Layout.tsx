import { NavLink, Outlet } from 'react-router-dom';

export default function Layout() {
  return (
    <div className="layout">
      <header className="topnav">
        <div className="brand">Showrunner</div>
        <nav>
          <NavLink to="/" end>Watchlist</NavLink>
          <NavLink to="/up-next">Up Next</NavLink>
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
