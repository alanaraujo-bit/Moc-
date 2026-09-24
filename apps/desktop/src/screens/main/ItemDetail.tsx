import { useCallback, useEffect, useState } from "react";
import {
  Archive,
  ArrowCounterClockwise,
  ArrowSquareOut,
  ClockCounterClockwise,
  Copy,
  CopySimple,
  DotsThree,
  FolderSimple,
  PencilSimple,
  Star,
  Trash,
} from "@phosphor-icons/react";
import { ItemTile } from "../../components/tile/Tile";
import { Confirm, Menu, type MenuEntry } from "../../components/ui/overlays";
import { Button, IconButton } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { fullDate, relativeTime } from "../../lib/format";
import { api, errorMessage } from "../../lib/ipc";
import { template } from "../../lib/templates";
import { hostOf } from "../../lib/tile";
import type { FieldView, ItemKind, ItemView } from "../../lib/types";
import { useVault } from "../../state/vault";
import { CardFace } from "./CardFace";
import { copyField, FieldRow } from "./fields";
import { HistoryDialog } from "./HistoryDialog";
import { Attachments, WifiShare } from "./Attachments";
import { isMobile } from "../../lib/platform";
import s from "./ItemDetail.module.css";

/** The field copied by Enter in search and Ctrl+C in the list, per kind. */
export const PRIMARY_FIELD: Record<ItemKind, string> = {
  login: "password",
  password: "password",
  card: "number",
  identity: "cpf",
  document: "number",
  note: "notes",
  wifi: "password",
  license: "licenseKey",
  bankAccount: "accountNumber",
  server: "password",
  database: "password",
  apiCredential: "apiKey",
  sshKey: "publicKey",
  cryptoWallet: "address",
  healthPlan: "memberNumber",
  vehicle: "plate",
  custom: "",
};

function labelFor(item: ItemView, f: FieldView): string {
  if (f.label) return f.label;
  return template(item.kind).fields.find((t) => t.id === f.id)?.label ?? f.id;
}

export function ItemDetail({ id }: { id: string }) {
  const [item, setItem] = useState<ItemView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [purging, setPurging] = useState(false);
  const [history, setHistory] = useState(false);
  const itemVersion = useVault((st) => st.itemVersion);
  const vaults = useVault((st) => st.vaults);
  const refresh = useVault((st) => st.refresh);
  const edit = useVault((st) => st.edit);
  const select = useVault((st) => st.select);

  useEffect(() => {
    let alive = true;
    setError(null);
    api
      .item(id)
      .then((it) => alive && setItem(it))
      .catch((e) => alive && setError(errorMessage(e)));
    return () => {
      alive = false;
    };
  }, [id, itemVersion]);

  const copyPrimary = useCallback(() => {
    if (!item) return;
    const t = template(item.kind);
    const fid = PRIMARY_FIELD[item.kind] || item.fields.find((f) => f.hasValue)?.id;
    if (!fid) return;
    const f = item.fields.find((x) => x.id === fid);
    if (fid !== "notes" && !f?.hasValue) {
      toast(`${t.label} sem ${labelFor(item, { id: fid, label: "", type: "text", value: null, hasValue: false, section: null, strength: null }).toLowerCase()} para copiar`);
      return;
    }
    void copyField(item.id, fid, fid === "notes" ? "Nota" : labelFor(item, f!));
  }, [item]);

  useEffect(() => {
    const onCopy = () => copyPrimary();
    const onCopyUser = () => {
      if (item?.fields.some((f) => f.id === "username" && f.hasValue)) void copyField(item.id, "username", "Usuário");
    };
    window.addEventListener("moco:copy-primary", onCopy);
    window.addEventListener("moco:copy-username", onCopyUser);
    return () => {
      window.removeEventListener("moco:copy-primary", onCopy);
      window.removeEventListener("moco:copy-username", onCopyUser);
    };
  }, [copyPrimary, item]);

  if (error) return <div className={s.state}>{error}</div>;
  if (!item) return <div className={s.state} aria-busy="true" />;

  const t = template(item.kind);
  const vault = vaults.find((v) => v.id === item.vaultId);
  const trashed = item.trashedAt != null;

  const act = async (fn: () => Promise<unknown>, msg: string, undo?: () => Promise<unknown>) => {
    try {
      await fn();
      await refresh();
      toast(msg, undo ? { action: { label: "Desfazer", run: () => void undo().then(refresh) } } : { tone: "success" });
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    }
  };

  const menu: MenuEntry[] = trashed
    ? [
        { label: "Restaurar", icon: <ArrowCounterClockwise size={16} />, onSelect: () => act(() => api.restore(item.id), "Item restaurado") },
        { separator: true },
        { label: "Apagar de vez…", icon: <Trash size={16} />, danger: true, onSelect: () => setPurging(true) },
      ]
    : [
        { label: "Duplicar", icon: <CopySimple size={16} />, onSelect: () => act(() => api.duplicate(item.id).then((c) => select(c.id)), "Item duplicado") },
        ...(vaults.length > 1
          ? [
              { heading: "Mover para" } as MenuEntry,
              ...vaults
                .filter((v) => v.id !== item.vaultId)
                .map((v) => ({
                  label: v.name,
                  icon: <FolderSimple size={16} />,
                  onSelect: () => act(() => api.move(item.id, v.id), `Movido para ${v.name}`),
                })),
              { separator: true } as MenuEntry,
            ]
          : [{ separator: true } as MenuEntry]),
        { label: "Histórico de versões", icon: <ClockCounterClockwise size={16} />, onSelect: () => setHistory(true) },
        {
          label: item.archived ? "Desarquivar" : "Arquivar",
          icon: <Archive size={16} />,
          onSelect: () =>
            act(
              () => api.setArchived(item.id, !item.archived),
              item.archived ? "Item desarquivado" : "Arquivado. Continua guardado, só saiu da frente.",
              item.archived ? undefined : () => api.setArchived(item.id, false),
            ),
        },
        { separator: true },
        {
          label: "Mover para a lixeira",
          icon: <Trash size={16} />,
          shortcut: "Del",
          danger: true,
          onSelect: () => act(() => api.trash(item.id), "Movido para a lixeira", () => api.restore(item.id)),
        },
      ];

  // Group fields by template section, keeping custom fields at the end.
  const sections = t.sections ?? [];
  const bySection = (sid: string | null) => item.fields.filter((f) => (f.section ?? null) === sid && (f.hasValue || f.type === "totp"));
  const unsectioned = item.fields.filter((f) => !f.section || !sections.some((x) => x.id === f.section)).filter((f) => f.hasValue);
  const prominent = new Set(t.fields.filter((f) => f.prominent).map((f) => f.id));

  return (
    <article className={s.root} aria-label={item.title}>
      {trashed && (
        <div className={s.banner}>
          <Trash size={16} />
          <span>Na lixeira desde {relativeTime(item.trashedAt!)}. Some de vez em 30 dias.</span>
          <Button size="sm" onClick={() => act(() => api.restore(item.id), "Item restaurado")}>
            Restaurar
          </Button>
        </div>
      )}
      <header className={s.header}>
        <ItemTile kind={item.kind} title={item.title} urls={item.urls} size={52} />
        <div className={s.headText}>
          <h1 className={`${s.title} selectable`}>{item.title}</h1>
          <p className={s.crumbs}>
            {vault && (
              <span className={s.vault}>
                <span className={s.vaultChip} style={{ background: `var(--glaze-${vault.color || "cobalt"})` }} />
                {vault.name}
              </span>
            )}
            <span>{t.label}</span>
            {item.archived && <span className={s.flag}>Arquivado</span>}
          </p>
        </div>
        <div className={s.headActions}>
          {!trashed && (
            <IconButton
              label={item.favorite ? "Tirar dos favoritos" : "Favoritar"}
              active={item.favorite}
              onClick={() => act(() => api.setFavorite(item.id, !item.favorite), item.favorite ? "Saiu dos favoritos" : "Favoritado")}
            >
              <Star size={18} weight={item.favorite ? "fill" : "regular"} className={item.favorite ? s.starOn : undefined} />
            </IconButton>
          )}
          {!trashed && (
            <Button icon={<PencilSimple size={15} />} onClick={() => edit({ mode: "edit", id: item.id })}>
              Editar
            </Button>
          )}
          <Menu
            trigger={
              <IconButton label="Mais ações">
                <DotsThree size={20} weight="bold" />
              </IconButton>
            }
            items={menu}
          />
        </div>
      </header>

      <div className={s.body}>
        {item.kind === "card" && <CardFace item={item} />}

        {unsectioned.length > 0 && (
          <section className={s.group}>
            {unsectioned.map((f) => (
              <FieldRow key={f.id} itemId={item.id} field={f} label={labelFor(item, f)} prominent={prominent.has(f.id)} revision={item.revision} />
            ))}
          </section>
        )}

        {item.kind === "wifi" && !trashed && <WifiShare item={item} />}

        {sections.map((sec) => {
          const fields = bySection(sec.id);
          if (!fields.length) return null;
          return (
            <section key={sec.id} className={s.group}>
              <h2 className={s.groupTitle}>{sec.label}</h2>
              {fields.map((f) => (
                <FieldRow key={f.id} itemId={item.id} field={f} label={labelFor(item, f)} revision={item.revision} />
              ))}
            </section>
          );
        })}

        {item.urls.length > 0 && (
          <section className={s.group}>
            <h2 className={s.groupTitle}>{item.urls.length === 1 ? "Site" : "Sites"}</h2>
            {item.urls.map((u) => (
              <div key={u} className={s.urlRow}>
                <button className={s.url} onClick={() => api.openUrl(u)} title="Abrir no navegador">
                  <span className={s.host}>{hostOf(u)}</span>
                  <span className={s.fullUrl}>{u}</span>
                </button>
                <IconButton small label="Abrir no navegador" onClick={() => api.openUrl(u)}>
                  <ArrowSquareOut size={15} />
                </IconButton>
                <IconButton
                  small
                  label="Copiar endereço"
                  onClick={async () => {
                    const r = await api.copyText(u, false);
                    void r;
                    toast("Endereço copiado", { tone: "success" });
                  }}
                >
                  <Copy size={15} />
                </IconButton>
              </div>
            ))}
          </section>
        )}

        {item.notes && (
          <section className={s.group}>
            <h2 className={s.groupTitle}>{item.kind === "note" ? "Nota" : "Observações"}</h2>
            <p className={`${s.notes} selectable`}>{item.notes}</p>
          </section>
        )}

        {item.tags.length > 0 && (
          <section className={s.tags}>
            {item.tags.map((tag) => (
              <button key={tag} className={s.tag} onClick={() => useVault.getState().setView({ type: "tag", tag })}>
                #{tag}
              </button>
            ))}
          </section>
        )}

        {/* Picking and saving files on Android needs content:// support in files.rs — desktop only for now. */}
        {!isMobile && <Attachments item={item} />}

        {item.passwordHistory.length > 0 && <PasswordHistory item={item} />}

        <footer className={s.meta}>
          <span title={fullDate(item.updatedAt)}>Modificado {relativeTime(item.updatedAt)}</span>
          <span title={fullDate(item.createdAt)}>Criado {relativeTime(item.createdAt)}</span>
        </footer>
      </div>

      <Confirm
        open={purging}
        onOpenChange={setPurging}
        title={`Apagar “${item.title}” de vez?`}
        description="Não dá para desfazer. O item, o histórico de versões e as senhas antigas somem deste computador."
        confirmLabel="Apagar de vez"
        danger
        onConfirm={async () => {
          setPurging(false);
          await act(() => api.purge(item.id), "Apagado de vez");
          select(null);
        }}
      />
      <HistoryDialog open={history} onOpenChange={setHistory} item={item} />
    </article>
  );
}

function PasswordHistory({ item }: { item: ItemView }) {
  const [open, setOpen] = useState(false);
  return (
    <section className={s.group}>
      <button className={s.disclosure} onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        Senhas anteriores <span className={s.count}>{item.passwordHistory.length}</span>
      </button>
      {open &&
        item.passwordHistory.map((h, i) => (
          <FieldRow
            key={i}
            itemId={item.id}
            revision={item.revision}
            label={`Trocada ${relativeTime(h.replacedAt)}`}
            field={{ id: `history:${i}`, label: "", type: "concealed", value: null, hasValue: true, section: null, strength: null }}
          />
        ))}
    </section>
  );
}
