import { useRef, useState } from "react";
import { Fingerprint } from "@phosphor-icons/react";
import { Dialog } from "../../components/ui/overlays";
import { Button, PasswordField, Switch } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { api, errorCode, errorMessage } from "../../lib/ipc";
import type { SecurityStatus } from "../../lib/types";
import { useApp } from "../../state/app";
import { isMobile } from "../../lib/platform";

// Same feature, the words each platform uses for it.
const T = isMobile
  ? {
      title: "Abrir com a biometria",
      detail: "Digital ou rosto do celular. A chave fica no chip de segurança do aparelho e só vale aqui.",
      unavailable: "Cadastre uma digital ou rosto nas configurações do Android (Segurança) para usar aqui.",
      dialog: "Confirme sua senha mestra. Em seguida o Android vai pedir sua digital ou rosto.",
      on: "Biometria ativada. Da próxima vez, é só encostar o dedo ou olhar para o celular.",
      off: "Biometria desativada neste celular.",
      canceled: "Você cancelou a biometria. Tente de novo quando quiser.",
    }
  : {
      title: "Abrir com o Windows Hello",
      detail: "Rosto, digital ou PIN do Windows. A chave fica no chip de segurança do computador e só vale aqui.",
      unavailable: "O Windows Hello não está configurado neste computador. Ative em Configurações do Windows › Contas › Opções de entrada.",
      dialog: "Confirme sua senha mestra. Em seguida o Windows vai pedir seu rosto, digital ou PIN — duas vezes, para conferir.",
      on: "Windows Hello ativado. Da próxima vez, é só olhar ou encostar o dedo.",
      off: "Windows Hello desativado neste computador.",
      canceled: "Você cancelou o Windows Hello. Tente de novo quando quiser.",
    };
import s from "./SettingsScreen.module.css";

export function HelloSetting({ status, onChange }: { status: SecurityStatus | null; onChange: () => void }) {
  const info = useApp((st) => st.info);
  const boot = useApp((st) => st.boot);
  const enrolled = !!status?.deviceKeys.some((k) => k === "windows-hello" || k === "android-biometric");
  const available = !!info?.helloAvailable;
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pw = useRef<HTMLInputElement>(null);

  const enable = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.helloEnable(pw.current?.value ?? "");
      setAsking(false);
      onChange();
      void boot();
      toast(T.on, { tone: "success", duration: 4000 });
    } catch (e) {
      const code = errorCode(e);
      if (code === "hello_canceled") setError(T.canceled);
      else setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const disable = async () => {
    try {
      await api.helloDisable();
      onChange();
      void boot();
      toast(T.off);
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    }
  };

  return (
    <>
      <div className={s.row}>
        <label className={s.rowText} htmlFor="hello-switch">
          <span className={s.rowTitle}>{T.title}</span>
          <span className={s.rowDetail}>
            {available
              ? T.detail
              : T.unavailable}
          </span>
        </label>
        <div className={s.control}>
          <Switch
            id="hello-switch"
            label={T.title}
            checked={enrolled}
            disabled={!available && !enrolled}
            onChange={(v) => (v ? setAsking(true) : void disable())}
          />
        </div>
      </div>
      <Dialog
        open={asking}
        onOpenChange={setAsking}
        title={isMobile ? "Ativar a biometria" : "Ativar o Windows Hello"}
        description={T.dialog}
        footer={
          <>
            <Button onClick={() => setAsking(false)}>Cancelar</Button>
            <Button variant="primary" icon={<Fingerprint size={16} />} loading={busy} onClick={enable}>
              Continuar
            </Button>
          </>
        }
      >
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void enable();
          }}
        >
          <PasswordField ref={pw} label="Senha mestra" autoFocus error={error ?? undefined} />
        </form>
      </Dialog>
    </>
  );
}
