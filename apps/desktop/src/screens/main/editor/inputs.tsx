import { useEffect, useRef, useState } from "react";
import { Popover } from "radix-ui";
import { MagicWand } from "@phosphor-icons/react";
import { GeneratorPanel } from "../../../components/generator/GeneratorPanel";
import { StrengthMeter } from "../../../components/strength/StrengthMeter";
import { Button, PasswordField, TextArea, TextField } from "../../../components/ui/primitives";
import {
  formatCardNumber,
  formatCep,
  formatCnpj,
  formatCpf,
  isValidCnpj,
  isValidCpf,
  isValidLuhn,
} from "../../../lib/format";
import { api } from "../../../lib/ipc";
import type { FieldTemplate } from "../../../lib/templates";
import type { FieldType, Strength, TotpCode } from "../../../lib/types";
import s from "./Editor.module.css";

function maskMonthYear(v: string) {
  const d = v.replace(/\D/g, "").slice(0, 4);
  return d.length > 2 ? `${d.slice(0, 2)}/${d.slice(2)}` : d;
}

function maskPhone(v: string) {
  const d = v.replace(/\D/g, "");
  if (d.startsWith("0800")) return d.replace(/^(\d{4})(\d{3})(\d{0,4}).*/, "$1 $2 $3").trim();
  const x = d.slice(0, 11);
  if (x.length <= 2) return x;
  if (x.length <= 6) return `(${x.slice(0, 2)}) ${x.slice(2)}`;
  if (x.length <= 10) return `(${x.slice(0, 2)}) ${x.slice(2, 6)}-${x.slice(6)}`;
  return `(${x.slice(0, 2)}) ${x.slice(2, 7)}-${x.slice(7)}`;
}

const MASKS: Partial<Record<FieldType, (v: string) => string>> = {
  cardNumber: formatCardNumber,
  cpf: formatCpf,
  cnpj: formatCnpj,
  cep: formatCep,
  monthYear: maskMonthYear,
  phone: maskPhone,
};

function validation(type: FieldType, v: string): string | undefined {
  if (!v) return undefined;
  const d = v.replace(/\D/g, "");
  if (type === "cpf" && d.length === 11 && !isValidCpf(d)) return "Esse CPF não parece válido. Confira os números.";
  if (type === "cnpj" && d.length === 14 && !isValidCnpj(d)) return "Esse CNPJ não parece válido.";
  if (type === "cardNumber" && d.length >= 13 && !isValidLuhn(d)) return "Esse número de cartão não parece válido.";
  if (type === "email" && v.length > 3 && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(v)) return "Confira o e-mail.";
  if (type === "monthYear" && d.length === 4 && (Number(d.slice(0, 2)) < 1 || Number(d.slice(0, 2)) > 12)) return "Mês inválido.";
  return undefined;
}

function Generate({ onUse, mode }: { onUse: (v: string) => void; mode?: "characters" | "pin" }) {
  const [open, setOpen] = useState(false);
  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <Button size="sm" variant="ghost" icon={<MagicWand size={15} />} tabIndex={-1}>
          Gerar
        </Button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content className={s.genPop} side="bottom" align="end" sideOffset={8} collisionPadding={12}>
          <GeneratorPanel
            initialMode={mode}
            onUse={(v) => {
              onUse(v);
              setOpen(false);
            }}
          />
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

function SecretInput({
  label,
  value,
  onChange,
  generate,
  pin,
  context,
  placeholder,
  hint,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  generate?: boolean;
  pin?: boolean;
  context: string[];
  placeholder?: string;
  hint?: string;
}) {
  const [strength, setStrength] = useState<Strength | null>(null);
  const timer = useRef<number>(0);
  useEffect(() => {
    window.clearTimeout(timer.current);
    if (!value || pin) {
      setStrength(null);
      return;
    }
    timer.current = window.setTimeout(() => {
      api.strength(value, context).then(setStrength).catch(() => setStrength(null));
    }, 150);
    return () => window.clearTimeout(timer.current);
  }, [value, pin, context]);

  return (
    <div className={s.secret}>
      <PasswordField
        label={label}
        value={value}
        onChange={(e) => onChange(pin ? e.target.value.replace(/\D/g, "") : e.target.value)}
        inputMode={pin ? "numeric" : undefined}
        placeholder={placeholder}
        hint={hint}
        trailing={generate ? <Generate onUse={onChange} mode={pin ? "pin" : undefined} /> : undefined}
      />
      {generate && value && <StrengthMeter strength={strength} />}
    </div>
  );
}

function TotpInput({ label, value, onChange, hint, placeholder }: { label: string; value: string; onChange: (v: string) => void; hint?: string; placeholder?: string }) {
  const [preview, setPreview] = useState<TotpCode | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!value.trim()) {
      setPreview(null);
      setError(null);
      return;
    }
    const t = window.setTimeout(() => {
      api
        .totpPreview(value)
        .then((c) => {
          setPreview(c);
          setError(null);
        })
        .catch((e) => {
          setPreview(null);
          setError(e?.message ?? "Chave inválida.");
        });
    }, 200);
    return () => window.clearTimeout(t);
  }, [value]);
  return (
    <TextField
      label={label}
      value={value}
      mono
      onChange={(e) => onChange(e.target.value.trim())}
      placeholder={placeholder}
      error={error ?? undefined}
      hint={preview ? `Funcionando: código atual ${preview.code.slice(0, 3)} ${preview.code.slice(3)}${preview.issuer ? ` · ${preview.issuer}` : ""}` : hint}
    />
  );
}

export function FieldInput({
  tpl,
  value,
  onChange,
  context,
}: {
  tpl: FieldTemplate;
  value: string;
  onChange: (v: string) => void;
  context: string[];
}) {
  const t = tpl.type;
  if (t === "concealed" || t === "pin") {
    return (
      <SecretInput
        label={tpl.label}
        value={value}
        onChange={onChange}
        generate={tpl.generate}
        pin={t === "pin"}
        context={context}
        placeholder={tpl.placeholder}
        hint={tpl.hint}
      />
    );
  }
  if (t === "totp") return <TotpInput label={tpl.label} value={value} onChange={onChange} hint={tpl.hint} placeholder={tpl.placeholder} />;
  if (t === "multiline" || t === "sshPrivateKey" || t === "seedPhrase") {
    return (
      <TextArea
        label={tpl.label}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        mono={t !== "multiline"}
        placeholder={tpl.placeholder}
        hint={tpl.hint}
        rows={t === "multiline" ? 3 : 5}
      />
    );
  }
  if (t === "select" && tpl.options) {
    const id = `sel-${tpl.id}`;
    return (
      <div className={s.selectField}>
        <label htmlFor={id} className={s.selectLabel}>
          {tpl.label}
        </label>
        <select id={id} className={s.select} value={value} onChange={(e) => onChange(e.target.value)}>
          <option value="">—</option>
          {tpl.options.map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
          {value && !tpl.options.includes(value) && <option value={value}>{value}</option>}
        </select>
      </div>
    );
  }
  const mask = MASKS[t];
  return (
    <TextField
      label={tpl.label}
      value={value}
      type={t === "date" ? "date" : t === "email" ? "email" : "text"}
      inputMode={t === "number" || t === "cpf" || t === "cnpj" || t === "cep" || t === "cardNumber" ? "numeric" : t === "phone" ? "tel" : undefined}
      mono={t === "cardNumber"}
      placeholder={tpl.placeholder}
      hint={tpl.hint}
      error={validation(t, value)}
      onChange={(e) => onChange(mask ? mask(e.target.value) : e.target.value)}
    />
  );
}
