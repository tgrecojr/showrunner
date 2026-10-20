import { vi } from "vitest";

/**
 * Replace window.matchMedia with a controllable fake so components that read
 * the narrow breakpoint can be rendered as narrow or wide and flipped mid-test.
 * happy-dom's own matchMedia always reports `matches: false` and never fires.
 */
export function installMatchMedia(initial: boolean) {
	const listeners = new Set<(e: MediaQueryListEvent) => void>();
	let matches = initial;
	const queries: string[] = [];
	const mql = {
		get matches() {
			return matches;
		},
		media: "",
		onchange: null,
		addEventListener: (_type: string, cb: (e: MediaQueryListEvent) => void) => {
			listeners.add(cb);
		},
		removeEventListener: (
			_type: string,
			cb: (e: MediaQueryListEvent) => void,
		) => {
			listeners.delete(cb);
		},
		addListener: () => {},
		removeListener: () => {},
		dispatchEvent: () => true,
	} as unknown as MediaQueryList;
	const spy = vi
		.spyOn(window, "matchMedia")
		.mockImplementation((query: string) => {
			queries.push(query);
			return mql;
		});
	return {
		spy,
		queries,
		listenerCount: () => listeners.size,
		set(value: boolean) {
			matches = value;
			for (const cb of listeners) {
				cb({ matches: value } as MediaQueryListEvent);
			}
		},
	};
}

/** Make window.matchMedia disappear for one test; returns a restore function. */
export function removeMatchMedia(): () => void {
	const original = window.matchMedia;
	Object.defineProperty(window, "matchMedia", {
		value: undefined,
		configurable: true,
		writable: true,
	});
	return () => {
		Object.defineProperty(window, "matchMedia", {
			value: original,
			configurable: true,
			writable: true,
		});
	};
}
