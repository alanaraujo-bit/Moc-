import { useCallback, useEffect, useState } from "react";
import { ArrowRight, Copy, EyeSlash, Printer, ShieldCheck, Key } from "@phosphor-icons/react";
import { MocoMark, Wordmark } from "../../components/brand/Brand";
import { KindTile } from "../../components/tile/Tile";
import { MasterPasswordForm, type MasterPasswordState } from "../../components/strength/MasterPasswordForm";
import { Button, Spinner } from "../../components/ui/primitives";
import { Confirm } from "../../components/ui/overlays";
import { toast } from "../../components/ui/toast";
import { TileWall } from "../../components/wall/TileWall";
import { api, errorMessage } from "../../lib/ipc";
import type { CreatedAccount, ItemKind } from "../../lib/types";
import { useApp } from "../../state/app";
import { useVault } from "../../state/vault";
import { EmergencyKitSheet, PrintPortal } from "./Printable";
import { SignIn } from "./SignIn";
import { deviceNoun, isMobile } from "../../lib/platform";
import { CodeBlock, printNow, RecoverySheet } from "./RecoverySheet";
import s from "./Onboarding.module.css";

type Step = "welcome" | "signin" | "password" | "creating" | "kit" | "recovery" | "done";

const PROGRESS: Record<Step, number> = { welcome: 0.04, signin: 0.3, password: 0.18, creating: 0.34, kit: 0.52, recovery: 0.72, done: 1 };

export function Onboarding() {
  const setPhase = useApp((st) => st.setPhase);
  const [step, setStep] = useState<Step>("welcome");
  const [pw, setPw] = useState<MasterPasswordState | null>(null);
  const [created, setCreated] = useState<CreatedAccount | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [kitSaved, setKitSaved] = useState(false);
  const [printing, setPrinting] = useState(false);
  const [skipRecovery, setSkipRecovery] = useState(false);
  const onPw = useCallback((st: MasterPasswordState) => setPw(st), []);

  const create = async () => {
    if (!pw?.valid) return;
    const password = pw.read();
    setStep("creating");
    setError(null);
    const started = Date.now();
    try {
      const acct = await api.createAccount(password);
      pw.clear();
      // Give the moment a beat; key derivation is fast but the step deserves a breath.
      const wait = Math.max(0, 900 - (Date.now() - started));
      window.setTimeout(() => {
        setCreated(acct);
        setStep("kit");
      }, wait);
    } catch (e) {
      setError(errorMessage(e));
      setStep("password");
    }
  };

  const finish = async (kind?: ItemKind) => {
    setPhase("unlocked");
    if (kind) {
      await useVault.getState().load();
      const vault = useVault.getState().vaults[0];
      if (vault) useVault.getState().edit({ mode: "new", kind, vaultId: vault.id });
    }
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (step === "welcome" && e.key === "Enter") setStep("password");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [step]);

  return (
    <div className={s.root}>
      <section className={s.content}>
        <header className={s.brand}>
          <MocoMark size={26} />
          <Wordmark height={19} />
        </header>

        <div className={s.stepWrap} key={step}>
          {step === "welcome" && (
            <div className={s.step}>
              <h1 className={s.hero}>
                Pode esquecer.
                <br />O Mocó lembra.
              </h1>
              <p className={s.lede}>
                Um cantinho seguro para suas senhas, documentos, cartões e tudo que importa — trancado com uma chave que só você
                tem.
              </p>
              <ul className={s.points}>
                <li>
                  <ShieldCheck size={20} />
                  <span>
                    <strong>Só você vê o que guarda.</strong> Tudo é cifrado aqui no seu {deviceNoun}, antes de ir para qualquer lugar.
                  </span>
                </li>
                <li>
                  <Key size={20} />
                  <span>
                    <strong>Uma senha para lembrar.</strong> A senha mestra abre o Mocó; ele cuida de todas as outras.
                  </span>
                </li>
                <li>
                  <EyeSlash size={20} />
                  <span>
                    <strong>Nem nós conseguimos abrir.</strong> Por isso, vamos te ajudar a guardar um plano B.
                  </span>
                </li>
              </ul>
              {isMobile ? (
                // On a phone most people already have Mocó on their computer.
                <div className={s.row}>
                  <Button variant="primary" size="lg" onClick={() => setStep("signin")} icon={null}>
                    Já uso o Mocó <ArrowRight size={16} weight="bold" />
                  </Button>
                  <Button size="lg" onClick={() => setStep("password")}>
                    Criar meu Mocó
                  </Button>
                </div>
              ) : (
                <>
                  <div className={s.row}>
                    <Button variant="primary" size="lg" onClick={() => setStep("password")} icon={null}>
                      Criar meu Mocó <ArrowRight size={16} weight="bold" />
                    </Button>
                    <span className={s.muted}>Leva uns dois minutos.</span>
                  </div>
                  <button className={s.linkBtn} onClick={() => setStep("signin")}>
                    Já uso o Mocó em outro computador
                  </button>
                </>
              )}
            </div>
          )}

          {step === "signin" && <SignIn onBack={() => setStep("welcome")} />}

          {step === "password" && (
            <div className={s.step}>
              <h1 className={s.title}>Crie sua senha mestra</h1>
              <p className={s.lede}>
                É a única senha que você vai precisar decorar. Ela nunca sai deste {deviceNoun} e nós não temos cópia — então escolha
                uma que você não vá esquecer.
              </p>
              <MasterPasswordForm onChange={onPw} onSubmit={create} autoFocus />
              {error && (
                <p className={s.error} role="alert">
                  {error}
                </p>
              )}
              <div className={s.row}>
                <Button variant="primary" size="lg" disabled={!pw?.valid} onClick={create}>
                  Continuar
                </Button>
                <Button variant="ghost" onClick={() => setStep("welcome")}>
                  Voltar
                </Button>
              </div>
            </div>
          )}

          {step === "creating" && (
            <div className={`${s.step} ${s.center}`}>
              <Spinner size={22} />
              <h1 className={s.title}>Preparando seu Mocó…</h1>
              <p className={s.lede}>Gerando suas chaves com segurança. É rapidinho.</p>
            </div>
          )}

          {step === "kit" && created && (
            <div className={s.step}>
              <h1 className={s.title}>Guarde seu Kit de Emergência</h1>
              <p className={s.lede}>
                Esta é a sua <strong>Chave Secreta</strong>. Ela fica guardada neste {deviceNoun} e você não precisa digitá-la no dia a
                dia — mas, para abrir o Mocó em outro aparelho, vai precisar dela e da senha mestra.
              </p>
              <CodeBlock value={created.secretKey} label="Chave Secreta" />
              <div className={s.actions}>
                <Button variant="primary" icon={<Printer size={16} />} onClick={() => printNow(setPrinting)}>
                  Imprimir ou salvar em PDF
                </Button>
                <Button
                  variant="ghost"
                  icon={<Copy size={16} />}
                  onClick={async () => {
                    const r = await api.copyText(created.secretKey, true);
                    toast("Chave Secreta copiada", { countdown: r.clearsIn || undefined, detail: r.clearsIn ? `some em ${r.clearsIn}s` : undefined });
                  }}
                >
                  Copiar
                </Button>
              </div>
              <label className={s.check}>
                <input type="checkbox" checked={kitSaved} onChange={(e) => setKitSaved(e.target.checked)} />
                <span>Imprimi ou salvei o Kit num lugar seguro, fora deste {deviceNoun}.</span>
              </label>
              <Button variant="primary" size="lg" disabled={!kitSaved} onClick={() => setStep("recovery")} className={s.fit}>
                Continuar
              </Button>
            </div>
          )}

          {step === "recovery" && created && (
            <div className={s.step}>
              <h1 className={s.title}>E se um dia você esquecer?</h1>
              <p className={s.lede}>
                Este <strong>Código de Recuperação</strong> cria uma senha mestra nova se você esquecer a atual. Guarde longe do Kit:
                assim, perder um papel não compromete o outro.
              </p>
              <RecoverySheet code={created.recoveryCode} onDone={() => setStep("done")} />
              <button className={s.linkBtn} onClick={() => setSkipRecovery(true)}>
                Prefiro não ter código de recuperação
              </button>
            </div>
          )}

          {step === "done" && (
            <div className={s.step}>
              <h1 className={s.hero}>Tá guardado.</h1>
              <p className={s.lede}>Seu Mocó está pronto. Por onde quer começar?</p>
              <div className={s.starters}>
                {(
                  [
                    ["login", "Um login", "site ou app"],
                    ["card", "Um cartão", "crédito ou débito"],
                    ["wifi", "O Wi-Fi de casa", "com QR code para visitas"],
                    ["document", "Um documento", "RG, CNH, passaporte"],
                  ] as [ItemKind, string, string][]
                ).map(([kind, label, sub]) => (
                  <button key={kind} className={s.starter} onClick={() => finish(kind)}>
                    <KindTile kind={kind} size={40} />
                    <span>
                      <strong>{label}</strong>
                      <span>{sub}</span>
                    </span>
                  </button>
                ))}
              </div>
              <Button variant="ghost" onClick={() => finish()} className={s.fitGhost}>
                Ir para o meu Mocó
              </Button>
            </div>
          )}
        </div>

        <footer className={s.progress} aria-hidden={step === "welcome"}>
          {["password", "kit", "recovery"].includes(step) && (
            <>
              <span className={s.stepLabel}>Passo {step === "password" ? 1 : step === "kit" ? 2 : 3} de 3</span>
              {["password", "kit", "recovery"].map((st, i) => (
                <span key={st} className={s.dot} data-on={i + 1 <= (step === "password" ? 1 : step === "kit" ? 2 : 3)} />
              ))}
            </>
          )}
        </footer>
      </section>

      <aside className={s.wall}>
        <TileWall tile={64} progress={PROGRESS[step]} seed={23} />
      </aside>

      {printing && created && (
        <PrintPortal>
          <EmergencyKitSheet secretKey={created.secretKey} />
        </PrintPortal>
      )}

      <Confirm
        open={skipRecovery}
        onOpenChange={setSkipRecovery}
        title="Seguir sem código de recuperação?"
        description={
          <>
            Sem o código, esquecer a senha mestra significa <strong>perder tudo o que está no Mocó</strong>. Não há outro jeito de
            recuperar — nem por nós. Você pode criar um código depois, nas configurações.
          </>
        }
        confirmLabel="Seguir sem código"
        danger
        onConfirm={async () => {
          setSkipRecovery(false);
          try {
            await api.disableRecovery();
          } catch {
            /* surfaced in settings */
          }
          setStep("done");
        }}
      />
    </div>
  );
}
