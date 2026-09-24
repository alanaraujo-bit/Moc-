import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/cascadia-mono/400.css";
import "./styles/base.css";
import { App } from "./App";
import { initBridge } from "./lib/ipc";

// A desktop app, not a web page: no browser context menu or reload shortcuts outside
// text fields, and no dropping files onto the window to navigate away.
if (!import.meta.env.DEV) {
  window.addEventListener("contextmenu", (e) => {
    const el = e.target as HTMLElement;
    if (!el.closest("input, textarea, .selectable")) e.preventDefault();
  });
}
window.addEventListener("keydown", (e) => {
  if (!import.meta.env.DEV && (e.key === "F5" || (e.ctrlKey && e.key.toLowerCase() === "r" && !e.shiftKey))) e.preventDefault();
  if (e.ctrlKey && (e.key.toLowerCase() === "p" || e.key.toLowerCase() === "u")) e.preventDefault();
});
window.addEventListener("dragover", (e) => e.preventDefault());
window.addEventListener("drop", (e) => e.preventDefault());

initBridge().then(() => {
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
});
