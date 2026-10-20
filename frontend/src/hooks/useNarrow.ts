import { useEffect, useState } from "react";

/**
 * The one narrow breakpoint. The stylesheet's `@media` block and this query
 * must stay identical so layout and behaviour switch together.
 */
export const NARROW_QUERY = "(max-width: 900px)";

function narrowQuery(): MediaQueryList | null {
	if (
		typeof window === "undefined" ||
		typeof window.matchMedia !== "function"
	) {
		return null;
	}
	return window.matchMedia(NARROW_QUERY);
}

// @spec APP-SPA-007, APP-SPA-008
export function useNarrow(): boolean {
	const [narrow, setNarrow] = useState(() => narrowQuery()?.matches ?? false);

	useEffect(() => {
		const mql = narrowQuery();
		if (!mql) return;
		const onChange = (e: MediaQueryListEvent) => setNarrow(e.matches);
		setNarrow(mql.matches);
		mql.addEventListener("change", onChange);
		return () => mql.removeEventListener("change", onChange);
	}, []);

	return narrow;
}
