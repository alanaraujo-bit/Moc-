import { useCallback, useEffect, useRef, useState } from "react";
import { deviceNoun } from "../../lib/platform";
import { ArrowRight, Fingerprint, Key, Question } from "@phosphor-icons/react";
import { MocoMark } from "../../components/brand/Brand";
import { Button, IconButton, PasswordField, Spinner, TextField } from "../../components/ui/primitives";
import { TileWall } from "../../components/wall/TileWall";
import { api, errorCode, errorMessage } from "../../lib/ipc";
import { useApp } from "../../state/app";
import { RecoverFlow } from "./RecoverFlow";
import s from "./LockScreen.module.css";

const REASONS: Record<string, string> = {
  idle: "Trancamos por inatividade.",
  session: "Trancamos quando o Windows foi bloqueado.",
  sleep: "Trancamos quando o computador dormiu.",
  minimize: "Trancamos ao minimizar.",
  background: "Trancamos porque o Mocó ficou em segundo plano.",
};

export function LockScreen({ reason }: { reason?: string | null }) {
  const info = useApp((st) => st.info);
  const setPhase = useApp((st) => st.setPhase);
  const pwRef = useRef<HTMLInputElement>(null);
  const skRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [needSecretKey, setNeedSecretKey] = useState(!info?.secretKeyOnDevice);
  const [caps, setCaps] = useState(false);
  const [shake, setShake] = useState(0);
  const [turning, setTurning] = useState(false);
  const [recovering, setRecovering] = useState(false);
  const helloReady = !!info?.helloEnrolled && !info?.passwordDue && !needSecretKey;
  const helloTried = useRef(false);
  const [helloBusy, setHelloBusy] = useState(false);

  const tryHello = useCallback(async () => {
    setHelloBusy(true);
    setError(null);
    try {
      await api.unlockHello();
      setTurning(true);
    } catch (e) {
      const code = errorCode(e);
      if (code !== "hello_canceled") setError(errorMessage(e));
      pwRef.current?.focus();
    } finally {
      setHelloBusy(false);
    }
  }, []);

  useEffect(() => {
    if (helloReady && !helloTried.current && reason !== "manual") {
      helloTried.current = true;
      void tryHello();
    }
  }, [helloReady, tryHello, reason]);

  useEffect(() => {
    pwRef.current?.focus();
  }, [needSecretKey, recovering]);

  const submit = async () => {
    const password = pwRef.current?.value ?? "";
    if (!password) {
      setError("Digite sua senha mestra.");
      return;
    }
    const secretKey = needSecretKey ? skRef.current?.value ?? "" : undefined;
    setBusy(true);
    setError(null);
    try {
      await api.unlock(password, secretKey);
      // Don't keep the password around in the DOM.
      if (pwRef.current) pwRef.current.value = "";
      if (skRef.current) skRef.current.value = "";
      setTurning(true);
    } catch (e) {
      const code = errorCode(e);
      if (code === "secret_key_missing") {
        setNeedSecretKey(true);
        setError(null);
        setBusy(false);
        window.setTimeout(() => skRef.current?.focus(), 30);
        return;
      }
      setError(code === "bad_credentials" && needSecretKey ? "Senha mestra ou Chave Secreta incorreta." : errorMessage(e));
      setShake((n) => n + 1);
      pwRef.current?.select();
      setBusy(false);
    }
  };

  const onTurned = useCallback(() => setPhase("unlocked"), [setPhase]);

  if (recovering) {
    return <RecoverFlow onCancel={() => setRecovering(false)} needSecretKey={needSecretKey} />;
  }

  return (
    <div className={s.root}>
      <TileWall opening={{ cols: 9, rows: 8 }} turning={turning} onTurned={onTurned} />
      <form
        className={s.panel}
        data-leaving={turning || undefined}
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <MocoMark size={52} />
        <div className={s.copy}>
          <h1 className={s.title}>Seu Mocó está trancado.</h1>
          <p className={s.sub}>
            {reason && REASONS[reason]
              ? REASONS[reason]
              : info?.helloEnrolled && info.passwordDue
                ? "Hoje é dia de digitar a senha mestra — para você nunca esquecê-la."
                : "Digite sua senha mestra para abrir."}
          </p>
        </div>

        <div className={s.fields} key={shake} data-shake={shake > 0 || undefined}>
          <PasswordField
            ref={pwRef}
            large
            aria-label="Senha mestra"
            placeholder="Senha mestra"
            disabled={busy}
            error={error}
            hint={caps ? "Caps Lock está ativado." : undefined}
            onKeyDown={(e) => setCaps(e.getModifierState("CapsLock"))}
            onKeyUp={(e) => setCaps(e.getModifierState("CapsLock"))}
            trailing={
              !needSecretKey && (
                <IconButton label="Abrir" type="submit" className={s.go} disabled={busy} tooltipSide="right">
                  {busy ? <Spinner size={16} /> : <ArrowRight size={18} weight="bold" />}
                </IconButton>
              )
            }
          />
          {needSecretKey && (
            <>
              <TextField
                ref={skRef}
                large
                mono
                label={
                  <span className={s.skLabel}>
                    <Key size={14} /> Chave Secreta
                  </span>
                }
                placeholder="M1-XXXXXX-XXXXXX-XXXXXX-XXXXXX-XXXX"
                hint={`Ela está no seu Kit de Emergência. Depois de digitar uma vez, este ${deviceNoun} lembra.`}
                disabled={busy}
              />
              <Button type="submit" variant="primary" size="lg" full loading={busy}>
                {busy ? "Abrindo…" : "Abrir"}
              </Button>
            </>
          )}
        </div>

        {helloReady && (
          <Button variant="secondary" size="lg" full icon={<Fingerprint size={18} />} loading={helloBusy} onClick={tryHello}>
            Usar o Windows Hello
          </Button>
        )}
        <div className={s.foot}>
          <Button variant="ghost" size="sm" icon={<Question size={15} />} onClick={() => setRecovering(true)} disabled={busy}>
            Esqueci a senha mestra
          </Button>
        </div>
      </form>
      <div className={s.corner}>Mocó {info?.version}</div>
    </div>
  );
}
