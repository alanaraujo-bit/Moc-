import { useCallback, useRef, useState } from "react";
import { ArrowLeft, Lifebuoy } from "@phosphor-icons/react";
import { Button, TextField } from "../../components/ui/primitives";
import { MasterPasswordForm, type MasterPasswordState } from "../../components/strength/MasterPasswordForm";
import { TileWall } from "../../components/wall/TileWall";
import { api, errorMessage } from "../../lib/ipc";
import { useApp } from "../../state/app";
import { RecoverySheet } from "../onboarding/RecoverySheet";
import s from "./RecoverFlow.module.css";

export function RecoverFlow({ onCancel, needSecretKey }: { onCancel: () => void; needSecretKey: boolean }) {
  const setPhase = useApp((st) => st.setPhase);
  const codeRef = useRef<HTMLInputElement>(null);
  const skRef = useRef<HTMLInputElement>(null);
  const [pw, setPw] = useState<MasterPasswordState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [freshCode, setFreshCode] = useState<string | null>(null);
  const onPw = useCallback((st: MasterPasswordState) => setPw(st), []);

  const submit = async () => {
    if (!pw?.valid) return;
    const code = codeRef.current?.value ?? "";
    if (!code.trim()) {
      setError("Digite o código de recuperação.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const next = await api.recover(code, pw.read(), needSecretKey ? skRef.current?.value : undefined);
      pw.clear();
      if (codeRef.current) codeRef.current.value = "";
      setFreshCode(next);
    } catch (e) {
      setError(errorMessage(e).replace("Senha mestra incorreta.", "Esse código de recuperação não confere."));
    } finally {
      setBusy(false);
    }
  };

  if (freshCode) {
    return (
      <div className={s.root}>
        <TileWall opening={{ cols: 13, rows: 11 }} progress={0.35} />
        <div className={s.panel}>
          <h1 className={s.title}>Pronto. Senha nova no lugar.</h1>
          <p className={s.sub}>
            O código que você usou não vale mais. Este é o novo — guarde-o no lugar do antigo.
          </p>
          <RecoverySheet code={freshCode} onDone={() => setPhase("unlocked")} doneLabel="Abrir meu Mocó" />
        </div>
      </div>
    );
  }

  return (
    <div className={s.root}>
      <TileWall opening={{ cols: 13, rows: 13 }} />
      <div className={s.panel}>
        <Button variant="ghost" size="sm" icon={<ArrowLeft size={15} />} onClick={onCancel} className={s.back}>
          Voltar
        </Button>
        <div className={s.head}>
          <span className={s.icon}>
            <Lifebuoy size={22} />
          </span>
          <h1 className={s.title}>Recuperar acesso</h1>
          <p className={s.sub}>
            Com o <strong>código de recuperação</strong> que você guardou ao criar o Mocó, dá para definir uma senha mestra
            nova. Sem ele, não existe outro caminho — nem nós conseguimos abrir seu cofre.
          </p>
        </div>
        <div className={s.form}>
          <TextField ref={codeRef} large mono label="Código de recuperação" placeholder="XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX" autoFocus />
          {needSecretKey && (
            <TextField
              ref={skRef}
              large
              mono
              label="Chave Secreta"
              placeholder="M1-XXXXXX-XXXXXX-XXXXXX-XXXXXX-XXXX"
              hint="Está no seu Kit de Emergência."
            />
          )}
          <MasterPasswordForm onChange={onPw} onSubmit={submit} labels={{ password: "Nova senha mestra", confirm: "Confirme a nova senha" }} />
          {error && (
            <p className={s.error} role="alert">
              {error}
            </p>
          )}
          <Button variant="primary" size="lg" full loading={busy} disabled={!pw?.valid} onClick={submit}>
            Definir nova senha
          </Button>
        </div>
      </div>
    </div>
  );
}
