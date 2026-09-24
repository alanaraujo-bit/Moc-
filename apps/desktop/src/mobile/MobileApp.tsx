import { useEffect } from "react";
import { api } from "../lib/ipc";
import { MobileMain, useLoadVault } from "./MobileMain";

/** Unlocked Mocó on a phone. Leaving the app starts the lock clock (see autolock.rs). */
export function MobileUnlocked() {
  const loaded = useLoadVault();

  useEffect(() => {
    const onVisibility = () => void api.appVisibility(document.visibilityState === "visible").catch(() => {});
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, []);

  if (!loaded) return null;
  return <MobileMain />;
}
