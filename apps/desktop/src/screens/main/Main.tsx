import { useEffect, useRef } from "react";
import { Kbd } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { api, errorMessage } from "../../lib/ipc";
import { selectVisible, useVault } from "../../state/vault";
import { ItemEditor } from "./editor/ItemEditor";
import { ItemDetail } from "./ItemDetail";
import { ListPane } from "./ListPane";
import { newItemBus } from "./NewItem";
import { Sidebar } from "./Sidebar";
import { GeneratorScreen } from "../generator/GeneratorScreen";
import { SecurityScreen } from "../security/SecurityScreen";
import { SettingsScreen } from "../settings/SettingsScreen";
import s from "./Main.module.css";

function inTextField(el: Element | null) {
  return !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || (el as HTMLElement).isContentEditable);
}

export async function lockApp() {
  try {
    await api.lock();
  } catch (e) {
    toast(errorMessage(e), { tone: "danger" });
  }
}

export function Main() {
  const loaded = useVault((st) => st.loaded);
  const load = useVault((st) => st.load);
  const screen = useVault((st) => st.screen);
  const selectedId = useVault((st) => st.selectedId);
  const editing = useVault((st) => st.editing);
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!loaded) load().catch((e) => toast(errorMessage(e), { tone: "danger" }));
  }, [loaded, load]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const st = useVault.getState();
      const key = e.key.toLowerCase();
      const typing = inTextField(document.activeElement);
      if (e.ctrlKey && !e.shiftKey && (key === "f" || key === "k")) {
        e.preventDefault();
        st.setScreen("vault");
        window.setTimeout(() => {
          searchRef.current?.focus();
          searchRef.current?.select();
        }, 0);
      } else if (e.ctrlKey && key === "n") {
        e.preventDefault();
        newItemBus.open();
      } else if (e.ctrlKey && key === "l") {
        e.preventDefault();
        void lockApp();
      } else if (e.ctrlKey && key === "e" && st.selectedId && st.screen === "vault" && !st.editing) {
        e.preventDefault();
        st.edit({ mode: "edit", id: st.selectedId });
      } else if (e.ctrlKey && key === "," ) {
        e.preventDefault();
        st.setScreen("settings");
      } else if (e.ctrlKey && key === "g") {
        e.preventDefault();
        st.setScreen("generator");
      } else if (e.ctrlKey && key === "c" && !typing && !window.getSelection()?.toString() && st.selectedId && !st.editing) {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent(e.shiftKey ? "moco:copy-username" : "moco:copy-primary"));
      } else if (e.key === "Delete" && !typing && st.selectedId && !st.editing && st.screen === "vault") {
        e.preventDefault();
        const id = st.selectedId;
        const item = st.items.find((i) => i.id === id);
        if (!item || item.trashedAt != null) return;
        const visible = selectVisible(st);
        const idx = visible.findIndex((v) => v.id === id);
        const next = visible[idx + 1] ?? visible[idx - 1];
        api
          .trash(id)
          .then(() => st.refresh())
          .then(() => {
            st.select(next?.id ?? null);
            toast(`“${item.title}” foi para a lixeira`, {
              action: { label: "Desfazer", run: () => void api.restore(id).then(() => useVault.getState().refresh()) },
            });
          })
          .catch((err) => toast(errorMessage(err), { tone: "danger" }));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (!loaded) return <div className={s.loading} />;

  return (
    <div className={s.root}>
      <Sidebar onLock={() => void lockApp()} />
      {screen === "vault" ? (
        <>
          <ListPane searchRef={searchRef} />
          <main className={s.detail}>
            {editing ? (
              <ItemEditor key={editing.mode === "edit" ? editing.id : `new-${editing.kind}`} editing={editing} />
            ) : selectedId ? (
              <ItemDetail id={selectedId} />
            ) : (
              <EmptyDetail />
            )}
          </main>
        </>
      ) : (
        <main className={s.wide}>
          {screen === "generator" && <GeneratorScreen />}
          {screen === "security" && <SecurityScreen />}
          {screen === "settings" && <SettingsScreen />}
        </main>
      )}
    </div>
  );
}

function EmptyDetail() {
  return (
    <div className={s.empty}>
      <svg className={s.emptyTiles} viewBox="0 0 120 120" aria-hidden="true">
        <rect x="0.5" y="0.5" width="39" height="39" rx="6" />
        <rect x="40.5" y="0.5" width="39" height="39" rx="6" />
        <rect x="80.5" y="0.5" width="39" height="39" rx="6" />
        <rect x="0.5" y="40.5" width="39" height="39" rx="6" />
        <rect x="40.5" y="40.5" width="39" height="39" rx="6" className={s.center} />
        <rect x="80.5" y="40.5" width="39" height="39" rx="6" />
        <rect x="0.5" y="80.5" width="39" height="39" rx="6" />
        <rect x="40.5" y="80.5" width="39" height="39" rx="6" />
        <rect x="80.5" y="80.5" width="39" height="39" rx="6" />
        <path d="M0.5 25 A24.5 24.5 0 0 1 25 0.5" className={s.arc} />
        <path d="M119.5 95 A24.5 24.5 0 0 1 95 119.5" className={s.arc} />
        <path d="M52 79.5 V63 a8 8 0 0 1 16 0 V79.5" className={s.door} />
      </svg>
      <p className={s.emptyTitle}>Escolha um item para ver os detalhes.</p>
      <ul className={s.shortcuts}>
        <li>
          <Kbd keys="Ctrl+F" /> buscar
        </li>
        <li>
          <Kbd keys="Ctrl+N" /> novo item
        </li>
        <li>
          <Kbd keys="Enter" /> na busca copia a senha
        </li>
        <li>
          <Kbd keys="Ctrl+L" /> trancar
        </li>
      </ul>
    </div>
  );
}
