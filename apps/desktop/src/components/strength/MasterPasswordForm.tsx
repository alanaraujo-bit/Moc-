import { useEffect, useRef, useState } from "react";
import { MagicWand } from "@phosphor-icons/react";
import { Button, PasswordField } from "../ui/primitives";
import { api } from "../../lib/ipc";
import type { Strength } from "../../lib/types";
import { StrengthMeter } from "./StrengthMeter";
import s from "./MasterPasswordForm.module.css";

export const MIN_MASTER = 10;

export interface MasterPasswordState {
  valid: boolean;
  /** Read at submit time; never mirrored into React state. */
  read: () => string;
  clear: () => void;
}

/**
 * New master password + confirmation, with live strength. The inputs are uncontrolled:
 * the value is only read when the caller submits, then cleared.
 */
export function MasterPasswordForm({
  onChange,
  onSubmit,
  autoFocus,
  labels = { password: "Senha mestra", confirm: "Confirme a senha mestra" },
}: {
  onChange: (st: MasterPasswordState) => void;
  onSubmit?: () => void;
  autoFocus?: boolean;
  labels?: { password: string; confirm: string };
}) {
  const pw = useRef<HTMLInputElement>(null);
  const confirm = useRef<HTMLInputElement>(null);
  const [strength, setStrength] = useState<Strength | null>(null);
  const [len, setLen] = useState(0);
  const [mismatch, setMismatch] = useState(false);
  const [confirmTouched, setConfirmTouched] = useState(false);
  const timer = useRef<number>(0);

  const evaluate = () => {
    const v = pw.current?.value ?? "";
    const c = confirm.current?.value ?? "";
    setLen(v.length);
    const mm = c.length > 0 && c !== v;
    setMismatch(mm);
    window.clearTimeout(timer.current);
    if (!v) {
      setStrength(null);
    } else {
      timer.current = window.setTimeout(async () => {
        try {
          setStrength(await api.strength(v, ["moco", "mocó"]));
        } catch {
          setStrength(null);
        }
      }, 120);
    }
  };

  const score = strength?.score ?? 0;
  const valid = len >= MIN_MASTER && score >= 2 && !mismatch && (confirm.current?.value ?? "") === (pw.current?.value ?? "") && len > 0;

  useEffect(() => {
    onChange({
      valid,
      read: () => pw.current?.value ?? "",
      clear: () => {
        if (pw.current) pw.current.value = "";
        if (confirm.current) confirm.current.value = "";
      },
    });
  }, [valid, onChange]);

  const suggest = async () => {
    const g = await api.generate({ mode: "passphrase", words: 4, separator: " ", capitalize: false, includeNumber: true, language: "pt" });
    if (pw.current && confirm.current) {
      pw.current.value = g.value;
      confirm.current.value = "";
      pw.current.type = "text";
      evaluate();
      confirm.current.focus();
    }
  };

  let hint: string;
  if (len === 0) hint = "Uma frase que só você saiba funciona melhor que uma palavra cheia de símbolos.";
  else if (len < MIN_MASTER) hint = `Mais ${MIN_MASTER - len} ${MIN_MASTER - len === 1 ? "caractere" : "caracteres"}, pelo menos.`;
  else if (score < 2) hint = strength?.warning ?? "Ainda fácil de adivinhar. Tente juntar palavras que não combinam.";
  else if (score < 3) hint = "Dá pra usar. Uma ou duas palavras a mais deixam bem melhor.";
  else hint = "Ótima. Agora é só não esquecer.";

  return (
    <form
      className={s.root}
      onSubmit={(e) => {
        e.preventDefault();
        if (valid) onSubmit?.();
      }}
    >
      <div className={s.row}>
        <PasswordField
          ref={pw}
          large
          label={labels.password}
          autoFocus={autoFocus}
          onInput={evaluate}
          hint={hint}
          trailing={
            <Button size="sm" variant="ghost" icon={<MagicWand size={15} />} onClick={suggest} tabIndex={-1}>
              Sugerir
            </Button>
          }
        />
        <StrengthMeter strength={strength} empty={len === 0} />
      </div>
      <PasswordField
        ref={confirm}
        large
        label={labels.confirm}
        onInput={() => {
          setConfirmTouched(true);
          evaluate();
        }}
        onBlur={() => setConfirmTouched(true)}
        error={confirmTouched && mismatch ? "As duas senhas não estão iguais." : undefined}
      />
      <button type="submit" hidden />
    </form>
  );
}
