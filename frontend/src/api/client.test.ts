import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./client";

type FetchArgs = [RequestInfo | URL, RequestInit | undefined];

let fetchSpy: ReturnType<typeof vi.fn>;

beforeEach(() => {
	fetchSpy = vi.fn();
	globalThis.fetch = fetchSpy as unknown as typeof fetch;
});

afterEach(() => {
	vi.restoreAllMocks();
});

function jsonResponse(status: number, body: unknown): Response {
	return new Response(JSON.stringify(body), {
		status,
		headers: { "content-type": "application/json" },
	});
}

function textResponse(status: number, body: string): Response {
	return new Response(body, { status });
}

describe("api client", () => {
	it("builds /api/v1 URLs with JSON content-type and parses JSON success", async () => {
		fetchSpy.mockResolvedValueOnce(jsonResponse(200, { results: [] }));
		await api.search("bear");
		const [url, init] = fetchSpy.mock.calls[0] as FetchArgs;
		expect(url).toBe("/api/v1/search?q=bear");
		expect(init?.headers).toEqual({ "Content-Type": "application/json" });
	});

	it("encodes search query strings safely", async () => {
		fetchSpy.mockResolvedValueOnce(jsonResponse(200, { results: [] }));
		await api.search("the bear & friends");
		const [url] = fetchSpy.mock.calls[0] as FetchArgs;
		expect(url).toBe("/api/v1/search?q=the%20bear%20%26%20friends");
	});

	it("returns undefined for 204 No Content", async () => {
		fetchSpy.mockResolvedValueOnce(new Response(null, { status: 204 }));
		const result = await api.deleteShow(1);
		expect(result).toBeUndefined();
	});

	it('throws Error("API <status>: <error>") when JSON body has error field', async () => {
		fetchSpy.mockResolvedValueOnce(
			jsonResponse(404, { error: "show 9 missing" }),
		);
		await expect(api.getShow(9)).rejects.toThrowError(
			"API 404: show 9 missing",
		);
	});

	it("falls back to raw text when error body is not JSON", async () => {
		fetchSpy.mockResolvedValueOnce(textResponse(500, "plain failure"));
		await expect(api.listShows()).rejects.toThrowError(
			"API 500: plain failure",
		);
	});

	it("falls back to raw text when JSON has no error field", async () => {
		fetchSpy.mockResolvedValueOnce(jsonResponse(400, { other: "noise" }));
		await expect(api.listShows()).rejects.toThrowError(/API 400/);
	});

	it("listShows hits /shows", async () => {
		fetchSpy.mockResolvedValueOnce(jsonResponse(200, { shows: [] }));
		const r = await api.listShows();
		const [url] = fetchSpy.mock.calls[0] as FetchArgs;
		expect(url).toBe("/api/v1/shows");
		expect(r).toEqual({ shows: [] });
	});

	it("upNext hits /up-next", async () => {
		fetchSpy.mockResolvedValueOnce(jsonResponse(200, { items: [] }));
		await api.upNext();
		const [url] = fetchSpy.mock.calls[0] as FetchArgs;
		expect(url).toBe("/api/v1/up-next");
	});

	it("addShow POSTs JSON body with tmdb_id", async () => {
		fetchSpy.mockResolvedValueOnce(jsonResponse(201, { tmdb_id: 7 }));
		await api.addShow(7);
		const [url, init] = fetchSpy.mock.calls[0] as FetchArgs;
		expect(url).toBe("/api/v1/shows");
		expect(init?.method).toBe("POST");
		expect(init?.body).toBe(JSON.stringify({ tmdb_id: 7 }));
	});

	it("getShow hits /shows/:id", async () => {
		fetchSpy.mockResolvedValueOnce(jsonResponse(200, { tmdb_id: 5 }));
		await api.getShow(5);
		const [url] = fetchSpy.mock.calls[0] as FetchArgs;
		expect(url).toBe("/api/v1/shows/5");
	});

	it("deleteShow uses DELETE method", async () => {
		fetchSpy.mockResolvedValueOnce(new Response(null, { status: 204 }));
		await api.deleteShow(5);
		const [url, init] = fetchSpy.mock.calls[0] as FetchArgs;
		expect(url).toBe("/api/v1/shows/5");
		expect(init?.method).toBe("DELETE");
	});

	it("setEpisodeWatched PATCHes with body", async () => {
		fetchSpy.mockResolvedValueOnce(jsonResponse(200, { tmdb_id: 1 }));
		await api.setEpisodeWatched(1, 2, 3, true);
		const [url, init] = fetchSpy.mock.calls[0] as FetchArgs;
		expect(url).toBe("/api/v1/episodes/1/2/3");
		expect(init?.method).toBe("PATCH");
		expect(init?.body).toBe(JSON.stringify({ watched: true }));
	});

	it("bulkWatch POSTs scope + watched", async () => {
		fetchSpy.mockResolvedValueOnce(jsonResponse(200, { tmdb_id: 1 }));
		await api.bulkWatch(1, { type: "all" }, true);
		const [url, init] = fetchSpy.mock.calls[0] as FetchArgs;
		expect(url).toBe("/api/v1/shows/1/bulk-watch");
		expect(JSON.parse(init?.body as string)).toEqual({
			scope: { type: "all" },
			watched: true,
		});
	});

	it("calendar passes start/end as query params", async () => {
		fetchSpy.mockResolvedValueOnce(jsonResponse(200, { episodes: [] }));
		await api.calendar("2026-01-01", "2026-01-31");
		const [url] = fetchSpy.mock.calls[0] as FetchArgs;
		expect(url).toBe("/api/v1/calendar?start=2026-01-01&end=2026-01-31");
	});

	it("sync hits POST /sync", async () => {
		fetchSpy.mockResolvedValueOnce(
			jsonResponse(200, { shows_synced: 0, errors: [] }),
		);
		await api.sync();
		const [url, init] = fetchSpy.mock.calls[0] as FetchArgs;
		expect(url).toBe("/api/v1/sync");
		expect(init?.method).toBe("POST");
	});
});
