import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowsClockwise, CloudCheck, CloudSlash, Desktop, DeviceMobile, ShieldCheck, Trash } from "@phosphor-icons/react";
import { Confirm, Dialog } from "../../components/ui/overlays";
import { Button, PasswordField, TextField } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { relativeTime } from "../../lib/format";
import { api, errorMessage, on } from "../../lib/ipc";
import type { CloudDevice, CloudMe, CloudStatus } from "../../lib/types";
import { useVault } from "../../state/vault";
import { CodeBlock } from "../onboarding/RecoverySheet";
import s from "./SettingsScreen.module.css";
import x from "./SyncSection.module.css";

export function SyncSection() {
  const [status, setStatus] = useState<CloudStatus | null>(null);
  const [me, setMe] = useState<CloudMe | null>(null);
  const [dialog, setDialog] = useState<null | "enable" | "2fa" | "2fa-off" | "signout" | "delete">(null);
  const [revoking, setRevoking] = useState<CloudDevice | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const st = await api.cloudStatus();
      setStatus(st);
      if (st.connected) setMe(await api.cloudMe().catch(() => null));
      else setMe(null);
    } catch {
      /* locked or unavailable */
    }
  }, []);

  useEffect(() => {
    void load();
    const t = window.setInterval(load, 15000);
    const un = on("moco://synced", () => void load());
    return () => {
      window.clearInterval(t);
      void un.then((u) => u());
    };
  }, [load]);

  const syncNow = async () => {
    setBusy(true);
    try {
      const o = await api.cloudSyncNow();
      await useVault.getState().refresh();
      toast(o.pulled || o.pushed ? `Sincronizado: ${o.pushed} enviados, ${o.pulled} recebidos` : "Tudo em dia.", { tone: "success" });
      if (o.rejected) toast(`${o.rejected} alteração(ões) do servidor recusada(s) por não passarem na verificação.`, { tone: "danger", duration: 8000 });
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    } finally {
      setBusy(false);
      void load();
    }
  };

  if (!status) return null;

  if (!status.connected) {
    return (
      <>
        <div className={x.intro}>
          <CloudSlash size={22} />
          <div>
            <strong>Seu Mocó está só neste computador.</strong>
            <p>
              Ative a sincronização para ter seus itens em outros dispositivos e uma cópia segura fora daqui. O servidor recebe
              apenas dados cifrados — sem sua senha mestra e sua Chave Secreta, ninguém consegue ler, nem nós.
            </p>
          </div>
          <Button variant="primary" onClick={() => setDialog("enable")}>
            Ativar sincronização
          </Button>
        </div>
        <EnableDialog open={dialog === "enable"} onOpenChange={(o) => setDialog(o ? "enable" : null)} onDone={load} />
      </>
    );
  }

  return (
    <>
      <div className={s.row}>
        <div className={s.rowText}>
          <span className={s.rowTitle}>
            <CloudCheck size={16} className={x.ok} /> Sincronizando como {status.email}
          </span>
          <span className={s.rowDetail}>
            {status.lastError
              ? `Última tentativa falhou: ${status.lastError}`
              : status.lastSyncAt
                ? `Última sincronização ${relativeTime(status.lastSyncAt)}${status.pending ? ` · ${status.pending} alteração(ões) aguardando` : ""}.`
                : "Primeira sincronização em andamento."}
          </span>
        </div>
        <div className={s.control}>
          <Button icon={<ArrowsClockwise size={15} />} loading={busy || status.syncing} onClick={syncNow}>
            Sincronizar agora
          </Button>
        </div>
      </div>

      <div className={s.row}>
        <div className={s.rowText}>
          <span className={s.rowTitle}>Verificação em duas etapas</span>
          <span className={s.rowDetail}>
            {me?.twoFactor
              ? `Ativa. Para entrar em um dispositivo novo, o Mocó pede também um código do seu app autenticador. ${me.recoveryCodesLeft} códigos de emergência restantes.`
              : "Proteja a entrada em dispositivos novos com um código do seu app autenticador."}
          </span>
        </div>
        <div className={s.control}>
          {me?.twoFactor ? (
            <Button variant="ghost" onClick={() => setDialog("2fa-off")}>
              Desativar
            </Button>
          ) : (
            <Button icon={<ShieldCheck size={15} />} onClick={() => setDialog("2fa")}>
              Ativar
            </Button>
          )}
        </div>
      </div>

      <div className={x.devices}>
        <h3>Dispositivos conectados</h3>
        <ul>
          {me?.devices.map((d) => (
            <li key={d.id}>
              <span className={x.icon}>{d.platform === "windows" ? <Desktop size={18} /> : <DeviceMobile size={18} />}</span>
              <span className={x.devText}>
                <strong>
                  {d.name} {d.current && <span className={x.here}>este computador</span>}
                </strong>
                <span>Visto {relativeTime(d.lastSeenAt)} · conectado {relativeTime(d.createdAt)}</span>
              </span>
              {!d.current && (
                <Button size="sm" variant="dangerGhost" onClick={() => setRevoking(d)}>
                  Remover
                </Button>
              )}
            </li>
          ))}
        </ul>
      </div>

      <div className={s.row}>
        <div className={s.rowText}>
          <span className={s.rowTitle}>Desconectar este computador</span>
          <span className={s.rowDetail}>Os itens continuam aqui, só param de sincronizar.</span>
        </div>
        <div className={s.control}>
          <Button variant="ghost" onClick={() => setDialog("signout")}>
            Desconectar
          </Button>
        </div>
      </div>
      <div className={s.row}>
        <div className={s.rowText}>
          <span className={s.rowTitle}>Apagar a conta no servidor</span>
          <span className={s.rowDetail}>Remove tudo que está no servidor. Este computador mantém seus itens.</span>
        </div>
        <div className={s.control}>
          <Button variant="dangerGhost" icon={<Trash size={15} />} onClick={() => setDialog("delete")}>
            Apagar conta…
          </Button>
        </div>
      </div>

      <TotpDialog open={dialog === "2fa"} onOpenChange={(o) => setDialog(o ? "2fa" : null)} onDone={load} />
      <TotpOffDialog open={dialog === "2fa-off"} onOpenChange={(o) => setDialog(o ? "2fa-off" : null)} onDone={load} />
      <Confirm
        open={!!revoking}
        onOpenChange={(o) => !o && setRevoking(null)}
        title={`Remover “${revoking?.name ?? ""}”?`}
        description="Ele deixa de sincronizar imediatamente e não consegue mais entrar sem a senha mestra e a Chave Secreta. O que já estava salvo nele continua lá, cifrado — se o aparelho foi perdido ou roubado, troque também a senha mestra."
        confirmLabel="Remover dispositivo"
        danger
        onConfirm={async () => {
          const d = revoking;
          setRevoking(null);
          if (!d) return;
          try {
            await api.cloudRevokeDevice(d.id);
            toast(`${d.name} removido`, { tone: "success" });
            void load();
          } catch (e) {
            toast(errorMessage(e), { tone: "danger" });
          }
        }}
      />
      <Confirm
        open={dialog === "signout"}
        onOpenChange={(o) => setDialog(o ? "signout" : null)}
        title="Desconectar este computador?"
        description="O Mocó continua funcionando aqui com tudo que você tem. Alterações feitas daqui em diante não vão para outros dispositivos até você entrar de novo."
        confirmLabel="Desconectar"
        onConfirm={async () => {
          setDialog(null);
          try {
            await api.cloudSignout();
            toast("Sincronização desativada neste computador.");
            void load();
          } catch (e) {
            toast(errorMessage(e), { tone: "danger" });
          }
        }}
      />
      <DeleteAccountDialog open={dialog === "delete"} onOpenChange={(o) => setDialog(o ? "delete" : null)} onDone={load} />
    </>
  );
}

function EnableDialog({ open, onOpenChange, onDone }: { open: boolean; onOpenChange: (o: boolean) => void; onDone: () => void }) {
  const [email, setEmail] = useState("");
  const pw = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.cloudSignup(email, pw.current?.value ?? "");
      onOpenChange(false);
      onDone();
      toast("Sincronização ativada. Tá guardado aqui e na nuvem.", { tone: "success", duration: 4000 });
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
      title="Ativar sincronização"
      description="Seu e-mail identifica a conta. Para entrar em outro dispositivo você vai precisar dele, da senha mestra e da Chave Secreta do Kit de Emergência."
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancelar</Button>
          <Button variant="primary" loading={busy} onClick={submit} disabled={!email.includes("@")}>
            Ativar
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
        <TextField label="E-mail" type="email" value={email} onChange={(e) => setEmail(e.target.value)} autoFocus placeholder="voce@email.com" />
        <PasswordField ref={pw} label="Senha mestra" error={error ?? undefined} />
        <button type="submit" hidden />
      </form>
    </Dialog>
  );
}

function TotpDialog({ open, onOpenChange, onDone }: { open: boolean; onOpenChange: (o: boolean) => void; onDone: () => void }) {
  const [setup, setSetup] = useState<{ secret: string; uri: string } | null>(null);
  const [code, setCode] = useState("");
  const [codes, setCodes] = useState<string[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setSetup(null);
      setCodes(null);
      setCode("");
      setError(null);
      return;
    }
    api.cloudTotpSetup().then(setSetup).catch((e) => setError(errorMessage(e)));
  }, [open]);

  const enable = async () => {
    if (!setup) return;
    setBusy(true);
    setError(null);
    try {
      const r = await api.cloudTotpEnable(setup.secret, code);
      setCodes(r.recoveryCodes);
      onDone();
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
      title={codes ? "Guarde os códigos de emergência" : "Ativar verificação em duas etapas"}
      description={
        codes
          ? "Se perder o celular com o app autenticador, cada código abaixo permite entrar uma vez. Guarde junto do Kit de Emergência."
          : "No app autenticador (Google Authenticator, Microsoft Authenticator, Authy…), adicione uma conta com a chave abaixo e digite o código de 6 dígitos que aparecer."
      }
      footer={
        codes ? (
          <Button variant="primary" onClick={() => onOpenChange(false)}>
            Guardei
          </Button>
        ) : (
          <>
            <Button onClick={() => onOpenChange(false)}>Cancelar</Button>
            <Button variant="primary" loading={busy} disabled={code.length < 6 || !setup} onClick={enable}>
              Ativar
            </Button>
          </>
        )
      }
    >
      {codes ? (
        <CodeBlock value={codes.join("-")} label="Códigos de emergência" />
      ) : (
        <>
          {setup && <CodeBlock value={setup.secret.match(/.{1,4}/g)!.join("-")} label="Chave do autenticador" />}
          <TextField
            label="Código de 6 dígitos"
            value={code}
            inputMode="numeric"
            onChange={(e) => setCode(e.target.value.replace(/\D/g, "").slice(0, 6))}
            error={error ?? undefined}
            mono
            autoFocus
          />
        </>
      )}
    </Dialog>
  );
}

function TotpOffDialog({ open, onOpenChange, onDone }: { open: boolean; onOpenChange: (o: boolean) => void; onDone: () => void }) {
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Desativar a verificação em duas etapas?"
      description="Novos dispositivos passam a entrar só com e-mail, senha mestra e Chave Secreta."
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancelar</Button>
          <Button
            variant="danger"
            onClick={async () => {
              try {
                await api.cloudTotpDisable(code);
                onOpenChange(false);
                onDone();
                toast("Verificação em duas etapas desativada.");
              } catch (e) {
                setError(errorMessage(e));
              }
            }}
          >
            Desativar
          </Button>
        </>
      }
    >
      <TextField label="Código atual do autenticador" value={code} onChange={(e) => setCode(e.target.value.replace(/\D/g, "").slice(0, 6))} error={error ?? undefined} mono autoFocus />
    </Dialog>
  );
}

function DeleteAccountDialog({ open, onOpenChange, onDone }: { open: boolean; onOpenChange: (o: boolean) => void; onDone: () => void }) {
  const pw = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Apagar a conta no servidor?"
      description="Tudo que está no servidor — itens, dispositivos, histórico de acesso — é apagado de vez. Outros dispositivos param de sincronizar. Este computador mantém uma cópia completa dos seus itens."
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancelar</Button>
          <Button
            variant="danger"
            loading={busy}
            onClick={async () => {
              setBusy(true);
              setError(null);
              try {
                await api.cloudDeleteAccount(pw.current?.value ?? "");
                onOpenChange(false);
                onDone();
                toast("Conta apagada do servidor. Seus itens continuam neste computador.");
              } catch (e) {
                setError(errorMessage(e));
              } finally {
                setBusy(false);
              }
            }}
          >
            Apagar de vez
          </Button>
        </>
      }
    >
      <PasswordField ref={pw} label="Senha mestra" autoFocus error={error ?? undefined} />
    </Dialog>
  );
}
