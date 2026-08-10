import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
	plugins: [react()],
	test: {
		globals: false,
		environment: "jsdom",
		setupFiles: ["./src/test/setup.ts"],
		css: false,
		coverage: {
			provider: "v8",
			reporter: ["text", "lcov", "html"],
			include: ["src/**/*.{ts,tsx}"],
			exclude: [
				"src/main.tsx",
				"src/types/**",
				"src/**/*.d.ts",
				"src/test/**",
				"src/**/*.test.{ts,tsx}",
			],
			thresholds: {
				lines: 85,
				statements: 85,
				functions: 85,
				branches: 80,
			},
		},
	},
});
