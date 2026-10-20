import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * The unit-test environment lays out nothing and applies no stylesheet, so the
 * narrow-width rules are pinned here as text: the single breakpoint, the rules
 * that must live inside it, the document head, and the manifest.
 */

const root = resolve(__dirname, "..");
const css = readFileSync(join(root, "src/index.css"), "utf8");
const html = readFileSync(join(root, "index.html"), "utf8");
const hookSource = readFileSync(join(root, "src/hooks/useNarrow.ts"), "utf8");

const NARROW = "(max-width: 900px)";

interface Rule {
	selector: string;
	body: string;
	/** Media preludes enclosing this rule, outermost first. */
	media: string[];
}

function stripComments(text: string): string {
	return text.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Split a stylesheet (or block body) into top-level `prelude { body }` pairs. */
function blocks(text: string): { prelude: string; body: string }[] {
	const out: { prelude: string; body: string }[] = [];
	let depth = 0;
	let start = 0;
	let preludeStart = 0;
	for (let i = 0; i < text.length; i++) {
		const ch = text[i];
		if (ch === "{") {
			if (depth === 0) {
				start = i + 1;
			}
			depth++;
		} else if (ch === "}") {
			depth--;
			if (depth === 0) {
				out.push({
					prelude: text.slice(preludeStart, start - 1).trim(),
					body: text.slice(start, i).trim(),
				});
				preludeStart = i + 1;
			}
		}
	}
	return out;
}

function flatten(text: string, media: string[] = []): Rule[] {
	const rules: Rule[] = [];
	for (const b of blocks(text)) {
		if (b.prelude.startsWith("@media")) {
			rules.push(...flatten(b.body, [...media, b.prelude]));
		} else if (!b.prelude.startsWith("@")) {
			rules.push({ selector: b.prelude, body: b.body, media });
		}
	}
	return rules;
}

const rules = flatten(stripComments(css));
const narrowRules = rules.filter((r) =>
	r.media.some((m) => m.includes("max-width")),
);
const baseRules = rules.filter((r) => r.media.length === 0);

function decl(rule: Rule, prop: string): string | undefined {
	const m = rule.body.match(
		new RegExp(`(?:^|;)\\s*${prop.replace(/-/g, "\\-")}\\s*:\\s*([^;]+)`),
	);
	return m?.[1]?.trim();
}

/** All declarations of `prop` on rules whose selector list contains `selector` exactly. */
function narrowDecl(selector: string, prop: string): string | undefined {
	const matches = narrowRules.filter((r) =>
		r.selector.split(",").some((s) => s.trim() === selector),
	);
	for (const r of matches) {
		const v = decl(r, prop);
		if (v !== undefined) return v;
	}
	return undefined;
}

function baseDecl(selector: string, prop: string): string | undefined {
	for (const r of baseRules.filter((r) =>
		r.selector.split(",").some((s) => s.trim() === selector),
	)) {
		const v = decl(r, prop);
		if (v !== undefined) return v;
	}
	return undefined;
}

function sourceFiles(dir: string): string[] {
	const out: string[] = [];
	for (const name of readdirSync(dir)) {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) out.push(...sourceFiles(p));
		else if (/\.(ts|tsx)$/.test(name) && !/\.test\.tsx?$/.test(name))
			out.push(p);
	}
	return out;
}

describe("stylesheet: the narrow breakpoint", () => {
	// @spec APP-SPA-007
	it("declares exactly one max-width media query, at 900px, matching useNarrow", () => {
		const widthQueries = new Set(
			rules
				.flatMap((r) => r.media)
				.filter((m) => m.includes("max-width") || m.includes("min-width")),
		);
		expect([...widthQueries]).toEqual([`@media ${NARROW}`]);
		expect(hookSource).toContain(`"${NARROW}"`);
	});

	// @spec APP-SPA-007
	it("keeps every width-conditioned rule inside that one block", () => {
		// Any other prelude mentioning a width is a second breakpoint.
		const preludes = stripComments(css).match(/@media[^{]+/g) ?? [];
		for (const p of preludes) {
			if (/width/.test(p)) expect(p.trim()).toBe(`@media ${NARROW}`);
		}
	});
});

describe("stylesheet: top bar", () => {
	// @spec APP-SPA-009
	it("stays sticky and, when narrow, wraps into a brand row over a scrolling, fading link strip", () => {
		expect(baseDecl(".topnav", "position")).toBe("sticky");
		expect(narrowDecl(".topnav", "flex-wrap")).toBe("wrap");
		expect(narrowDecl(".topnav-brand-row", "flex")).toMatch(/100%/);
		expect(narrowDecl(".topnav-links", "flex")).toMatch(/100%/);
		expect(narrowDecl(".topnav-links", "overflow-x")).toBe("auto");
		expect(narrowDecl(".topnav-links", "min-width")).toBe("0");
		expect(narrowDecl(".topnav-links", "mask-image")).toMatch(
			/linear-gradient/,
		);
		expect(narrowDecl(".topnav-links", "-webkit-mask-image")).toMatch(
			/linear-gradient/,
		);
		expect(narrowDecl(".topnav-links::after", "content")).toBeDefined();
	});

	// @spec APP-SPA-013
	it("gives links and controls 44px targets when narrow without changing font size", () => {
		expect(narrowDecl(".topnav-links a", "min-height")).toBe("44px");
		for (const sel of [".btn", ".ep-through-btn", ".season-toggle"]) {
			expect(narrowDecl(sel, "min-height"), sel).toBe("44px");
			expect(narrowDecl(sel, "min-width"), sel).toBe("44px");
			expect(narrowDecl(sel, "font-size"), sel).toBeUndefined();
		}
		expect(narrowDecl(".topnav-links a", "font-size")).toBeUndefined();
	});

	// @spec APP-SPA-014
	it("uses dvh with a vh fallback and safe-area insets", () => {
		const layout = baseRules.find((r) => r.selector === ".layout");
		expect(layout).toBeDefined();
		const body = layout?.body ?? "";
		const vh = body.indexOf("min-height: 100vh");
		const dvh = body.indexOf("min-height: 100dvh");
		expect(vh).toBeGreaterThanOrEqual(0);
		expect(dvh).toBeGreaterThan(vh);

		for (const sel of [".topnav-brand-row", ".main", ".topnav-links"]) {
			expect(narrowDecl(sel, "padding-left"), sel).toBe(
				"max(1rem, env(safe-area-inset-left))",
			);
			expect(narrowDecl(sel, "padding-right"), sel).toBe(
				"max(1rem, env(safe-area-inset-right))",
			);
		}
		expect(narrowDecl(".topnav", "padding-top")).toMatch(
			/env\(safe-area-inset-top\)/,
		);
		expect(narrowDecl(".main", "padding-bottom")).toMatch(
			/env\(safe-area-inset-bottom\)/,
		);
	});

	// @spec APP-SPA-015
	it("guards hover styles, hides the strip scrollbar only on touch, and keeps focus rings", () => {
		const hoverRules = rules.filter((r) => r.selector.includes(":hover"));
		expect(hoverRules.length).toBeGreaterThan(0);
		for (const r of hoverRules) {
			expect(
				r.media.some((m) => m.includes("(hover: hover)")),
				r.selector,
			).toBe(true);
			for (const s of r.selector.split(",")) {
				expect(s, r.selector).toContain(":hover");
			}
		}
		const scrollbarRules = rules.filter(
			(r) =>
				r.body.includes("scrollbar-width") ||
				r.selector.includes("::-webkit-scrollbar"),
		);
		expect(scrollbarRules.length).toBeGreaterThan(0);
		for (const r of scrollbarRules) {
			expect(
				r.media.some((m) => m.includes("(hover: none)")),
				r.selector,
			).toBe(true);
		}
		for (const r of rules.filter((r) =>
			/topnav-links a|nav a/.test(r.selector),
		)) {
			expect(decl(r, "outline") ?? "", r.selector).not.toMatch(/^(none|0)$/);
		}
	});
});

describe("stylesheet: shared patterns when narrow", () => {
	// @spec APP-SPA-011
	it("floats a small poster and clears the overview and actions beneath it", () => {
		expect(narrowDecl(".show-header .poster-lg", "width")).toBe("100px");
		expect(narrowDecl(".show-header .poster-lg", "height")).toBe("150px");
		expect(narrowDecl(".show-header .poster-lg", "float")).toBe("left");
		expect(narrowDecl(".show-header .detail-overview", "clear")).toBe("both");
		expect(narrowDecl(".show-header .action-row", "clear")).toBe("both");
		expect(narrowDecl(".show-header .action-row .btn", "flex")).toMatch(/100%/);
	});

	// @spec APP-SPA-012
	it("collapses the card grid to one column and leaves the card's poster alone", () => {
		expect(narrowDecl(".results-grid", "grid-template-columns")).toBe("1fr");
		expect(narrowDecl(".result-card", "flex-direction")).toBeUndefined();
		expect(narrowDecl(".poster", "width")).toBeUndefined();
		expect(baseDecl(".poster", "width")).toBe("92px");
	});

	// @spec AIRING-UPNEXT-013
	it("wraps the Up Next row so the action button spans the full width", () => {
		expect(narrowDecl(".upnext-row", "flex-wrap")).toBe("wrap");
		expect(narrowDecl(".upnext-action", "flex")).toMatch(/100%/);
		expect(narrowDecl(".upnext-action .btn", "width")).toBe("100%");
	});

	// @spec SHOWS-UI-016
	it("drops the air date under the name and stretches the through button across both lines", () => {
		expect(narrowDecl(".episode-label", "grid-template-columns")).toBe(
			"auto auto 1fr",
		);
		expect(narrowDecl(".episode-label .ep-date", "grid-column")).toBe("3");
		expect(narrowDecl(".episode-row .ep-through-btn", "align-self")).toBe(
			"stretch",
		);
	});

	// @spec SHOWS-UI-017
	it("truncates episode names when wide and wraps them when narrow", () => {
		expect(baseDecl(".ep-name", "text-overflow")).toBe("ellipsis");
		expect(baseDecl(".ep-name", "white-space")).toBe("nowrap");
		expect(narrowDecl(".ep-name", "white-space")).toBe("normal");
	});
});

describe("document head and manifest", () => {
	// @spec APP-SPA-016
	it("carries the viewport, theme-color, manifest, and touch-icon tags", () => {
		expect(html).toMatch(
			/<meta\s+name="viewport"\s+content="width=device-width, initial-scale=1, viewport-fit=cover"\s*\/?>/,
		);
		expect(html).toMatch(
			/<meta\s+name="theme-color"\s+content="#1a1a1a"\s*\/?>/,
		);
		expect(html).toMatch(
			/<link\s+rel="manifest"\s+href="\/manifest\.webmanifest"\s*\/?>/,
		);
		expect(html).toMatch(
			/<link\s+rel="apple-touch-icon"\s+(?:sizes="180x180"\s+)?href="\/apple-touch-icon\.png"\s*\/?>/,
		);
		expect(existsSync(join(root, "public/apple-touch-icon.png"))).toBe(true);
	});

	// @spec APP-SPA-017
	it("declares a standalone manifest with icons and registers no service worker", () => {
		const manifest = JSON.parse(
			readFileSync(join(root, "public/manifest.webmanifest"), "utf8"),
		) as {
			name: string;
			display: string;
			start_url: string;
			theme_color: string;
			background_color: string;
			icons: { src: string; sizes: string; type: string }[];
		};
		expect(manifest.name).toBe("Showrunner");
		expect(manifest.display).toBe("standalone");
		expect(manifest.start_url).toBe("/");
		expect(manifest.theme_color).toBe("#1a1a1a");
		expect(manifest.background_color).toBe("#fafafa");
		const sizes = manifest.icons.map((i) => i.sizes).sort();
		expect(sizes).toEqual(["192x192", "512x512"]);
		for (const icon of manifest.icons) {
			expect(icon.type).toBe("image/png");
			expect(
				existsSync(join(root, "public", icon.src.replace(/^\//, ""))),
			).toBe(true);
		}
		for (const file of sourceFiles(join(root, "src"))) {
			expect(readFileSync(file, "utf8"), file).not.toMatch(/serviceWorker/);
		}
		expect(html).not.toMatch(/serviceWorker/);
	});
});
