import { useMemo, useRef, useState } from "react";
import { ArrowLeft, CheckCircle, FileArrowUp, Warning } from "@phosphor-icons/react";
import { KindTile } from "../../components/tile/Tile";
import { Dialog } from "../../components/ui/overlays";
import { Button, PasswordField, Switch } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { api, errorMessage } from "../../lib/ipc";
import type { ImportPreview, ImportSource } from "../../lib/types";
import { useVault } from "../../state/vault";
import s from "./ImportDialog.module.css";

const SOURCES: { id: ImportSource | "moco"; name: string; how: string }[] = [
  { id: "chrome", name: "Chrome, Edge ou Brave", how: "Configurações › Senhas (ou Gerenciador de senhas) › Exportar senhas. Gera um .csv." },
  { id: "firefox", name: "Firefox", how: "about:logins › menu ⋯ › Exportar logins. Gera um .csv." },
  { id: "bitwarden", name: "Bitwarden", how: "Ferramentas › Exportar cofre › formato .json (sem criptografia) ou .csv." },
  { id: "onePassword", name: "1Password", how: "Arquivo › Exportar › escolha o formato 1PUX." },
  { id: "lastPass", name: "LastPass", how: "Opções avançadas › Exportar. Gera um .csv." },
  { id: "protonPass", name: "Proton Pass", how: "Configurações › Exportar › formato CSV." },
  { id: "keePass", name: "KeePass / KeePassXC", how: "Banco de dados › Exportar › CSV." },
  { id: "genericCsv", name: "Outra planilha (CSV)", how: "Qualquer planilha com colunas como nome, site, usuário e senha — em português ou inglês." },
  { id: "moco", name: "Backup do Mocó", how: "Um arquivo .moco criado em Exportar › Backup do Mocó." },
];

type Step = "source" | "backupPassword" | "preview" | "done";

export function ImportDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const vaults = useVault((st) => st.vaults);
  const refresh = useVault((st) => st.refresh);
  const setView = useVault((st) => st.setView);
  const [step, setStep] = useState<Step>("source");
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [vaultId, setVaultId] = useState<string>("");
  const [skipDup, setSkipDup] = useState(true);
  const [foldersAsTags, setFoldersAsTags] = useState(true);
  const [result, setResult] = useState<{ imported: number; skipped: number } | null>(null);
  const [hover, setHover] = useState<string | null>(null);
  const pw = useRef<HTMLInputElement>(null);

  const reset = () => {
    setStep("source");
    setPreview(null);
    setError(null);
    setResult(null);
  };

  const close = (o: boolean) => {
    if (!o) {
      if (step === "preview") void api.importCancel();
      reset();
    }
    onOpenChange(o);
  };

  const pick = async (source: ImportSource) => {
    setBusy(true);
    setError(null);
    try {
      const p = await api.importPick(source);
      if (p) {
        setPreview(p);
        setVaultId(vaults[0]?.id ?? "");
        setStep("preview");
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const pickBackup = async () => {
    setBusy(true);
    setError(null);
    try {
      const p = await api.importBackupPick(pw.current?.value ?? "");
      if (p) {
        setPreview(p);
        setVaultId(vaults[0]?.id ?? "");
        setStep("preview");
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const commit = async () => {
    if (!preview) return;
    setBusy(true);
    try {
      const r = await api.importCommit({
        vaultId,
        skipDuplicates: skipDup,
        foldersAsTags,
        tag: `importado/${preview.source.split(",")[0].split(" ")[0].toLowerCase()}`,
        exclude: [],
      });
      setResult(r);
      setStep("done");
      await refresh();
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    } finally {
      setBusy(false);
    }
  };

  const toImport = preview ? preview.total - (skipDup ? preview.duplicates : 0) : 0;
  const rows = useMemo(() => (preview ? preview.rows.slice(0, 200) : []), [preview]);
  const howTo = SOURCES.find((x) => x.id === hover);

  return (
    <Dialog
      open={open}
      onOpenChange={close}
      wide
      title={
        step === "source"
          ? "Trazer suas coisas para o Mocó"
          : step === "backupPassword"
            ? "Abrir backup do Mocó"
            : step === "preview"
              ? `Encontramos ${preview?.total.toLocaleString("pt-BR")} ${preview?.total === 1 ? "item" : "itens"}`
              : "Pronto, tá tudo aqui."
      }
      description={
        step === "source"
          ? "Escolha de onde vêm os dados. O arquivo é lido aqui, no seu computador — nada é enviado."
          : step === "preview"
            ? `${preview?.fileName} · ${preview?.source}`
            : undefined
      }
      footer={
        step === "preview" ? (
          <>
            <Button icon={<ArrowLeft size={15} />} variant="ghost" onClick={() => (void api.importCancel(), reset())}>
              Outro arquivo
            </Button>
            <span style={{ flex: 1 }} />
            <Button onClick={() => close(false)}>Cancelar</Button>
            <Button variant="primary" loading={busy} disabled={toImport === 0 || !vaultId} onClick={commit}>
              Importar {toImport.toLocaleString("pt-BR")} {toImport === 1 ? "item" : "itens"}
            </Button>
          </>
        ) : step === "done" ? (
          <>
            <Button onClick={() => close(false)}>Fechar</Button>
            <Button
              variant="primary"
              onClick={() => {
                setView({ type: "vault", id: vaultId });
                close(false);
              }}
            >
              Ver itens
            </Button>
          </>
        ) : step === "backupPassword" ? (
          <>
            <Button variant="ghost" onClick={() => setStep("source")}>
              Voltar
            </Button>
            <Button variant="primary" loading={busy} onClick={pickBackup}>
              Escolher arquivo .moco
            </Button>
          </>
        ) : (
          <Button onClick={() => close(false)}>Cancelar</Button>
        )
      }
    >
      {step === "source" && (
        <div className={s.sourceWrap}>
          <ul className={s.sources}>
            {SOURCES.map((src) => (
              <li key={src.id}>
                <button
                  className={s.source}
                  disabled={busy}
                  onMouseEnter={() => setHover(src.id)}
                  onFocus={() => setHover(src.id)}
                  onClick={() => (src.id === "moco" ? setStep("backupPassword") : void pick(src.id as ImportSource))}
                >
                  <FileArrowUp size={18} />
                  {src.name}
                </button>
              </li>
            ))}
          </ul>
          <div className={s.how} aria-live="polite">
            {howTo ? (
              <>
                <strong>Como exportar do {howTo.name}</strong>
                <p>{howTo.how}</p>
                <p className={s.tip}>Depois de importar, apague o arquivo exportado: ele guarda suas senhas sem proteção.</p>
              </>
            ) : (
              <p className={s.muted}>Passe o mouse numa opção para ver como exportar de lá.</p>
            )}
            {error && (
              <p className={s.error} role="alert">
                <Warning size={15} weight="fill" /> {error}
              </p>
            )}
          </div>
        </div>
      )}

      {step === "backupPassword" && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void pickBackup();
          }}
        >
          <PasswordField ref={pw} label="Senha do backup" hint="A senha escolhida quando o backup foi criado." autoFocus error={error ?? undefined} />
        </form>
      )}

      {step === "preview" && preview && (
        <div className={s.preview}>
          <div className={s.stats}>
            <span>
              <strong>{toImport.toLocaleString("pt-BR")}</strong> para importar
            </span>
            {preview.duplicates > 0 && (
              <span>
                <strong>{preview.duplicates}</strong> {preview.duplicates === 1 ? "parece repetido" : "parecem repetidos"}
              </span>
            )}
            {preview.withoutPassword > 0 && (
              <span>
                <strong>{preview.withoutPassword}</strong> sem senha
              </span>
            )}
            {preview.skipped.length > 0 && (
              <span>
                <strong>{preview.skipped.length}</strong> ignorados
              </span>
            )}
          </div>
          <div className={s.options}>
            <label className={s.opt}>
              <span>Guardar no cofre</span>
              <select value={vaultId} onChange={(e) => setVaultId(e.target.value)} className={s.select}>
                {vaults.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.name}
                  </option>
                ))}
              </select>
            </label>
            {preview.duplicates > 0 && (
              <label className={s.opt}>
                <span>Pular os que já estão no Mocó</span>
                <Switch checked={skipDup} onChange={setSkipDup} label="Pular repetidos" />
              </label>
            )}
            {preview.folders.length > 0 && (
              <label className={s.opt}>
                <span>Transformar {preview.folders.length === 1 ? "a pasta" : `as ${preview.folders.length} pastas`} em etiquetas</span>
                <Switch checked={foldersAsTags} onChange={setFoldersAsTags} label="Pastas como etiquetas" />
              </label>
            )}
          </div>
          <ul className={s.rows}>
            {rows.map((r) => (
              <li key={r.index} className={s.row} data-dim={(r.duplicate && skipDup) || undefined}>
                <KindTile kind={r.kind} size={26} />
                <span className={s.rowTitle}>{r.title}</span>
                <span className={s.rowSub}>{r.subtitle}</span>
                {r.folder && foldersAsTags && <span className={s.tag}>#{r.folder}</span>}
                {r.duplicate ? <span className={s.flag}>{skipDup ? "já existe · pulado" : "já existe"}</span> : r.issue && <span className={s.flagWarn}>{r.issue}</span>}
              </li>
            ))}
            {preview.rows.length > rows.length && <li className={s.more}>e mais {preview.rows.length - rows.length}…</li>}
          </ul>
          {preview.skipped.length > 0 && (
            <details className={s.skipped}>
              <summary>Ignorados ({preview.skipped.length})</summary>
              <ul>
                {preview.skipped.map(([name, why], i) => (
                  <li key={i}>
                    {name} — {why}
                  </li>
                ))}
              </ul>
            </details>
          )}
        </div>
      )}

      {step === "done" && result && (
        <div className={s.done}>
          <CheckCircle size={28} weight="fill" className={s.ok} />
          <div>
            <p>
              <strong>{result.imported.toLocaleString("pt-BR")}</strong> {result.imported === 1 ? "item importado" : "itens importados"}
              {result.skipped > 0 && `, ${result.skipped} pulados`}.
            </p>
            <p className={s.muted}>
              Todos ganharam a etiqueta <code>importado</code>, para você revisar com calma. Agora apague o arquivo exportado — ele não tem
              proteção nenhuma.
            </p>
          </div>
        </div>
      )}
    </Dialog>
  );
}
