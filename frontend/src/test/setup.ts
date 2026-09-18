import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

// happy-dom does not implement window.confirm; provide a default so tests can
// vi.spyOn(window, "confirm") the same way they did under jsdom.
if (typeof window.confirm !== "function") {
	Object.defineProperty(window, "confirm", {
		value: vi.fn(() => true),
		writable: true,
		configurable: true,
	});
}

afterEach(() => {
	cleanup();
});
