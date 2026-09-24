import { useRef, useState } from "react";
import { DownloadSimple, FileArrowUp, Warning } from "@phosphor-icons/react";
import { Dialog } from "../../components/ui/overlays";
import { Button, PasswordField } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { api, errorMessage } from "../../lib/ipc";
import type { ExportResult } from "../../lib/types";
import { ImportDialog } from "../import/ImportDialog";
import s from "./SettingsScreen.module.css";

function announce(r: ExportResult | null, what: string) {
  if (!r) return;
  toast(`${what}: ${r.count.toLocaleString("pt-BR")} itens salvos`, { tone: "success", duration: 4000 });
  if (r.cloudSynced) {
    toast("Atenção: essa pasta sincroniza com a nuvem. Uma cópia do arquivo vai sair deste computador.", { tone: "danger", duration: 9000 });
  }
}

export function DataSection() {
  const [importing, setImporting] = useState(false);
  const [dialog, setDialog] = useState<null | "backup" | "csv">(null);
  return (
    <>
      <div className={s.row}>
        <div className={s.rowText}>
          <span className={s.rowTitle}>Importar</span>
          <span className={s.rowDetail}>Do navegador, do Bitwarden, 1Password, LastPass, Proton Pass, KeePass ou de uma planilha.</span>
        </div>
        <div className={s.control}>
          <Button icon={<FileArrowUp size={15} />} onClick={() => setImporting(true)}>
            Importar…
          </Button>
        </div>
      </div>
      <div className={s.row}>
        <div className={s.rowText}>
          <span className={s.rowTitle}>Backup do Mocó</span>
          <span className={s.rowDetail}>Tudo, cifrado com uma senha que você escolhe. Serve para guardar uma cópia ou levar para outro computador.</span>
        </div>
        <div className={s.control}>
          <Button icon={<DownloadSimple size={15} />} onClick={() => setDialog("backup")}>
            Exportar backup…
          </Button>
        </div>
      </div>
      <div className={s.row}>
        <div className={s.rowText}>
          <span className={s.rowTitle}>Logins em CSV</span>
          <span className={s.rowDetail}>Planilha sem proteção, no formato que navegadores e outros gerenciadores aceitam. Seus dados são seus.</span>
        </div>
        <div className={s.control}>
          <Button variant="ghost" onClick={() => setDialog("csv")}>
            Exportar CSV…
          </Button>
        </div>
      </div>
      <ImportDialog open={importing} onOpenChange={setImporting} />
      <BackupDialog open={dialog === "backup"} onOpenChange={(o) => setDialog(o ? "backup" : null)} />
      <CsvDialog open={dialog === "csv"} onOpenChange={(o) => setDialog(o ? "csv" : null)} />
    </>
  );
}

function BackupDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const master = useRef<HTMLInputElement>(null);
  const exportPw = useRef<HTMLInputElement>(null);
  const confirm = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{ field: string; msg: string } | null>(null);

  const submit = async () => {
    const e = exportPw.current?.value ?? "";
    if (e.length < 8) return setError({ field: "export", msg: "Use pelo menos 8 caracteres." });
    if (e !== confirm.current?.value) return setError({ field: "confirm", msg: "As senhas não estão iguais." });
    setBusy(true);
    setError(null);
    try {
      const r = await api.exportBackup(master.current?.value ?? "", e);
      if (r) {
        onOpenChange(false);
        announce(r, "Backup criado");
      }
    } catch (err) {
      setError({ field: "master", msg: errorMessage(err) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Exportar backup do Mocó"
      description="O arquivo .moco leva todos os seus itens, cifrados com a senha abaixo. Sem ela, ninguém abre — nem você. Pode ser diferente da senha mestra."
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancelar</Button>
          <Button variant="primary" loading={busy} onClick={submit}>
            Escolher onde salvar
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
        style={{ display: "flex", flexDirection: "column", gap: 14 }}
      >
        <PasswordField ref={master} label="Sua senha mestra" autoFocus error={error?.field === "master" ? error.msg : undefined} />
        <PasswordField ref={exportPw} label="Senha do backup" error={error?.field === "export" ? error.msg : undefined} />
        <PasswordField ref={confirm} label="Confirme a senha do backup" error={error?.field === "confirm" ? error.msg : undefined} />
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}

function CsvDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const pw = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.exportCsv(pw.current?.value ?? "");
      if (r) {
        onOpenChange(false);
        announce(r, "CSV criado");
      }
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
      title="Exportar logins sem proteção?"
      description={
        <>
          <Warning size={16} weight="fill" style={{ color: "var(--danger)", verticalAlign: "-3px", marginRight: 6 }} />
          O arquivo CSV mostra suas senhas em texto puro. Qualquer pessoa ou programa com acesso a ele consegue ler tudo. Use só para
          levar os dados a outro lugar, e apague o arquivo logo depois.
        </>
      }
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancelar</Button>
          <Button variant="danger" loading={busy} onClick={submit}>
            Entendi, exportar
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <PasswordField ref={pw} label="Senha mestra" autoFocus error={error ?? undefined} />
      </form>
    </Dialog>
  );
}
