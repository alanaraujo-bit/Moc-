import { memo, useEffect, useMemo, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowsDownUp, Check, Key, MagnifyingGlass, Paperclip, Star, Trash, X } from "@phosphor-icons/react";
import { ItemTile } from "../../components/tile/Tile";
import { Menu } from "../../components/ui/overlays";
import { Button, IconButton, Kbd } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { relativeTime } from "../../lib/format";
import { api, errorMessage } from "../../lib/ipc";
import { highlightRanges } from "../../lib/search";
import { template } from "../../lib/templates";
import type { ItemSummary } from "../../lib/types";
import { selectVisible, useVault, type Sort, type View } from "../../state/vault";
import { newItemBus } from "./NewItem";
import s from "./ListPane.module.css";

const SORTS: { id: Sort; label: string }[] = [
  { id: "updated", label: "Modificados recentemente" },
  { id: "used", label: "Usados recentemente" },
  { id: "title", label: "Nome (A–Z)" },
  { id: "created", label: "Criados recentemente" },
];

function viewTitle(view: View, vaultName: (id: string) => string, trashCount: number): string {
  switch (view.type) {
    case "all":
      return "Tudo";
    case "favorites":
      return "Favoritos";
    case "recents":
      return "Recentes";
    case "vault":
      return vaultName(view.id);
    case "kind":
      return template(view.kind).plural;
    case "tag":
      return `#${view.tag}`;
    case "archive":
      return "Arquivados";
    case "trash":
      return trashCount ? "Lixeira" : "Lixeira";
  }
}

function Highlighted({ text, q }: { text: string; q: string }) {
  const ranges = q ? highlightRanges(text, q) : [];
  if (!ranges.length) return <>{text}</>;
  const parts: React.ReactNode[] = [];
  let last = 0;
  ranges.forEach(([a, b], i) => {
    if (a > last) parts.push(text.slice(last, a));
    parts.push(<mark key={i}>{text.slice(a, b)}</mark>);
    last = b;
  });
  parts.push(text.slice(last));
  return <>{parts}</>;
}

const Row = memo(function Row({
  item,
  selected,
  q,
  compact,
  onSelect,
}: {
  item: ItemSummary;
  selected: boolean;
  q: string;
  compact: boolean;
  onSelect: (id: string) => void;
}) {
  return (
    <div
      role="option"
      aria-selected={selected}
      id={`row-${item.id}`}
      className={s.row}
      data-selected={selected || undefined}
      data-compact={compact || undefined}
      onMouseDown={() => onSelect(item.id)}
    >
      <ItemTile kind={item.kind} title={item.title} urls={item.urls} size={compact ? 28 : 34} />
      <div className={s.rowText}>
        <span className={s.rowTitle}>
          <Highlighted text={item.title} q={q} />
        </span>
        {!compact && <span className={s.rowSub}>{item.subtitle ? <Highlighted text={item.subtitle} q={q} /> : template(item.kind).label}</span>}
      </div>
      <span className={s.rowMeta}>
        {item.hasTotp && <Key size={13} aria-label="Tem 2FA" />}
        {item.attachmentCount > 0 && <Paperclip size={13} aria-label="Tem anexos" />}
        {item.favorite && <Star size={13} weight="fill" className={s.star} aria-label="Favorito" />}
      </span>
    </div>
  );
});

export function ListPane({ searchRef }: { searchRef: React.RefObject<HTMLInputElement | null> }) {
  const items = useVault((st) => st.items);
  const index = useVault((st) => st.index);
  const view = useVault((st) => st.view);
  const query = useVault((st) => st.query);
  const sort = useVault((st) => st.sort);
  const usage = useVault((st) => st.usage);
  const vaults = useVault((st) => st.vaults);
  const selectedId = useVault((st) => st.selectedId);
  const select = useVault((st) => st.select);
  const setQuery = useVault((st) => st.setQuery);
  const setSort = useVault((st) => st.setSort);
  const refresh = useVault((st) => st.refresh);
  const compact = false;

  const visible = useMemo(() => selectVisible({ items, index, view, query, sort, usage }), [items, index, view, query, sort, usage]);
  const trashCount = useMemo(() => items.filter((i) => i.trashedAt != null).length, [items]);
  const scrollRef = useRef<HTMLDivElement>(null);
  const rowHeight = compact ? 44 : 56;
  const virt = useVirtualizer({ count: visible.length, getScrollElement: () => scrollRef.current, estimateSize: () => rowHeight, overscan: 12 });

  // Keep a valid selection: first result while searching, otherwise stay put if still visible.
  useEffect(() => {
    if (query && visible.length && (!selectedId || !visible.some((v) => v.id === selectedId))) {
      select(visible[0].id);
    }
  }, [query, visible, selectedId, select]);

  useEffect(() => {
    const i = visible.findIndex((v) => v.id === selectedId);
    if (i >= 0) virt.scrollToIndex(i, { align: "auto" });
  }, [selectedId, visible, virt]);

  const move = (delta: number) => {
    if (!visible.length) return;
    const i = visible.findIndex((v) => v.id === selectedId);
    const next = i < 0 ? 0 : Math.max(0, Math.min(visible.length - 1, i + delta));
    select(visible[next].id);
  };

  const vaultName = (id: string) => vaults.find((v) => v.id === id)?.name ?? "Cofre";
  const title = viewTitle(view, vaultName, trashCount);

  const emptyTrash = async () => {
    try {
      const n = await api.emptyTrash();
      await refresh();
      toast(n === 1 ? "1 item apagado de vez" : `${n} itens apagados de vez`);
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    }
  };

  return (
    <section className={s.root} aria-label="Itens">
      <div className={s.searchBar}>
        <div className={s.search}>
          <MagnifyingGlass size={16} className={s.searchIcon} />
          <input
            ref={searchRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={`Buscar em ${view.type === "all" || query ? "tudo" : title}`}
            aria-label="Buscar"
            spellCheck={false}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                move(1);
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                move(-1);
              } else if (e.key === "Escape") {
                if (query) {
                  e.stopPropagation();
                  setQuery("");
                }
              } else if (e.key === "Enter" && selectedId) {
                e.preventDefault();
                // Enter on a result copies the main secret — the fastest path.
                void window.dispatchEvent(new CustomEvent("moco:copy-primary"));
              }
            }}
          />
          {query ? (
            <IconButton small label="Limpar busca" onClick={() => setQuery("")}>
              <X size={14} />
            </IconButton>
          ) : (
            <span className={s.searchHint}>
              <Kbd keys="Ctrl+F" />
            </span>
          )}
        </div>
      </div>

      <header className={s.header}>
        <div className={s.headerText}>
          <h2 className={s.title}>{query ? "Resultados" : title}</h2>
          <span className={s.count}>{visible.length.toLocaleString("pt-BR")}</span>
        </div>
        {view.type === "trash" && trashCount > 0 && !query ? (
          <Button size="sm" variant="dangerGhost" icon={<Trash size={14} />} onClick={emptyTrash}>
            Esvaziar
          </Button>
        ) : (
          !query && (
            <Menu
              trigger={
                <IconButton small label="Ordenar">
                  <ArrowsDownUp size={15} />
                </IconButton>
              }
              items={SORTS.map((o) => ({
                label: o.label,
                icon: o.id === sort ? <Check size={14} /> : <span style={{ width: 14 }} />,
                onSelect: () => setSort(o.id),
              }))}
            />
          )
        )}
      </header>

      {view.type === "trash" && trashCount > 0 && !query && (
        <p className={s.notice}>Itens na lixeira são apagados de vez depois de 30 dias.</p>
      )}

      <div
        ref={scrollRef}
        className={s.scroll}
        role="listbox"
        aria-label={title}
        aria-activedescendant={selectedId ? `row-${selectedId}` : undefined}
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            move(1);
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            move(-1);
          } else if (e.key === "Home") {
            e.preventDefault();
            if (visible[0]) select(visible[0].id);
          } else if (e.key === "End") {
            e.preventDefault();
            if (visible.length) select(visible[visible.length - 1].id);
          }
        }}
      >
        {visible.length === 0 ? (
          <EmptyList view={view} query={query} onClear={() => setQuery("")} hasAny={items.some((i) => i.trashedAt == null)} />
        ) : (
          <div style={{ height: virt.getTotalSize(), position: "relative" }}>
            {virt.getVirtualItems().map((vi) => {
              const item = visible[vi.index];
              return (
                <div key={item.id} style={{ position: "absolute", top: 0, left: 0, right: 0, transform: `translateY(${vi.start}px)`, height: vi.size }}>
                  <Row item={item} selected={item.id === selectedId} q={query} compact={compact} onSelect={select} />
                </div>
              );
            })}
          </div>
        )}
      </div>
      <p className={s.meta} aria-hidden="true">
        {selectedId && visible.find((v) => v.id === selectedId)
          ? `Modificado ${relativeTime(visible.find((v) => v.id === selectedId)!.updatedAt)}`
          : " "}
      </p>
    </section>
  );
}

function EmptyList({ view, query, onClear, hasAny }: { view: View; query: string; onClear: () => void; hasAny: boolean }) {
  if (query) {
    return (
      <div className={s.empty}>
        <p className={s.emptyTitle}>Nada com “{query}”.</p>
        <p className={s.emptyText}>
          A busca olha nomes, usuários, sites, etiquetas e tipos. Dá para filtrar com <code>tipo:cartão</code>, <code>#trabalho</code> ou{" "}
          <code>is:fav</code>.
        </p>
        <Button size="sm" onClick={onClear}>
          Limpar busca
        </Button>
      </div>
    );
  }
  const copy: Record<string, [string, string]> = {
    all: hasAny ? ["Nada por aqui.", ""] : ["Seu Mocó está vazio.", "Tudo que é importante merece um lugar melhor do que um bloco de notas."],
    favorites: ["Nenhum favorito ainda.", "Marque com a estrela os itens que você mais usa — eles ficam sempre à mão aqui."],
    recents: ["Nada usado ainda.", "Os itens que você abrir ou copiar aparecem aqui, do mais recente para o mais antigo."],
    vault: ["Este cofre está vazio.", "Crie um item aqui dentro ou mova itens de outro cofre."],
    kind: ["Nenhum item deste tipo.", ""],
    tag: ["Nenhum item com esta etiqueta.", ""],
    archive: ["Nada arquivado.", "Arquive o que você não usa mais, mas não quer apagar. Sai da frente e continua guardado."],
    trash: ["A lixeira está vazia.", "Itens apagados ficam aqui por 30 dias antes de sumirem de vez."],
  };
  const [title, text] = copy[view.type];
  return (
    <div className={s.empty}>
      <p className={s.emptyTitle}>{title}</p>
      {text && <p className={s.emptyText}>{text}</p>}
      {(view.type === "all" || view.type === "vault") && (
        <Button size="sm" variant="primary" onClick={() => newItemBus.open()}>
          Adicionar item
        </Button>
      )}
    </div>
  );
}
