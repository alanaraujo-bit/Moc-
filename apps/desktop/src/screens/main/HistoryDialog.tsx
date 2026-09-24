import { useEffect, useState } from "react";
import { Dialog } from "../../components/ui/overlays";
import { Button } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { fullDate, relativeTime } from "../../lib/format";
import { api, errorMessage } from "../../lib/ipc";
import type { ItemView, VersionView } from "../../lib/types";
import { useVault } from "../../state/vault";
import s from "./HistoryDialog.module.css";

function diffSummary(a: ItemView, b: ItemView): string[] {
  const out: string[] = [];
  if (a.title !== b.title) out.push(`Nome: “${a.title}”`);
  const ids = new Set([...a.fields.map((f) => f.id), ...b.fields.map((f) => f.id)]);
  for (const id of ids) {
    const fa = a.fields.find((f) => f.id === id);
    const fb = b.fields.find((f) => f.id === id);
    const label = fa?.label || fb?.label || id;
    if (!fa && fb) out.push(`Sem o campo ${label}`);
    else if (fa && !fb) out.push(`Tinha o campo ${label}`);
    else if (fa && fb && (fa.value !== fb.value || fa.hasValue !== fb.hasValue)) {
      out.push(fa.value === null ? `${label} diferente` : `${label}: ${fa.value || "(vazio)"}`);
    }
  }
  if (a.urls.join() !== b.urls.join()) out.push("Sites diferentes");
  if (a.notes !== b.notes) out.push("Observações diferentes");
  if (a.tags.join() !== b.tags.join()) out.push("Etiquetas diferentes");
  return out.length ? out : ["Sem diferenças visíveis"];
}

export function HistoryDialog({ open, onOpenChange, item }: { open: boolean; onOpenChange: (o: boolean) => void; item: ItemView }) {
  const [versions, setVersions] = useState<VersionView[] | null>(null);
  const [busy, setBusy] = useState<number | null>(null);
  const refresh = useVault((st) => st.refresh);

  useEffect(() => {
    if (!open) return;
    setVersions(null);
    api.history(item.id).then(setVersions).catch((e) => toast(errorMessage(e), { tone: "danger" }));
  }, [open, item.id]);

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      wide
      title="Histórico de versões"
      description="O Mocó guarda as 20 últimas versões de cada item. Restaurar cria uma versão nova — nada se perde."
      footer={<Button onClick={() => onOpenChange(false)}>Fechar</Button>}
    >
      {versions === null ? (
        <p className={s.muted}>Carregando…</p>
      ) : versions.length === 0 ? (
        <p className={s.muted}>Este item ainda não foi alterado.</p>
      ) : (
        <ol className={s.list}>
          {versions.map((v) => (
            <li key={v.revision} className={s.version}>
              <div className={s.when}>
                <strong title={fullDate(v.savedAt)}>{relativeTime(v.savedAt)}</strong>
                <span>versão {v.revision}</span>
              </div>
              <ul className={s.changes}>
                {diffSummary(v.item, item).map((c, i) => (
                  <li key={i}>{c}</li>
                ))}
              </ul>
              <Button
                size="sm"
                loading={busy === v.revision}
                onClick={async () => {
                  setBusy(v.revision);
                  try {
                    await api.restoreVersion(item.id, v.revision);
                    await refresh();
                    onOpenChange(false);
                    toast("Versão restaurada", { tone: "success" });
                  } catch (e) {
                    toast(errorMessage(e), { tone: "danger" });
                  } finally {
                    setBusy(null);
                  }
                }}
              >
                Restaurar
              </Button>
            </li>
          ))}
        </ol>
      )}
    </Dialog>
  );
}
