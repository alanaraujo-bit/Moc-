import { useMemo, useState } from "react";
import {
  Archive,
  CaretRight,
  ClockCounterClockwise,
  GearSix,
  Hash,
  LockSimple,
  Password,
  Plus,
  ShieldCheck,
  SquaresFour,
  Star,
  Trash,
} from "@phosphor-icons/react";
import { MocoMark, Wordmark } from "../../components/brand/Brand";
import { KindGlyph } from "../../components/tile/Tile";
import { IconButton } from "../../components/ui/primitives";
import { TEMPLATES } from "../../lib/templates";
import type { ItemKind } from "../../lib/types";
import { inView, tagTree, useVault, type TagNode, type View } from "../../state/vault";
import { NewItemButton } from "./NewItem";
import { VaultDialog } from "./VaultDialog";
import s from "./Sidebar.module.css";

function sameView(a: View, b: View) {
  if (a.type !== b.type) return false;
  if (a.type === "vault" && b.type === "vault") return a.id === b.id;
  if (a.type === "kind" && b.type === "kind") return a.kind === b.kind;
  if (a.type === "tag" && b.type === "tag") return a.tag === b.tag;
  return true;
}

function NavItem({
  icon,
  label,
  count,
  active,
  onClick,
  depth = 0,
  trailing,
}: {
  icon: React.ReactNode;
  label: string;
  count?: number;
  active: boolean;
  onClick: () => void;
  depth?: number;
  trailing?: React.ReactNode;
}) {
  return (
    <button
      className={s.item}
      data-active={active || undefined}
      aria-current={active ? "page" : undefined}
      onClick={onClick}
      style={{ paddingLeft: 10 + depth * 14 }}
    >
      <span className={s.icon}>{icon}</span>
      <span className={s.label}>{label}</span>
      {trailing}
      {count !== undefined && count > 0 && <span className={s.count}>{count.toLocaleString("pt-BR")}</span>}
    </button>
  );
}

function TagBranch({ node, depth, view, setView }: { node: TagNode; depth: number; view: View; setView: (v: View) => void }) {
  const [open, setOpen] = useState(depth === 0 && node.children.length > 0 ? false : false);
  const active = view.type === "tag" && view.tag === node.path;
  return (
    <>
      <NavItem
        icon={<Hash size={15} />}
        label={node.name}
        count={node.count}
        depth={depth}
        active={active}
        onClick={() => setView({ type: "tag", tag: node.path })}
        trailing={
          node.children.length > 0 ? (
            <span
              className={s.caret}
              data-open={open || undefined}
              role="button"
              aria-label={open ? "Recolher" : "Expandir"}
              onClick={(e) => {
                e.stopPropagation();
                setOpen((o) => !o);
              }}
            >
              <CaretRight size={11} weight="bold" />
            </span>
          ) : null
        }
      />
      {open && node.children.map((c) => <TagBranch key={c.path} node={c} depth={depth + 1} view={view} setView={setView} />)}
    </>
  );
}

export function Sidebar({ onLock }: { onLock: () => void }) {
  const items = useVault((st) => st.items);
  const usage = useVault((st) => st.usage);
  const vaults = useVault((st) => st.vaults);
  const view = useVault((st) => st.view);
  const screen = useVault((st) => st.screen);
  const setView = useVault((st) => st.setView);
  const setScreen = useVault((st) => st.setScreen);
  const [vaultDialog, setVaultDialog] = useState(false);

  const counts = useMemo(() => {
    const c = (v: View) => items.filter((it) => inView(it, v, usage)).length;
    const kinds = new Map<ItemKind, number>();
    for (const it of items) if (it.trashedAt == null && !it.archived) kinds.set(it.kind, (kinds.get(it.kind) ?? 0) + 1);
    return {
      all: c({ type: "all" }),
      favorites: c({ type: "favorites" }),
      archive: c({ type: "archive" }),
      trash: c({ type: "trash" }),
      kinds,
    };
  }, [items, usage]);

  const tags = useMemo(() => tagTree(items), [items]);
  const onVault = screen === "vault";
  const is = (v: View) => onVault && sameView(view, v);

  return (
    <nav className={s.root} aria-label="Navegação">
      <div className={s.brand}>
        <MocoMark size={22} />
        <Wordmark height={17} />
        <span className={s.spacer} />
        <IconButton label="Trancar agora" shortcut="Ctrl+L" onClick={onLock} small>
          <LockSimple size={16} />
        </IconButton>
      </div>

      <div className={s.newRow}>
        <NewItemButton />
      </div>

      <div className={s.scroll}>
        <div className={s.group}>
          <NavItem icon={<SquaresFour size={16} />} label="Tudo" count={counts.all} active={is({ type: "all" })} onClick={() => setView({ type: "all" })} />
          <NavItem
            icon={<Star size={16} />}
            label="Favoritos"
            count={counts.favorites}
            active={is({ type: "favorites" })}
            onClick={() => setView({ type: "favorites" })}
          />
          <NavItem
            icon={<ClockCounterClockwise size={16} />}
            label="Recentes"
            active={is({ type: "recents" })}
            onClick={() => setView({ type: "recents" })}
          />
        </div>

        <div className={s.group}>
          <div className={s.heading}>
            <span>Cofres</span>
            <IconButton label="Novo cofre" small onClick={() => setVaultDialog(true)} tooltipSide="right">
              <Plus size={13} weight="bold" />
            </IconButton>
          </div>
          {vaults.map((v) => (
            <NavItem
              key={v.id}
              icon={<span className={s.vaultChip} style={{ background: `var(--glaze-${v.color || "cobalt"})` }} />}
              label={v.name}
              count={v.itemCount}
              active={is({ type: "vault", id: v.id })}
              onClick={() => setView({ type: "vault", id: v.id })}
            />
          ))}
        </div>

        {counts.kinds.size > 0 && (
          <div className={s.group}>
            <div className={s.heading}>
              <span>Tipos</span>
            </div>
            {TEMPLATES.filter((t) => counts.kinds.has(t.kind)).map((t) => (
              <NavItem
                key={t.kind}
                icon={<KindGlyph kind={t.kind} size={16} />}
                label={t.plural}
                count={counts.kinds.get(t.kind)}
                active={is({ type: "kind", kind: t.kind })}
                onClick={() => setView({ type: "kind", kind: t.kind })}
              />
            ))}
          </div>
        )}

        {tags.length > 0 && (
          <div className={s.group}>
            <div className={s.heading}>
              <span>Etiquetas</span>
            </div>
            {tags.map((t) => (
              <TagBranch key={t.path} node={t} depth={0} view={view} setView={setView} />
            ))}
          </div>
        )}

        <div className={s.group}>
          <NavItem
            icon={<Archive size={16} />}
            label="Arquivados"
            count={counts.archive}
            active={is({ type: "archive" })}
            onClick={() => setView({ type: "archive" })}
          />
          <NavItem icon={<Trash size={16} />} label="Lixeira" count={counts.trash} active={is({ type: "trash" })} onClick={() => setView({ type: "trash" })} />
        </div>
      </div>

      <div className={s.tools}>
        <NavItem
          icon={<ShieldCheck size={16} />}
          label="Central de segurança"
          active={screen === "security"}
          onClick={() => setScreen("security")}
        />
        <NavItem icon={<Password size={16} />} label="Gerador de senhas" active={screen === "generator"} onClick={() => setScreen("generator")} />
        <NavItem icon={<GearSix size={16} />} label="Configurações" active={screen === "settings"} onClick={() => setScreen("settings")} />
      </div>

      <VaultDialog open={vaultDialog} onOpenChange={setVaultDialog} />
    </nav>
  );
}
