import { useEffect, useMemo, useState } from "react";
import { Command } from "cmdk";
import { Dialog as RDialog } from "radix-ui";
import {
  ArrowSquareOut,
  Copy,
  FileArrowUp,
  GearSix,
  LockSimple,
  MoonStars,
  Password,
  PencilSimple,
  Plus,
  ShieldCheck,
  Star,
  Sun,
  User,
} from "@phosphor-icons/react";
import { ItemTile, KindGlyph } from "../../components/tile/Tile";
import { Kbd } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { api, errorMessage } from "../../lib/ipc";
import { search } from "../../lib/search";
import { TEMPLATES, template } from "../../lib/templates";
import type { ItemSummary } from "../../lib/types";
import { useApp } from "../../state/app";
import { useVault } from "../../state/vault";
import { copyField } from "./fields";
import { PRIMARY_FIELD } from "./ItemDetail";
import s from "./CommandPalette.module.css";

export const paletteBus = { open: () => {} };

export function CommandPalette({ onLock, onImport }: { onLock: () => void; onImport: () => void }) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [target, setTarget] = useState<ItemSummary | null>(null);
  const index = useVault((st) => st.index);
  const usage = useVault((st) => st.usage);
  const vaults = useVault((st) => st.vaults);
  const theme = useApp((st) => st.theme);
  const updateSettings = useApp((st) => st.updateSettings);

  useEffect(() => {
    paletteBus.open = () => {
      setTarget(null);
      setQ("");
      setOpen(true);
    };
  }, []);

  const items = useMemo(() => (q.trim() ? search(index, q, usage).slice(0, 8).map((h) => h.item) : []), [q, index, usage]);

  const run = (fn: () => void) => {
    setOpen(false);
    window.setTimeout(fn, 10);
  };

  const st = useVault.getState;
  const newItem = (kind: (typeof TEMPLATES)[number]["kind"]) =>
    run(() => {
      const v = st().view;
      const vaultId = v.type === "vault" ? v.id : vaults[0]?.id;
      if (vaultId) {
        st().setScreen("vault");
        st().edit({ mode: "new", kind, vaultId });
      }
    });

  const openItem = (it: ItemSummary) =>
    run(() => {
      st().setView({ type: "all" });
      st().setQuery("");
      st().select(it.id);
    });

  return (
    <RDialog.Root open={open} onOpenChange={setOpen}>
      <RDialog.Portal>
        <RDialog.Overlay className={s.overlay} />
        <RDialog.Content className={s.content} aria-describedby={undefined}>
          <RDialog.Title className="sr-only">Comandos</RDialog.Title>
          <Command label="Comandos" loop>
            <div className={s.inputRow}>
              {target && (
                <span className={s.scope}>
                  <ItemTile kind={target.kind} title={target.title} urls={target.urls} size={20} />
                  {target.title}
                </span>
              )}
              <Command.Input
                value={q}
                onValueChange={setQ}
                placeholder={target ? "O que fazer com este item?" : "Busque itens ou digite um comando…"}
                className={s.input}
                onKeyDown={(e) => {
                  if (e.key === "Backspace" && !q && target) setTarget(null);
                }}
              />
            </div>
            <Command.List className={s.list}>
              <Command.Empty className={s.empty}>Nada encontrado.</Command.Empty>

              {target ? (
                <Command.Group heading={template(target.kind).label} className={s.group}>
                  <Command.Item className={s.item} onSelect={() => run(() => void copyField(target.id, PRIMARY_FIELD[target.kind] || "notes", "Copiado"))}>
                    <Copy size={16} /> Copiar {PRIMARY_FIELD[target.kind] === "password" ? "senha" : "principal"}
                    <span className={s.kbd}>
                      <Kbd keys="Enter" />
                    </span>
                  </Command.Item>
                  {target.kind === "login" && (
                    <Command.Item className={s.item} onSelect={() => run(() => void copyField(target.id, "username", "Usuário"))}>
                      <User size={16} /> Copiar usuário
                    </Command.Item>
                  )}
                  {target.hasTotp && (
                    <Command.Item className={s.item} onSelect={() => run(() => void copyField(target.id, "totp", "Código"))}>
                      <Password size={16} /> Copiar código 2FA
                    </Command.Item>
                  )}
                  {target.urls[0] && (
                    <Command.Item className={s.item} onSelect={() => run(() => void api.openUrl(target.urls[0]))}>
                      <ArrowSquareOut size={16} /> Abrir site
                    </Command.Item>
                  )}
                  <Command.Item className={s.item} onSelect={() => openItem(target)}>
                    <KindGlyph kind={target.kind} size={16} /> Ver detalhes
                  </Command.Item>
                  <Command.Item
                    className={s.item}
                    onSelect={() =>
                      run(() => {
                        openItem(target);
                        st().edit({ mode: "edit", id: target.id });
                      })
                    }
                  >
                    <PencilSimple size={16} /> Editar
                  </Command.Item>
                  <Command.Item
                    className={s.item}
                    onSelect={() =>
                      run(async () => {
                        try {
                          await api.setFavorite(target.id, !target.favorite);
                          await st().refresh();
                          toast(target.favorite ? "Saiu dos favoritos" : "Favoritado", { tone: "success" });
                        } catch (e) {
                          toast(errorMessage(e), { tone: "danger" });
                        }
                      })
                    }
                  >
                    <Star size={16} /> {target.favorite ? "Tirar dos favoritos" : "Favoritar"}
                  </Command.Item>
                </Command.Group>
              ) : (
                <>
                  {items.length > 0 && (
                    <Command.Group heading="Itens" className={s.group}>
                      {items.map((it) => (
                        <Command.Item
                          key={it.id}
                          value={`${it.title} ${it.subtitle} ${it.urls.join(" ")} ${it.keywords.join(" ")} ${q} ${it.id}`}
                          className={s.item}
                          onSelect={() => {
                            setTarget(it);
                            setQ("");
                          }}
                        >
                          <ItemTile kind={it.kind} title={it.title} urls={it.urls} size={24} />
                          <span className={s.itemText}>
                            <strong>{it.title}</strong>
                            <span>{it.subtitle || template(it.kind).label}</span>
                          </span>
                          <span className={s.kbd}>ações ›</span>
                        </Command.Item>
                      ))}
                    </Command.Group>
                  )}
                  <Command.Group heading="Criar" className={s.group}>
                    {TEMPLATES.map((t) => (
                      <Command.Item key={t.kind} value={`novo ${t.label} ${t.aliases.join(" ")}`} className={s.item} onSelect={() => newItem(t.kind)}>
                        <Plus size={16} /> Novo {t.label.toLowerCase()}
                      </Command.Item>
                    ))}
                  </Command.Group>
                  <Command.Group heading="Ir para" className={s.group}>
                    <Command.Item value="central de seguranca senhas fracas" className={s.item} onSelect={() => run(() => st().setScreen("security"))}>
                      <ShieldCheck size={16} /> Central de segurança
                    </Command.Item>
                    <Command.Item value="gerador de senhas gerar" className={s.item} onSelect={() => run(() => st().setScreen("generator"))}>
                      <Password size={16} /> Gerador de senhas <span className={s.kbd}><Kbd keys="Ctrl+G" /></span>
                    </Command.Item>
                    <Command.Item value="configuracoes preferencias" className={s.item} onSelect={() => run(() => st().setScreen("settings"))}>
                      <GearSix size={16} /> Configurações <span className={s.kbd}><Kbd keys="Ctrl+," /></span>
                    </Command.Item>
                    {vaults.map((v) => (
                      <Command.Item key={v.id} value={`cofre ${v.name}`} className={s.item} onSelect={() => run(() => st().setView({ type: "vault", id: v.id }))}>
                        <span className={s.chip} style={{ background: `var(--glaze-${v.color || "cobalt"})` }} /> Cofre {v.name}
                      </Command.Item>
                    ))}
                  </Command.Group>
                  <Command.Group heading="Ações" className={s.group}>
                    <Command.Item value="importar senhas de outro gerenciador navegador" className={s.item} onSelect={() => run(onImport)}>
                      <FileArrowUp size={16} /> Importar…
                    </Command.Item>
                    <Command.Item
                      value="tema escuro claro aparencia"
                      className={s.item}
                      onSelect={() => run(() => void updateSettings({ theme: theme === "dark" ? "light" : "dark" }))}
                    >
                      {theme === "dark" ? <Sun size={16} /> : <MoonStars size={16} />} Tema {theme === "dark" ? "claro" : "escuro"}
                    </Command.Item>
                    <Command.Item value="trancar bloquear sair" className={s.item} onSelect={() => run(onLock)}>
                      <LockSimple size={16} /> Trancar agora <span className={s.kbd}><Kbd keys="Ctrl+L" /></span>
                    </Command.Item>
                  </Command.Group>
                </>
              )}
            </Command.List>
          </Command>
        </RDialog.Content>
      </RDialog.Portal>
    </RDialog.Root>
  );
}
