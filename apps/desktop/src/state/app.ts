import { create } from "zustand";
import { api, errorMessage } from "../lib/ipc";
import type { AppError, AppInfo, Settings } from "../lib/types";

export type Phase = "booting" | "onboarding" | "locked" | "unlocked" | "error";

interface AppStore {
  phase: Phase;
  info: AppInfo | null;
  settings: Settings | null;
  error: AppError | null;
  /** Effective theme after resolving "system". */
  theme: "light" | "dark";
  boot: () => Promise<void>;
  setPhase: (p: Phase) => void;
  updateSettings: (patch: Partial<Settings>) => Promise<void>;
}

const systemDark = () => window.matchMedia("(prefers-color-scheme: dark)").matches;

function resolveTheme(pref: Settings["theme"] | undefined): "light" | "dark" {
  if (pref === "light" || pref === "dark") return pref;
  return systemDark() ? "dark" : "light";
}

export function applyTheme(theme: "light" | "dark") {
  document.documentElement.dataset.theme = theme;
}

export const useApp = create<AppStore>((set, get) => ({
  phase: "booting",
  info: null,
  settings: null,
  error: null,
  theme: resolveTheme("system"),
  boot: async () => {
    try {
      const info = await api.appInfo();
      const theme = resolveTheme(info.settings.theme);
      applyTheme(theme);
      if (info.storageError) {
        set({ info, settings: info.settings, theme, phase: "error", error: info.storageError });
        return;
      }
      set({
        info,
        settings: info.settings,
        theme,
        phase: !info.initialized ? "onboarding" : info.unlocked ? "unlocked" : "locked",
      });
    } catch (e) {
      set({ phase: "error", error: { code: "boot", message: errorMessage(e) } });
    }
  },
  setPhase: (phase) => set({ phase }),
  updateSettings: async (patch) => {
    const current = get().settings;
    if (!current) return;
    const next = { ...current, ...patch };
    set({ settings: next });
    if (patch.theme) {
      const theme = resolveTheme(patch.theme);
      applyTheme(theme);
      set({ theme });
    }
    try {
      const saved = await api.updateSettings(next);
      set({ settings: saved });
    } catch (e) {
      set({ settings: current });
      throw e;
    }
  },
}));

// Follow the OS theme live when the preference is "system".
window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  const { settings } = useApp.getState();
  if (!settings || settings.theme === "system") {
    const theme = resolveTheme("system");
    applyTheme(theme);
    useApp.setState({ theme });
  }
});
