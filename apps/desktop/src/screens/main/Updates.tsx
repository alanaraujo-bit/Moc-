import { useEffect, useState } from "react";
import { ArrowCircleUp, ShieldWarning, Sparkle } from "@phosphor-icons/react";
import { create } from "zustand";
import { Dialog } from "../../components/ui/overlays";
import { Button } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { releaseOf } from "../../lib/changelog";
import { api, errorMessage, on } from "../../lib/ipc";
import type { UpdateInfo } from "../../lib/types";
import { useApp } from "../../state/app";
import s from "./Updates.module.css";

interface UpdateStore {
  info: UpdateInfo | null;
  checking: boolean;
  installing: boolean;
  progress: number | null;
  check: (manual?: boolean) => Promise<void>;
  install: () => Promise<void>;
}

export const useUpdates = create<UpdateStore>((set, get) => ({
  info: null,
  checking: false,
  installing: false,
  progress: null,
  check: async (manual = false) => {
    if (get().checking || get().installing) return;
    set({ checking: true });
    try {
      const info = await api.updateCheck();
      set({ info });
      if (manual && !info) toast("Você já está na versão mais recente.", { tone: "success" });
    } catch (e) {
      if (manual) toast(errorMessage(e), { tone: "danger" });
    } finally {
      set({ checking: false });
    }
  },
  install: async () => {
    set({ installing: true, progress: 0 });
    try {
      await api.updateInstall();
    } catch (e) {
      set({ installing: false, progress: null });
      toast(errorMessage(e), { tone: "danger", duration: 7000 });
    }
  },
}));

void on("moco://update-progress", (p) => {
  const { downloaded, total } = p as { downloaded: number; total: number | null };
  useUpdates.setState({ progress: total ? downloaded / total : null });
});

const SIX_HOURS = 6 * 3600 * 1000;

/** Background checks + banner in the sidebar + critical-update dialog. */
export function UpdateBanner() {
  const settings = useApp((st) => st.settings);
  const { info, installing, progress, check, install } = useUpdates();
  const [dismissedCritical, setDismissedCritical] = useState(false);

  useEffect(() => {
    if (!settings?.autoUpdate) return;
    const first = window.setTimeout(() => void check(), 8000);
    const every = window.setInterval(() => void check(), SIX_HOURS);
    return () => {
      window.clearTimeout(first);
      window.clearInterval(every);
    };
  }, [settings?.autoUpdate, settings?.updateChannel, check]);

  if (!info) return null;
  return (
    <>
      <div className={s.banner} data-critical={info.critical || undefined}>
        {info.critical ? <ShieldWarning size={18} weight="fill" /> : <ArrowCircleUp size={18} />}
        <div className={s.text}>
          <strong>{info.critical ? "Atualização de segurança" : `Mocó ${info.version}`}</strong>
          <span>{installing ? (progress != null ? `Baixando… ${Math.round(progress * 100)}%` : "Instalando…") : "Pronta para instalar"}</span>
        </div>
        <Button size="sm" variant={info.critical ? "danger" : "primary"} loading={installing} onClick={install}>
          Atualizar
        </Button>
        {installing && progress != null && <span className={s.bar} style={{ transform: `scaleX(${progress})` }} />}
      </div>
      <Dialog
        open={info.critical && !dismissedCritical && !installing}
        onOpenChange={(o) => !o && setDismissedCritical(true)}
        title="Atualização de segurança disponível"
        description={`O Mocó ${info.version} corrige um problema de segurança. Recomendamos atualizar agora — leva menos de um minuto e o Mocó volta trancado.`}
        footer={
          <>
            <Button onClick={() => setDismissedCritical(true)}>Depois</Button>
            <Button variant="primary" onClick={install}>
              Atualizar agora
            </Button>
          </>
        }
      >
        {info.notes && <p className={s.notes}>{info.notes}</p>}
      </Dialog>
    </>
  );
}

/** Shown once after the app was updated to a version with release notes. */
export function WhatsNew() {
  const info = useApp((st) => st.info);
  const settings = useApp((st) => st.settings);
  const updateSettings = useApp((st) => st.updateSettings);
  const [open, setOpen] = useState(false);
  const version = info?.version ?? "";
  const release = releaseOf(version);

  useEffect(() => {
    if (!settings || !version || settings.lastSeenVersion === version) return;
    if (settings.lastSeenVersion && release) setOpen(true);
    else void updateSettings({ lastSeenVersion: version });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [version, settings?.lastSeenVersion]);

  if (!release) return null;
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (!o) void updateSettings({ lastSeenVersion: version });
      }}
      wide
      title={
        <span className={s.newTitle}>
          <Sparkle size={20} weight="fill" /> Novidades no Mocó {release.version}
        </span>
      }
      description={release.title}
      footer={
        <Button variant="primary" onClick={() => (setOpen(false), void updateSettings({ lastSeenVersion: version }))}>
          Legal
        </Button>
      }
    >
      <ul className={s.highlights}>
        {release.highlights.map((h) => (
          <li key={h.title}>
            <strong>{h.title}</strong>
            <span>{h.body}</span>
          </li>
        ))}
      </ul>
    </Dialog>
  );
}
