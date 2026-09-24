import { useEffect, useState } from "react";
import { Dialog } from "../../components/ui/overlays";
import { Button, TextField } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { api, errorMessage } from "../../lib/ipc";
import { GLAZES } from "../../lib/tile";
import type { VaultInfo } from "../../lib/types";
import { useVault } from "../../state/vault";
import s from "./VaultDialog.module.css";

const COLOR_NAMES: Record<string, string> = {
  cobalt: "Cobalto",
  sky: "Céu",
  moss: "Musgo",
  ochre: "Ocre",
  clay: "Argila",
  plum: "Ameixa",
  slate: "Ardósia",
  ink: "Nanquim",
};

export function VaultDialog({ open, onOpenChange, vault }: { open: boolean; onOpenChange: (o: boolean) => void; vault?: VaultInfo }) {
  const [name, setName] = useState("");
  const [color, setColor] = useState("cobalt");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const load = useVault((st) => st.load);
  const setView = useVault((st) => st.setView);

  useEffect(() => {
    if (open) {
      setName(vault?.name ?? "");
      setColor(vault?.color || "cobalt");
      setError(null);
    }
  }, [open, vault]);

  const save = async () => {
    if (!name.trim()) {
      setError("Dê um nome ao cofre.");
      return;
    }
    setBusy(true);
    try {
      const attrs = { name: name.trim(), description: vault?.description ?? "", icon: vault?.icon ?? "", color };
      const v = vault ? await api.updateVault(vault.id, attrs) : await api.createVault(attrs);
      await load();
      if (!vault) setView({ type: "vault", id: v.id });
      onOpenChange(false);
      toast(vault ? "Cofre atualizado" : `Cofre “${v.name}” criado`, { tone: "success" });
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={vault ? "Editar cofre" : "Novo cofre"}
      description={vault ? undefined : "Cofres separam áreas da vida — casa, trabalho, família. Cada um tem sua própria chave."}
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancelar</Button>
          <Button variant="primary" loading={busy} onClick={save}>
            {vault ? "Salvar" : "Criar cofre"}
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
        className={s.form}
      >
        <TextField label="Nome" value={name} onChange={(e) => setName(e.target.value)} placeholder="Ex.: Trabalho" autoFocus error={error} maxLength={60} />
        <div className={s.colors} role="radiogroup" aria-label="Cor">
          {GLAZES.map((g) => (
            <button
              key={g}
              type="button"
              role="radio"
              aria-checked={color === g}
              aria-label={COLOR_NAMES[g]}
              title={COLOR_NAMES[g]}
              className={s.swatch}
              data-active={color === g || undefined}
              style={{ background: `var(--glaze-${g})` }}
              onClick={() => setColor(g)}
            />
          ))}
        </div>
      </form>
    </Dialog>
  );
}
