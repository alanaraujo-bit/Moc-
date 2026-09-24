import { useEffect, useMemo, useRef, useState } from "react";
import { Popover } from "radix-ui";
import { MagnifyingGlass, Plus } from "@phosphor-icons/react";
import { KindTile } from "../../components/tile/Tile";
import { Button, Kbd } from "../../components/ui/primitives";
import { fold } from "../../lib/search";
import { TEMPLATES } from "../../lib/templates";
import type { ItemKind } from "../../lib/types";
import { useVault } from "../../state/vault";
import s from "./NewItem.module.css";

// Most-used kinds first; the rest one keystroke away.
const FEATURED: ItemKind[] = ["login", "card", "note", "identity", "document", "wifi"];

export const newItemBus = { open: () => {} };

export function NewItemButton() {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const vaults = useVault((st) => st.vaults);
  const view = useVault((st) => st.view);
  const edit = useVault((st) => st.edit);
  const setScreen = useVault((st) => st.setScreen);

  useEffect(() => {
    newItemBus.open = () => setOpen(true);
  }, []);

  const list = useMemo(() => {
    const f = fold(q.trim());
    const all = f
      ? TEMPLATES.filter((t) => [t.label, t.plural, t.blurb, ...t.aliases].some((a) => fold(a).includes(f)))
      : [...FEATURED.map((k) => TEMPLATES.find((t) => t.kind === k)!), ...TEMPLATES.filter((t) => !FEATURED.includes(t.kind))];
    return all;
  }, [q]);

  useEffect(() => setActive(0), [q]);

  const choose = (kind: ItemKind) => {
    const vaultId = view.type === "vault" ? view.id : vaults[0]?.id;
    if (!vaultId) return;
    setOpen(false);
    setQ("");
    setScreen("vault");
    edit({ mode: "new", kind, vaultId });
  };

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <Button variant="primary" full icon={<Plus size={16} weight="bold" />}>
          Novo item
        </Button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          className={s.pop}
          side="right"
          align="start"
          sideOffset={10}
          collisionPadding={12}
          onOpenAutoFocus={(e) => {
            e.preventDefault();
            inputRef.current?.focus();
          }}
        >
          <div className={s.search}>
            <MagnifyingGlass size={15} />
            <input
              ref={inputRef}
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="O que você quer guardar?"
              aria-label="Tipo de item"
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setActive((a) => Math.min(list.length - 1, a + 1));
                } else if (e.key === "ArrowUp") {
                  e.preventDefault();
                  setActive((a) => Math.max(0, a - 1));
                } else if (e.key === "Enter" && list[active]) {
                  e.preventDefault();
                  choose(list[active].kind);
                }
              }}
            />
            <Kbd keys="Ctrl+N" />
          </div>
          <div className={s.list} role="listbox" aria-label="Tipos de item">
            {list.map((t, i) => (
              <button
                key={t.kind}
                role="option"
                aria-selected={i === active}
                data-active={i === active || undefined}
                className={s.option}
                onMouseEnter={() => setActive(i)}
                onClick={() => choose(t.kind)}
              >
                <KindTile kind={t.kind} size={32} />
                <span className={s.text}>
                  <strong>{t.label}</strong>
                  <span>{t.blurb}</span>
                </span>
              </button>
            ))}
            {list.length === 0 && <p className={s.none}>Nenhum tipo com esse nome. Que tal “Personalizado”?</p>}
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
