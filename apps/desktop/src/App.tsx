import { useEffect, useState } from "react";
import { Tooltip } from "radix-ui";
import { Toaster } from "./components/ui/toast";
import { on } from "./lib/ipc";
import { useApp } from "./state/app";
import { LockScreen } from "./screens/lock/LockScreen";
import { Onboarding } from "./screens/onboarding/Onboarding";
import { Main } from "./screens/main/Main";
import { toast } from "./components/ui/toast";
import { useVault } from "./state/vault";

const LOCK_REASON_KEY = "moco.lockReason";

export function App() {
  const phase = useApp((st) => st.phase);
  const error = useApp((st) => st.error);
  const boot = useApp((st) => st.boot);
  const [lockReason] = useState(() => {
    const r = sessionStorage.getItem(LOCK_REASON_KEY);
    sessionStorage.removeItem(LOCK_REASON_KEY);
    return r;
  });

  useEffect(() => {
    void boot();
  }, [boot]);

  useEffect(() => {
    const unsubs: Promise<() => void>[] = [
      // Locking wipes everything the webview knows: reload into a fresh JS heap.
      on("moco://locked", (reason) => {
        sessionStorage.setItem(LOCK_REASON_KEY, String(reason ?? ""));
        window.location.reload();
      }),
      on("moco://clipboard-cleared", () => toast("Área de transferência limpa")),
      on("moco://open-item", (id) => {
        const st = useVault.getState();
        st.setView({ type: "all" });
        st.setQuery("");
        st.select(String(id));
      }),
      on("moco://unlocked", () => {
        if (useApp.getState().phase === "locked") useApp.getState().setPhase("unlocked");
      }),
    ];
    return () => {
      unsubs.forEach((p) => p.then((u) => u()));
    };
  }, []);

  let screen: React.ReactNode = null;
  if (phase === "onboarding") screen = <Onboarding />;
  else if (phase === "locked") screen = <LockScreen reason={lockReason} />;
  else if (phase === "unlocked") screen = <Main />;
  else if (phase === "error")
    screen = (
      <div style={{ padding: 40, maxWidth: 560 }}>
        <h1 style={{ fontSize: 22, marginBottom: 12 }}>Não conseguimos abrir seu Mocó.</h1>
        <p style={{ color: "var(--ink-2)", lineHeight: 1.6 }}>{error?.message}</p>
        <p style={{ color: "var(--ink-3)", marginTop: 12, fontSize: 13 }}>
          Seus dados continuam cifrados no disco. Feche e abra o Mocó de novo; se persistir, fale com o suporte.
        </p>
      </div>
    );

  return (
    <Tooltip.Provider delayDuration={450} skipDelayDuration={200}>
      {screen}
      <Toaster />
    </Tooltip.Provider>
  );
}
