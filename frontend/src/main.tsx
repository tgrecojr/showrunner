import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./index.css";

// The document head (index.html) and the manifest (public/manifest.webmanifest)
// are static files; this entry point anchors their specs.
// @spec APP-SPA-016, APP-SPA-017

createRoot(document.getElementById("root")!).render(
	<StrictMode>
		<App />
	</StrictMode>,
);
