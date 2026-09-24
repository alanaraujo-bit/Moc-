import { lazy, memo, Suspense, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowLeft, GearSix, Key, LockSimple, MagnifyingGlass, Password, Plus, Star, X } from "@phosphor-icons/react";
import { MocoMark, Wordmark } from "../components/brand/Brand";
import { ItemTile, KindTile } from "../components/tile/Tile";
import { toast } from "../components/ui/toast";
import { errorMessage } from "../lib/ipc";
import { template, TEMPLATES } from "../lib/templates";
import type { ItemKind, ItemSummary } from "../lib/types";
import { selectVisible, useVault, type View } from "../state/vault";
import { ItemDetail } from "../screens/main/ItemDetail";
import { ItemEditor } from "../screens/main/editor/ItemEditor";
import { lockApp } from "../screens/main/Main";
import { useBackStack } from "./back";
import s from "./MobileMain.module.css";
const SettingsScreen = lazy(() => import("../screens/settings/SettingsScreen").then((m) => ({ default: m.SettingsScreen })));
const GeneratorScreen = lazy(() => import("../screens/generator/GeneratorScreen").then((m) => ({ default: m.GeneratorScreen })));

const FEATURED: ItemKind[] = ["login", "card", "note", "identity", "document", "wifi"];

/**
 * Mocó on a phone: one screen at a time — the list of tiles, then an item, then its editor
 * (or settings / generator). The Android back gesture walks back up the same stack.
 */
export function MobileMain() {
  const screen = useVault((st) => st.screen);
  const selectedId = useVault((st) => st.selectedId);
  const editing = useVault((st) => st.editing);

  const depth = editing ? (selectedId && editing.mode === "edit" ? 2 : 1) : selectedId || screen !== "vault" ? 1 : 0;
  const back = useBackStack(depth, () => {
    const st = useVault.getState();
    if (st.editing) st.edit(null);
    else if (st.selectedId) st.select(null);
    else if (st.screen !== "vault") st.setScreen("vault");
  });

  if (editing) {
    return (
      <Page title={editing.mode === "new" ? `Novo ${template(editing.kind).label.toLowerCase()}` : "Editar"} onBack={back} fill>
        <div className={s.pane}>
          <ItemEditor key={editing.mode === "edit" ? editing.id : `new-${editing.kind}`} editing={editing} />
        </div>
      </Page>
    );
  }
  if (selectedId) {
    return (
      <Page title="" onBack={back} fill>
        <div className={s.pane}>
          <ItemDetail id={selectedId} />
        </div>
      </Page>
    );
  }
  if (screen === "settings" || screen === "generator") {
    return (
      <Page title={screen === "settings" ? "Configurações" : "Gerador de senhas"} onBack={back}>
        <div className={s.wide}>
          <Suspense fallback={null}>{screen === "settings" ? <SettingsScreen /> : <GeneratorScreen />}</Suspense>
        </div>
      </Page>
    );
  }
  return <ListScreen />;
}

function Page({ title, onBack, fill, children }: { title: string; onBack: () => void; fill?: boolean; children: React.ReactNode }) {
  return (
    <div className={s.page}>
      <header className={s.bar}>
        <button className={s.iconBtn} onClick={onBack} aria-label="Voltar">
          <ArrowLeft size={22} />
        </button>
        <span className={s.barTitle}>{title}</span>
      </header>
      <div className={fill ? s.fill : s.scroll}>{children}</div>
    </div>
  );
}

function ListScreen() {
  const items = useVault((st) => st.items);
  const index = useVault((st) => st.index);
  const usage = useVault((st) => st.usage);
  const view = useVault((st) => st.view);
  const query = useVault((st) => st.query);
  const sort = useVault((st) => st.sort);
  const vaults = useVault((st) => st.vaults);
  const setView = useVault((st) => st.setView);
  const setQuery = useVault((st) => st.setQuery);
  const select = useVault((st) => st.select);
  const setScreen = useVault((st) => st.setScreen);
  const [adding, setAdding] = useState(false);

  const visible = useMemo(() => selectVisible({ items, index, usage, view, query, sort }), [items, index, usage, view, query, sort]);
  const chips: { label: string; view: View }[] = [
    { label: "Tudo", view: { type: "all" } },
    { label: "Favoritos", view: { type: "favorites" } },
    ...vaults.map((v) => ({ label: v.name, view: { type: "vault", id: v.id } as View })),
    { label: "Lixeira", view: { type: "trash" } },
  ];
  const same = (a: View, b: View) => a.type === b.type && JSON.stringify(a) === JSON.stringify(b);

  const scrollRef = useRef<HTMLDivElement>(null);
  const virt = useVirtualizer({ count: visible.length, getScrollElement: () => scrollRef.current, estimateSize: () => 64, overscan: 8 });

  return (
    <div className={s.page}>
      <header className={s.top}>
        <div className={s.brand}>
          <MocoMark size={26} />
          <Wordmark height={18} />
        </div>
        <button className={s.iconBtn} onClick={() => setScreen("generator")} aria-label="Gerador de senhas">
          <Password size={22} />
        </button>
        <button className={s.iconBtn} onClick={() => setScreen("settings")} aria-label="Configurações">
          <GearSix size={22} />
        </button>
        <button className={s.iconBtn} onClick={() => void lockApp()} aria-label="Trancar">
          <LockSimple size={22} />
        </button>
      </header>

      <div className={s.searchWrap}>
        <MagnifyingGlass size={18} className={s.searchIcon} />
        <input
          className={s.search}
          type="search"
          placeholder="Buscar no Mocó"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          enterKeyHint="search"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
        />
        {query && (
          <button className={s.clear} onClick={() => setQuery("")} aria-label="Limpar busca">
            <X size={16} />
          </button>
        )}
      </div>

      {!query && (
        <nav className={s.chips} aria-label="Filtros">
          {chips.map((c) => (
            <button key={c.label + JSON.stringify(c.view)} className={s.chip} data-on={same(c.view, view) || undefined} onClick={() => setView(c.view)}>
              {c.label}
            </button>
          ))}
        </nav>
      )}

      <div className={s.list} ref={scrollRef}>
        {visible.length === 0 ? (
          <Empty query={query} view={view} />
        ) : (
          <div style={{ height: virt.getTotalSize(), position: "relative" }}>
            {virt.getVirtualItems().map((v) => (
              <div key={visible[v.index].id} className={s.rowSlot} style={{ transform: `translateY(${v.start}px)` }}>
                <Row item={visible[v.index]} onOpen={select} />
              </div>
            ))}
          </div>
        )}
      </div>

      {view.type !== "trash" && (
        <button className={s.fab} onClick={() => setAdding(true)} aria-label="Novo item">
          <Plus size={24} weight="bold" />
        </button>
      )}
      {adding && <NewItemSheet onClose={() => setAdding(false)} />}
    </div>
  );
}

const Row = memo(function Row({ item, onOpen }: { item: ItemSummary; onOpen: (id: string) => void }) {
  return (
    <button className={s.row} onClick={() => onOpen(item.id)}>
      <ItemTile kind={item.kind} title={item.title} urls={item.urls} size={40} />
      <span className={s.rowText}>
        <span className={s.rowTitle}>{item.title}</span>
        <span className={s.rowSub}>{item.subtitle || template(item.kind).label}</span>
      </span>
      <span className={s.rowMeta}>
        {item.hasTotp && <Key size={14} aria-label="Tem 2FA" />}
        {item.favorite && <Star size={14} weight="fill" className={s.star} aria-label="Favorito" />}
      </span>
    </button>
  );
});

function Empty({ query, view }: { query: string; view: View }) {
  const text = query
    ? `Nada encontrado para “${query}”.`
    : view.type === "trash"
      ? "A lixeira está vazia."
      : view.type === "favorites"
        ? "Toque na estrela de um item para ele aparecer aqui."
        : "Nada aqui ainda. Toque em + para guardar o primeiro item.";
  return <p className={s.empty}>{text}</p>;
}

function NewItemSheet({ onClose }: { onClose: () => void }) {
  const vaults = useVault((st) => st.vaults);
  const view = useVault((st) => st.view);
  const edit = useVault((st) => st.edit);
  const writable = vaults.filter((v) => v.role !== "reader");
  const list = [...FEATURED.map((k) => TEMPLATES.find((t) => t.kind === k)!), ...TEMPLATES.filter((t) => !FEATURED.includes(t.kind))];

  // The sheet is a layer of its own: back closes it.
  useBackStack(1, onClose);

  const choose = (kind: ItemKind) => {
    const vaultId = view.type === "vault" && writable.some((v) => v.id === view.id) ? view.id : writable[0]?.id;
    if (!vaultId) {
      toast("Nenhum cofre aceita itens novos.", { tone: "danger" });
      return;
    }
    onClose();
    // Let the sheet's history entry unwind before the editor pushes its own.
    window.setTimeout(() => edit({ mode: "new", kind, vaultId }), 80);
  };

  return (
    <div className={s.scrim} onClick={() => history.back()}>
      <div className={s.sheet} role="dialog" aria-label="Novo item" onClick={(e) => e.stopPropagation()}>
        <div className={s.grip} aria-hidden="true" />
        <p className={s.sheetTitle}>O que você quer guardar?</p>
        <div className={s.kinds}>
          {list.map((t) => (
            <button key={t.kind} className={s.kind} onClick={() => choose(t.kind)}>
              <KindTile kind={t.kind} size={36} />
              <span>{t.label}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

export function useLoadVault() {
  const loaded = useVault((st) => st.loaded);
  const load = useVault((st) => st.load);
  useEffect(() => {
    if (!loaded) load().catch((e) => toast(errorMessage(e), { tone: "danger" }));
  }, [loaded, load]);
  return loaded;
}
