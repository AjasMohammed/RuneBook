// Bundle the typography locally (offline-safe for a desktop overlay): Hanken
// Grotesk is the reading face, Space Grotesk the display face for headlines.
// Without these imports the CSS font-family silently fell back to system-ui.
import "@fontsource/hanken-grotesk/400.css";
import "@fontsource/hanken-grotesk/500.css";
import "@fontsource/hanken-grotesk/600.css";
import "@fontsource/hanken-grotesk/700.css";
import "@fontsource/space-grotesk/500.css";
import "@fontsource/space-grotesk/700.css";

import "./app.css";
import App from "./App.svelte";

const app = new App({ target: document.getElementById("app") });

export default app;
