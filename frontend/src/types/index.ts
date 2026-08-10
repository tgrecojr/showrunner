export type MediaType = "tv" | "movie";

export interface SearchResult {
	media_type: MediaType;
	tmdb_id: number;
	name: string;
	overview: string | null;
	date: string | null;
	poster_url: string | null;
	already_tracked: boolean;
}

export interface SearchResponse {
	results: SearchResult[];
}

export interface MovieWatchlistItem {
	tmdb_id: number;
	name: string;
	overview: string | null;
	poster_url: string | null;
	backdrop_url: string | null;
	release_date: string | null;
	runtime: number | null;
	added_at: string;
}

export interface MovieListResponse {
	movies: MovieWatchlistItem[];
}

export interface CastMember {
	name: string;
	character: string | null;
	profile_url: string | null;
}

export interface MovieDetail {
	tmdb_id: number;
	name: string;
	overview: string | null;
	poster_url: string | null;
	backdrop_url: string | null;
	release_date: string | null;
	runtime: number | null;
	watch_providers: string[];
	directors: string[];
	cast: CastMember[];
}

export interface WatchlistItem {
	tmdb_id: number;
	name: string;
	poster_url: string | null;
	status: string | null;
	in_production: boolean;
	watched_count: number;
	aired_count: number;
	total_count: number;
	next_episode_air_date: string | null;
}

export interface WatchlistResponse {
	shows: WatchlistItem[];
}

export interface EpisodeDetail {
	episode_number: number;
	name: string | null;
	overview: string | null;
	air_date: string | null;
	runtime: number | null;
	watched: boolean;
}

export interface SeasonDetail {
	season_number: number;
	name: string | null;
	overview: string | null;
	air_date: string | null;
	episode_count: number;
	watched_count: number;
	episodes: EpisodeDetail[];
}

export interface SyncError {
	tmdb_id: number;
	message: string;
}

export interface SyncResponse {
	shows_synced: number;
	errors: SyncError[];
}

export interface UpNextItem {
	show_tmdb_id: number;
	show_name: string;
	poster_url: string | null;
	season_number: number;
	episode_number: number;
	episode_name: string | null;
	episode_overview: string | null;
	air_date: string;
	remaining: number;
}

export interface UpNextResponse {
	items: UpNextItem[];
}

export interface CalendarEpisode {
	show_tmdb_id: number;
	show_name: string;
	poster_url: string | null;
	season_number: number;
	episode_number: number;
	episode_name: string | null;
	air_date: string;
	watched: boolean;
}

export interface CalendarResponse {
	episodes: CalendarEpisode[];
}

export interface ShowDetail {
	tmdb_id: number;
	name: string;
	overview: string | null;
	poster_url: string | null;
	backdrop_url: string | null;
	status: string | null;
	in_production: boolean;
	first_air_date: string | null;
	last_air_date: string | null;
	watch_providers: string[];
	seasons: SeasonDetail[];
}
