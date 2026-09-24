import { MobileMain, useLoadVault } from "./MobileMain";

/** Unlocked Mocó on a phone. */
export function MobileUnlocked() {
  const loaded = useLoadVault();
  if (!loaded) return null;
  return <MobileMain />;
}
