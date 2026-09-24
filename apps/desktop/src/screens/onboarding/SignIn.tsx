import { useRef, useState } from "react";
import { ArrowLeft } from "@phosphor-icons/react";
import { Button, PasswordField, TextField } from "../../components/ui/primitives";
import { api, errorCode, errorMessage } from "../../lib/ipc";
import { useApp } from "../../state/app";
import s from "./Onboarding.module.css";

/** Signs in on a new computer: downloads the encrypted account and unlocks it here. */
export function SignIn({ onBack }: { onBack: () => void }) {
  const boot = useApp((st) => st.boot);
  const [email, setEmail] = useState("");
  const pw = useRef<HTMLInputElement>(null);
  const sk = useRef<HTMLInputElement>(null);
  const [totp, setTotp] = useState("");
  const [needTotp, setNeedTotp] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.cloudSignin(email, pw.current?.value ?? "", sk.current?.value ?? "", needTotp ? totp : undefined);
      if (pw.current) pw.current.value = "";
      if (sk.current) sk.current.value = "";
      await boot();
    } catch (e) {
      const code = errorCode(e);
      if (code === "totp_required") {
        setNeedTotp(true);
        setError(null);
      } else {
        setError(errorMessage(e));
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={s.step}>
      <Button variant="ghost" size="sm" icon={<ArrowLeft size={15} />} onClick={onBack} className={s.fit}>
        Voltar
      </Button>
      <h1 className={s.title}>Entrar no seu Mocó</h1>
      <p className={s.lede}>
        Use o e-mail da sua conta, a senha mestra e a <strong>Chave Secreta</strong> do seu Kit de Emergência. Depois disso, este computador lembra
        da Chave Secreta.
      </p>
      <form
        className={s.stack}
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <TextField label="E-mail" type="email" large value={email} onChange={(e) => setEmail(e.target.value)} autoFocus />
        <PasswordField ref={pw} large label="Senha mestra" />
        <TextField ref={sk} large mono label="Chave Secreta" placeholder="M1-XXXXXX-XXXXXX-XXXXXX-XXXXXX-XXXX" />
        {needTotp && (
          <TextField
            large
            mono
            label="Código de verificação"
            hint="Do seu app autenticador — ou um código de emergência."
            value={totp}
            onChange={(e) => setTotp(e.target.value.trim())}
            autoFocus
          />
        )}
        {error && (
          <p className={s.error} role="alert">
            {error}
          </p>
        )}
        <Button type="submit" variant="primary" size="lg" loading={busy} disabled={!email.includes("@")} className={s.fit}>
          {busy ? "Entrando e baixando seus itens…" : "Entrar"}
        </Button>
      </form>
    </div>
  );
}
