import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import Layout from './Layout';

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route element={<Layout />}>
          <Route path="/" element={<div>HomeRouteContent</div>} />
          <Route path="/watchlist" element={<div>WatchlistRouteContent</div>} />
          <Route path="/calendar" element={<div>CalendarRouteContent</div>} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

describe('Layout', () => {
  it('renders nav links and brand', () => {
    renderAt('/');
    expect(screen.getByRole('link', { name: 'Showrunner' })).toHaveAttribute('href', '/');
    expect(screen.getByRole('link', { name: 'Watchlist' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Up Next' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Search' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Calendar' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Settings' })).toBeInTheDocument();
  });

  it('marks active nav link via NavLink', () => {
    renderAt('/watchlist');
    const watchlistLink = screen.getByRole('link', { name: 'Watchlist' });
    expect(watchlistLink).toHaveClass('active');
  });

  it('renders the active route content via Outlet', () => {
    renderAt('/calendar');
    expect(screen.getByText('CalendarRouteContent')).toBeInTheDocument();
  });
});
