import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { installMatchMedia, removeMatchMedia } from "../test/matchMedia";
import { NARROW_QUERY, useNarrow } from "./useNarrow";

afterEach(() => {
	vi.restoreAllMocks();
});

describe("useNarrow", () => {
	// @spec APP-SPA-007, APP-SPA-008
	it("evaluates exactly the stylesheet's narrow breakpoint query", () => {
		const mm = installMatchMedia(false);
		renderHook(() => useNarrow());
		expect(NARROW_QUERY).toBe("(max-width: 900px)");
		expect(new Set(mm.queries)).toEqual(new Set([NARROW_QUERY]));
	});

	// @spec APP-SPA-008
	it("returns the current match synchronously on the first render", () => {
		installMatchMedia(true);
		const { result } = renderHook(() => useNarrow());
		expect(result.current).toBe(true);
	});

	// @spec APP-SPA-008
	it("updates when the media query emits change, in both directions", () => {
		const mm = installMatchMedia(false);
		const { result } = renderHook(() => useNarrow());
		expect(result.current).toBe(false);
		act(() => mm.set(true));
		expect(result.current).toBe(true);
		act(() => mm.set(false));
		expect(result.current).toBe(false);
	});

	// @spec APP-SPA-008
	it("unsubscribes on unmount", () => {
		const mm = installMatchMedia(false);
		const { unmount } = renderHook(() => useNarrow());
		expect(mm.listenerCount()).toBe(1);
		unmount();
		expect(mm.listenerCount()).toBe(0);
	});

	// @spec APP-SPA-008
	it("returns false without throwing when matchMedia is unavailable", () => {
		const restore = removeMatchMedia();
		try {
			const { result } = renderHook(() => useNarrow());
			expect(result.current).toBe(false);
		} finally {
			restore();
		}
	});
});
