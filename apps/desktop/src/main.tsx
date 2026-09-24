import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/cascadia-mono/400.css";
import "./styles/base.css";
import { App } from "./App";
import { api, on } from "./lib/ipc";
import { applyTheme } from "./state/app";
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
// Files dropped on the window are handled natively (see Attachments); never navigate.
window.addEventListener("dragover", (e) => e.preventDefault());
window.addEventListener("drop", (e) => e.preventDefault());

const isQuick = new URLSearchParams(window.location.search).get("w") === "quick";

initBridge().then(async () => {
  if (isQuick) {
    const theme = (pref: string) =>
      pref === "light" || pref === "dark" ? pref : window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    try {
      applyTheme(theme((await api.settings()).theme));
    } catch {
      applyTheme(theme("system"));
    }
    void on("moco://settings", (st) => applyTheme(theme((st as { theme: string }).theme)));
    void on("moco://locked", () => window.location.reload());
  }
  const Root = isQuick ? (await import("./screens/quick/QuickAccess")).QuickAccess : App;
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <Root />
    </StrictMode>,
  );
});
