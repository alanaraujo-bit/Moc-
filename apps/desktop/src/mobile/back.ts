import { useEffect, useRef } from "react";

/**
 * Android's back gesture navigates the WebView history (and closes the app when there is
 * none). Each open layer — a screen deeper in the stack, a bottom sheet — owns history
 * entries equal to its depth; back pops the topmost layer.
 */
interface Layer {
  pushed: number;
  pop: () => void;
}

const layers: Layer[] = [];
let ignore = 0;

window.addEventListener("popstate", () => {
  if (ignore > 0) {
    ignore -= 1;
    return;
  }
  for (let i = layers.length - 1; i >= 0; i--) {
    if (layers[i].pushed > 0) {
      layers[i].pushed -= 1;
      layers[i].pop();
      return;
    }
  }
});

/** Drops `n` of our entries without triggering a pop. */
function unwind(n: number) {
  if (n <= 0) return;
  ignore += 1;
  history.go(-n);
}

/** Keeps `depth` history entries for this layer; returns a "go back" for on-screen arrows. */
export function useBackStack(depth: number, pop: () => void): () => void {
  const layer = useRef<Layer | null>(null);
  if (!layer.current) layer.current = { pushed: 0, pop };
  layer.current.pop = pop;

  useEffect(() => {
    const l = layer.current!;
    layers.push(l);
    return () => {
      const i = layers.indexOf(l);
      if (i >= 0) layers.splice(i, 1);
      unwind(l.pushed);
      l.pushed = 0;
    };
  }, []);

  useEffect(() => {
    const l = layer.current!;
    while (l.pushed < depth) {
      history.pushState({ moco: true }, "");
      l.pushed += 1;
    }
    if (l.pushed > depth) {
      // The app moved up on its own (saved an edit, deleted an item): drop the extra entries.
      unwind(l.pushed - depth);
      l.pushed = depth;
    }
  }, [depth]);

  return () => history.back();
}

/**
 * Android back button: walk up our stack when something is open; at the top, send the app
 * to the background (Tauri calls this instead of its default once a listener exists).
 */
export async function installAndroidBack() {
  const { onBackButtonPress } = await import("@tauri-apps/api/app");
  const { api } = await import("../lib/ipc");
  await onBackButtonPress(() => {
    if (layers.some((l) => l.pushed > 0)) history.back();
    else void api.appBackground().catch(() => {});
  });
}

/**
 * Keeps the UI clear of the status bar, the navigation bar (gestures or three buttons), the
 * camera cutout and the keyboard, using the sizes Android reports. Edge to edge, the
 * WebView reports none of the bottom ones itself and may not shrink for the keyboard.
 */
let baseHeight = window.innerHeight;

async function measure() {
  const { api } = await import("../lib/ipc");
  const r = await api.appInsets().catch(() => null);
  if (!r) return;
  // If the WebView already shrank for the keyboard, only pad what it didn't cover.
  if (r.keyboard === 0) baseHeight = window.innerHeight;
  const shrunk = Math.max(0, baseHeight - window.innerHeight);
  const keyboard = Math.max(0, r.keyboard - shrunk);
  const root = document.documentElement.style;
  root.setProperty("--safe-top", `${r.top}px`);
  root.setProperty("--safe-left", `${r.left}px`);
  root.setProperty("--safe-right", `${r.right}px`);
  // With the keyboard up, the navigation bar is under it: pad for whichever is taller.
  root.setProperty("--safe-bottom", `${keyboard > 0 ? keyboard : r.bottom}px`);
  document.documentElement.dataset.keyboard = keyboard > 0 ? "open" : "closed";
  if (keyboard > 0) {
    const el = document.activeElement as HTMLElement | null;
    if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) el.scrollIntoView({ block: "center" });
  }
}

/** The keyboard animates in and out; look again as it settles. */
let polling = 0;
function measureSoon() {
  for (const ms of [0, 120, 350, 700]) window.setTimeout(() => void measure(), ms);
  // Closing the keyboard with Back fires no event in the page: keep checking while it's up.
  const started = Date.now();
  if (!polling) {
    polling = window.setInterval(() => {
      if (document.documentElement.dataset.keyboard !== "open" && Date.now() - started > 1000) {
        window.clearInterval(polling);
        polling = 0;
        return;
      }
      void measure();
    }, 300);
  }
}

export async function applyInsets() {
  await measure();
  window.addEventListener("resize", measureSoon);
  window.addEventListener("orientationchange", measureSoon);
  document.addEventListener("focusin", measureSoon);
  document.addEventListener("focusout", measureSoon);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") measureSoon();
  });
}

/**
 * Leaving and coming back to the app, on every screen: background time counts toward the
 * auto-lock, and a clipboard clear that came due meanwhile runs on return (autolock.rs,
 * commands::app_visibility).
 */
export async function trackVisibility() {
  const { api } = await import("../lib/ipc");
  document.addEventListener("visibilitychange", () => {
    void api.appVisibility(document.visibilityState === "visible").catch(() => {});
  });
}
