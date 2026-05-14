import { BrowserRouter, Route, Routes } from 'react-router-dom';
import Layout from './components/Layout';
import Watchlist from './pages/Watchlist';
import Movies from './pages/Movies';
import MovieDetail from './pages/MovieDetail';
import Search from './pages/Search';
import Calendar from './pages/Calendar';
import ShowDetail from './pages/ShowDetail';
import Settings from './pages/Settings';
import UpNext from './pages/UpNext';

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<Layout />}>
          <Route path="/" element={<UpNext />} />
          <Route path="/watchlist" element={<Watchlist />} />
          <Route path="/movies" element={<Movies />} />
          <Route path="/movies/:tmdbId" element={<MovieDetail />} />
          <Route path="/search" element={<Search />} />
          <Route path="/shows/:tmdbId" element={<ShowDetail />} />
          <Route path="/calendar" element={<Calendar />} />
          <Route path="/settings" element={<Settings />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
