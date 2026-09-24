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

/** Pads the UI clear of the status and gesture bars, using the sizes Android reports. */
export async function applyInsets() {
  const { api } = await import("../lib/ipc");
  const r = await api.appInsets().catch(() => null);
  if (!r) return;
  const root = document.documentElement.style;
  root.setProperty("--safe-top", `${r[0]}px`);
  root.setProperty("--safe-bottom", `${r[1]}px`);
}
