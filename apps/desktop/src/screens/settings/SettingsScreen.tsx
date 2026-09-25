import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { CloudArrowUp, Desktop, Eye, Fingerprint, HardDrives, Info, Key, Palette, ShieldCheck, UserCircle } from "@phosphor-icons/react";
import { MasterPasswordForm, type MasterPasswordState } from "../../components/strength/MasterPasswordForm";
import { Confirm, Dialog } from "../../components/ui/overlays";
import { Button, PasswordField, Segmented, Switch } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { relativeTime } from "../../lib/format";
import { api, errorMessage } from "../../lib/ipc";
import type { SecurityStatus, Settings } from "../../lib/types";
import { useApp } from "../../state/app";
import { EmergencyKitSheet, PrintPortal, RecoverySheetPrint } from "../onboarding/Printable";
import { CodeBlock, printNow } from "../onboarding/RecoverySheet";
import { HelloSetting } from "./HelloSetting";
import { DataSection } from "./DataSection";
import { SyncSection } from "./SyncSection";
import { useUpdates } from "../main/Updates";
import { deviceNoun, isMobile } from "../../lib/platform";
import s from "./SettingsScreen.module.css";

const ALL_SECTIONS = [
  { id: "aparencia", label: "Aparência", icon: <Palette size={16} /> },
  { id: "seguranca", label: "Segurança", icon: <ShieldCheck size={16} /> },
  { id: "desbloqueio", label: "Desbloqueio", icon: <Fingerprint size={16} /> },
  { id: "conta", label: "Conta e recuperação", icon: <UserCircle size={16} /> },
  { id: "sync", label: "Sincronização", icon: <CloudArrowUp size={16} /> },
  { id: "windows", label: "Windows", icon: <Desktop size={16} /> },
  { id: "dados", label: "Importar e exportar", icon: <HardDrives size={16} /> },
  { id: "privacidade", label: "Privacidade", icon: <Eye size={16} /> },
  { id: "sobre", label: "Sobre o Mocó", icon: <Info size={16} /> },
];

// Windows integration and file import/export are desktop features for now.
const SECTIONS = isMobile ? ALL_SECTIONS.filter((x) => !["windows", "dados"].includes(x.id)) : ALL_SECTIONS;

function Row({ title, detail, children, htmlFor }: { title: ReactNode; detail?: ReactNode; children: ReactNode; htmlFor?: string }) {
  return (
    <div className={s.row}>
      <label className={s.rowText} htmlFor={htmlFor}>
        <span className={s.rowTitle}>{title}</span>
        {detail && <span className={s.rowDetail}>{detail}</span>}
      </label>
      <div className={s.control}>{children}</div>
    </div>
  );
}

function Select<T extends string | number>({ value, options, onChange, label }: { value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; label: string }) {
  return (
    <select
      className={s.select}
      aria-label={label}
      value={String(value)}
      onChange={(e) => {
        const o = options.find((x) => String(x.value) === e.target.value);
        if (o) onChange(o.value);
      }}
    >
      {options.map((o) => (
        <option key={String(o.value)} value={String(o.value)}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

export function SettingsScreen() {
  const settings = useApp((st) => st.settings);
  const info = useApp((st) => st.info);
  const updateSettings = useApp((st) => st.updateSettings);
  const [status, setStatus] = useState<SecurityStatus | null>(null);
  const updates = useUpdates();
  const [active, setActive] = useState("aparencia");
  const scroller = useRef<HTMLDivElement>(null);

  const reloadStatus = useCallback(() => {
    api.security().then(setStatus).catch(() => {});
  }, []);
  useEffect(reloadStatus, [reloadStatus]);

  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const onScroll = () => {
      const tops = SECTIONS.map((sec) => [sec.id, document.getElementById(sec.id)?.offsetTop ?? 0] as const);
      const y = el.scrollTop + 80;
      let cur = SECTIONS[0].id;
      for (const [id, top] of tops) if (top <= y) cur = id;
      setActive(cur);
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  if (!settings) return null;

  const set = async (patch: Partial<Settings>) => {
    try {
      await updateSettings(patch);
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    }
  };

  return (
    <div className={s.root}>
      <nav className={s.index} aria-label="Seções">
        <h1 className={s.title}>Configurações</h1>
        {SECTIONS.map((sec) => (
          <button
            key={sec.id}
            className={s.indexItem}
            data-active={active === sec.id || undefined}
            onClick={() => document.getElementById(sec.id)?.scrollIntoView({ behavior: "smooth", block: "start" })}
          >
            {sec.icon}
            {sec.label}
          </button>
        ))}
      </nav>

      <div className={s.scroll} ref={scroller}>
        <section id="aparencia" className={s.section}>
          <h2>Aparência</h2>
          <Row title="Tema" detail="“Sistema” segue o modo claro ou escuro do aparelho.">
            <Segmented<Settings["theme"]>
              label="Tema"
              value={settings.theme}
              onChange={(theme) => set({ theme })}
              options={[
                { value: "system", label: "Sistema" },
                { value: "light", label: "Claro" },
                { value: "dark", label: "Escuro" },
              ]}
            />
          </Row>
        </section>

        <section id="seguranca" className={s.section}>
          <h2>Segurança</h2>
          <Row title="Trancar automaticamente" detail={isMobile ? "Depois de um tempo com o Mocó em segundo plano." : "Depois de um tempo sem usar o computador — não só o Mocó."}>
            <Select<number>
              label="Tempo para trancar"
              value={settings.autoLockMinutes}
              onChange={(autoLockMinutes) => set({ autoLockMinutes })}
              options={[
                { value: 1, label: "1 minuto" },
                { value: 5, label: "5 minutos" },
                { value: 10, label: "10 minutos" },
                { value: 30, label: "30 minutos" },
                { value: 60, label: "1 hora" },
                { value: 240, label: "4 horas" },
                { value: 0, label: "Nunca" },
              ]}
            />
          </Row>
{!isMobile && (<>
          <Row title="Trancar ao bloquear o Windows" detail="Win+L, troca de usuário ou tela de bloqueio.">
            <Switch label="Trancar ao bloquear o Windows" checked={settings.lockOnSessionLock} onChange={(lockOnSessionLock) => set({ lockOnSessionLock })} />
          </Row>
          <Row title="Trancar quando o computador dormir">
            <Switch label="Trancar ao dormir" checked={settings.lockOnSleep} onChange={(lockOnSleep) => set({ lockOnSleep })} />
          </Row>
          <Row title="Trancar ao minimizar">
            <Switch label="Trancar ao minimizar" checked={settings.lockOnMinimize} onChange={(lockOnMinimize) => set({ lockOnMinimize })} />
          </Row>
</>)}
          <Row
            title="Limpar a área de transferência"
            detail={isMobile ? "Senhas copiadas somem depois desse tempo — só se nada novo tiver sido copiado. Se você sair do Mocó antes, o Android só deixa limpar quando você voltar a ele (e apaga sozinho depois de uma hora). O teclado não mostra a prévia delas." : "Senhas copiadas somem depois desse tempo — só se nada novo tiver sido copiado. Elas nunca vão para o histórico (Win+V) nem para a nuvem."}
          >
            <Select<number>
              label="Tempo para limpar"
              value={settings.clipboardClearSeconds}
              onChange={(clipboardClearSeconds) => set({ clipboardClearSeconds })}
              options={[
                { value: 30, label: "30 segundos" },
                { value: 60, label: "1 minuto" },
                { value: 90, label: "90 segundos" },
                { value: 180, label: "3 minutos" },
                { value: 0, label: "Nunca" },
              ]}
            />
          </Row>
{!isMobile && (<>
          <Row title="Esconder de capturas de tela" detail="O Mocó aparece em preto em prints, gravações e chamadas de vídeo.">
            <Switch
              label="Esconder de capturas de tela"
              checked={settings.screenCaptureProtection}
              onChange={(screenCaptureProtection) => set({ screenCaptureProtection })}
            />
          </Row>
</>)}
          <Row title="Verificar vazamentos de senhas" detail="Consulta anônima no Have I Been Pwned: só 5 caracteres de uma impressão digital saem daqui.">
            <Switch label="Verificar vazamentos" checked={settings.breachCheck} onChange={(breachCheck) => set({ breachCheck })} />
          </Row>
        </section>

        <section id="desbloqueio" className={s.section}>
          <h2>Desbloqueio</h2>
          <HelloSetting status={status} onChange={reloadStatus} />
          <Row title="Pedir a senha mestra de vez em quando" detail={isMobile ? "Mesmo com a biometria — para você não esquecer a senha." : "Mesmo com o Windows Hello — para você não esquecer a senha."}>
            <Select<number>
              label="Frequência"
              value={settings.requirePasswordDays}
              onChange={(requirePasswordDays) => set({ requirePasswordDays })}
              options={[
                { value: 7, label: "A cada 7 dias" },
                { value: 14, label: "A cada 14 dias" },
                { value: 30, label: "A cada 30 dias" },
                { value: 0, label: "Nunca" },
              ]}
            />
          </Row>
        </section>

        <section id="conta" className={s.section}>
          <h2>Conta e recuperação</h2>
          <AccountActions status={status} onChange={reloadStatus} />
        </section>

        <section id="sync" className={s.section}>
          <h2>Sincronização e dispositivos</h2>
          <SyncSection />
        </section>

{!isMobile && (<>
        <section id="windows" className={s.section}>
          <h2>Windows</h2>
          <Row title="Abrir com o Windows" detail="O Mocó começa trancado, na bandeja do sistema.">
            <Switch label="Abrir com o Windows" checked={settings.launchAtStartup} onChange={(launchAtStartup) => set({ launchAtStartup })} />
          </Row>
          <Row title="Fechar para a bandeja" detail="O X esconde a janela; o Mocó continua pronto no canto do relógio.">
            <Switch label="Fechar para a bandeja" checked={settings.closeToTray} onChange={(closeToTray) => set({ closeToTray })} />
          </Row>
          <Row title="Acesso rápido" detail="Abre uma busca do Mocó por cima de qualquer programa.">
            <kbd className={s.shortcut}>Ctrl + Shift + Espaço</kbd>
          </Row>
        </section>
</>)}

{!isMobile && (<>
        <section id="dados" className={s.section}>
          <h2>Importar e exportar</h2>
          <DataSection />
        </section>
</>)}

        <section id="privacidade" className={s.section}>
          <h2>Privacidade</h2>
          <div className={s.privacy}>
            <div>
              <h3>O que fica neste {deviceNoun}</h3>
              <p>
                Seu cofre, cifrado com XChaCha20-Poly1305 — incluindo nomes, endereços e etiquetas, não só as senhas. A Chave Secreta,
                {" "}
                {isMobile ? "protegida pelo Keystore do Android (fica no chip de segurança do aparelho)" : "protegida pelo Windows (DPAPI)"}. As configurações do app, que não contêm segredos.
              </p>
            </div>
            <div>
              <h3>O que o Mocó consegue ver</h3>
              <p>
                Nada do que você guarda. Sem a sua senha mestra e a sua Chave Secreta, o conteúdo é ilegível — para nós também. Não há
                “recuperação pelo suporte”.
              </p>
            </div>
            <div>
              <h3>O que sai daqui</h3>
              <p>
                Com a sincronização ativa, seus itens vão para o servidor do Mocó já cifrados, junto com o mínimo para funcionar:
                seu e-mail, identificadores aleatórios, datas de alteração, tamanhos aproximados e a lista de dispositivos. Fora
                isso, só o que você pedir: a verificação de vazamentos (anônima) e a busca de atualizações. Não há telemetria de uso
                nem anúncios.
              </p>
            </div>
          </div>
        </section>

        <section id="sobre" className={s.section}>
          <h2>Sobre o Mocó</h2>
{!isMobile && (<>
          <Row title={`Versão ${info?.version ?? ""}`} detail="Atualizações são verificadas pela assinatura digital do Mocó antes de instalar.">
            <Button loading={updates.checking} onClick={() => void updates.check(true)}>
              Procurar atualizações
            </Button>
          </Row>
          <Row title="Atualizar automaticamente" detail="Baixa em segundo plano e avisa quando estiver pronta. Nunca reinicia sem você pedir.">
            <Switch label="Atualizar automaticamente" checked={settings.autoUpdate} onChange={(autoUpdate) => set({ autoUpdate })} />
          </Row>
          <Row title="Canal" detail="O beta recebe novidades antes, com um pouco mais de risco de defeitos.">
            <Segmented<Settings["updateChannel"]>
              label="Canal de atualização"
              value={settings.updateChannel}
              onChange={(updateChannel) => set({ updateChannel })}
              options={[
                { value: "stable", label: "Estável" },
                { value: "beta", label: "Beta" },
              ]}
            />
          </Row>
</>)}
          <p className={s.footnote}>Feito no Brasil. Guardado com carinho.</p>
        </section>
      </div>
    </div>
  );
}

function AccountActions({ status, onChange }: { status: SecurityStatus | null; onChange: () => void }) {
  const [dialog, setDialog] = useState<null | "password" | "kit" | "recovery" | "disableRecovery">(null);
  return (
    <>
      <Row title="Senha mestra" detail={status?.passwordChangedAt ? `Definida ${relativeTime(status.passwordChangedAt)}.` : undefined}>
        <Button onClick={() => setDialog("password")}>Trocar senha mestra</Button>
      </Row>
      <Row title="Kit de Emergência" detail="Mostra de novo a sua Chave Secreta para imprimir ou salvar.">
        <Button icon={<Key size={15} />} onClick={() => setDialog("kit")}>
          Ver Kit de Emergência
        </Button>
      </Row>
      <Row
        title="Código de recuperação"
        detail={
          status?.recoveryEnabled
            ? `Ativo desde ${status.recoveryCreatedAt ? relativeTime(status.recoveryCreatedAt) : "a criação"}. Gerar um novo invalida o anterior.`
            : "Desativado: se esquecer a senha mestra, não haverá como recuperar seus dados."
        }
      >
        <div className={s.buttons}>
          <Button onClick={() => setDialog("recovery")}>{status?.recoveryEnabled ? "Gerar novo código" : "Criar código"}</Button>
          {status?.recoveryEnabled && (
            <Button variant="dangerGhost" onClick={() => setDialog("disableRecovery")}>
              Desativar
            </Button>
          )}
        </div>
      </Row>
      <ChangePasswordDialog open={dialog === "password"} onOpenChange={(o) => setDialog(o ? "password" : null)} onDone={onChange} />
      <RevealDialog kind="kit" open={dialog === "kit"} onOpenChange={(o) => setDialog(o ? "kit" : null)} onDone={onChange} />
      <RevealDialog kind="recovery" open={dialog === "recovery"} onOpenChange={(o) => setDialog(o ? "recovery" : null)} onDone={onChange} />
      <Confirm
        open={dialog === "disableRecovery"}
        onOpenChange={(o) => setDialog(o ? "disableRecovery" : null)}
        title="Desativar o código de recuperação?"
        description={
          <>
            Sem ele, esquecer a senha mestra significa <strong>perder tudo o que está no Mocó</strong>. Nem nós conseguimos ajudar.
          </>
        }
        confirmLabel="Desativar"
        danger
        onConfirm={async () => {
          try {
            await api.disableRecovery();
            toast("Código de recuperação desativado");
            onChange();
          } catch (e) {
            toast(errorMessage(e), { tone: "danger" });
          }
          setDialog(null);
        }}
      />
    </>
  );
}

function ChangePasswordDialog({ open, onOpenChange, onDone }: { open: boolean; onOpenChange: (o: boolean) => void; onDone: () => void }) {
  const current = useRef<HTMLInputElement>(null);
  const [next, setNext] = useState<MasterPasswordState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const onNext = useCallback((st: MasterPasswordState) => setNext(st), []);

  const submit = async () => {
    if (!next?.valid) return;
    setBusy(true);
    setError(null);
    try {
      await api.changePassword(current.current?.value ?? "", next.read());
      next.clear();
      onOpenChange(false);
      onDone();
      toast("Senha mestra trocada. Use a nova a partir de agora.", { tone: "success", duration: 4000 });
    } catch (e) {
      setError(errorMessage(e).replace("Senha mestra incorreta.", "A senha atual não confere."));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Trocar a senha mestra"
      description="Seus itens continuam onde estão. Só a chave que abre o Mocó muda. A Chave Secreta continua a mesma."
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancelar</Button>
          <Button variant="primary" loading={busy} disabled={!next?.valid} onClick={submit}>
            Trocar senha
          </Button>
        </>
      }
    >
      <PasswordField ref={current} label="Senha mestra atual" autoFocus error={error ?? undefined} />
      <MasterPasswordForm onChange={onNext} onSubmit={submit} labels={{ password: "Nova senha mestra", confirm: "Confirme a nova senha" }} />
    </Dialog>
  );
}

function RevealDialog({ kind, open, onOpenChange, onDone }: { kind: "kit" | "recovery"; open: boolean; onOpenChange: (o: boolean) => void; onDone: () => void }) {
  const pw = useRef<HTMLInputElement>(null);
  const [value, setValue] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [printing, setPrinting] = useState(false);

  useEffect(() => {
    if (!open) {
      setValue(null);
      setError(null);
    }
  }, [open]);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      const password = pw.current?.value ?? "";
      const v = kind === "kit" ? await api.revealSecretKey(password) : await api.rotateRecovery(password);
      setValue(v);
      if (kind === "recovery") onDone();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const isKit = kind === "kit";
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={isKit ? "Kit de Emergência" : "Novo código de recuperação"}
      description={
        value
          ? isKit
            ? "Imprima ou salve em PDF e guarde longe do computador."
            : "O código anterior já não vale mais. Guarde este no lugar dele, separado do Kit de Emergência."
          : "Por segurança, confirme sua senha mestra."
      }
      footer={
        value ? (
          <>
            <Button onClick={() => printNow(setPrinting)}>Imprimir ou salvar em PDF</Button>
            <Button variant="primary" onClick={() => onOpenChange(false)}>
              Pronto
            </Button>
          </>
        ) : (
          <>
            <Button onClick={() => onOpenChange(false)}>Cancelar</Button>
            <Button variant="primary" loading={busy} onClick={submit}>
              {isKit ? "Mostrar" : "Gerar novo código"}
            </Button>
          </>
        )
      }
    >
      {value ? (
        <CodeBlock value={value} label={isKit ? "Chave Secreta" : "Código de recuperação"} />
      ) : (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          <PasswordField ref={pw} label="Senha mestra" autoFocus error={error ?? undefined} />
        </form>
      )}
      {printing && value && <PrintPortal>{isKit ? <EmergencyKitSheet secretKey={value} /> : <RecoverySheetPrint code={value} />}</PrintPortal>}
    </Dialog>
  );
}
