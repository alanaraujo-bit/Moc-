import { useEffect, useRef, useState } from "react";
import { ArrowSquareOut, Copy, Eye, EyeSlash } from "@phosphor-icons/react";
import { StrengthMeter } from "../../components/strength/StrengthMeter";
import { IconButton } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { displayValue } from "../../lib/format";
import { api, errorMessage } from "../../lib/ipc";
import type { FieldView, TotpCode, Uuid } from "../../lib/types";
import s from "./fields.module.css";

export async function copyField(itemId: Uuid, field: string, what: string) {
  try {
    const r = await api.copyField(itemId, field);
    toast(`${what} copiado`, {
      tone: "success",
      detail: r.clearsIn ? `some da área de transferência em ${r.clearsIn}s` : undefined,
    });
  } catch (e) {
    toast(errorMessage(e), { tone: "danger" });
  }
}

/** Password with character classes colored — digits and symbols easy to tell apart. */
export function PasswordText({ value, className }: { value: string; className?: string }) {
  return (
    <span className={`${s.pw} ${className ?? ""}`}>
      {Array.from(value).map((ch, i) => (
        <span key={i} data-c={/[0-9]/.test(ch) ? "d" : /[A-Za-zÀ-ÿ]/.test(ch) ? undefined : "s"}>
          {ch}
        </span>
      ))}
    </span>
  );
}

const REVEAL_MS = 60_000;

export function FieldRow({
  itemId,
  field,
  label,
  prominent,
  revision,
}: {
  itemId: Uuid;
  field: FieldView;
  label: string;
  prominent?: boolean;
  revision: number;
}) {
  const concealed = field.value === null;
  const [revealed, setRevealed] = useState<string | null>(null);
  const hideTimer = useRef<number>(0);

  useEffect(() => {
    setRevealed(null);
    return () => window.clearTimeout(hideTimer.current);
  }, [itemId, field.id, revision]);

  const toggle = async () => {
    if (revealed !== null) {
      setRevealed(null);
      return;
    }
    try {
      const v = await api.reveal(itemId, field.id);
      setRevealed(v);
      window.clearTimeout(hideTimer.current);
      hideTimer.current = window.setTimeout(() => setRevealed(null), REVEAL_MS);
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    }
  };

  if (field.type === "totp") return <TotpRow itemId={itemId} field={field} label={label} revision={revision} />;

  const isUrl = field.type === "url";
  const multiline = field.type === "multiline" || field.type === "sshPrivateKey" || field.type === "seedPhrase";
  const shown = concealed ? revealed : field.value;
  const monoConcealed = concealed && (field.type === "concealed" || field.type === "pin" || field.type === "cardNumber");

  return (
    <div className={s.row} data-prominent={prominent || undefined}>
      <span className={s.label}>{label}</span>
      <div className={s.valueLine}>
        <button
          className={s.value}
          onClick={() => copyField(itemId, field.id, label)}
          title="Clique para copiar"
          data-multiline={multiline || undefined}
        >
          {shown === null ? (
            <span className={s.dots} aria-label="Oculto">
              {"•".repeat(field.type === "pin" ? 4 : field.type === "cardNumber" ? 16 : 12)}
            </span>
          ) : monoConcealed ? (
            <PasswordText value={displayValue(field.type, shown)} />
          ) : (
            <span className={`${multiline && concealed ? "mono" : ""} ${s.text}`}>{displayValue(field.type, shown)}</span>
          )}
        </button>
        <span className={s.actions}>
          {concealed && (
            <IconButton small label={revealed !== null ? "Ocultar" : "Mostrar"} onClick={toggle}>
              {revealed !== null ? <EyeSlash size={15} /> : <Eye size={15} />}
            </IconButton>
          )}
          {isUrl && field.value && (
            <IconButton small label="Abrir no navegador" onClick={() => api.openUrl(field.value!)}>
              <ArrowSquareOut size={15} />
            </IconButton>
          )}
          <IconButton small label="Copiar" onClick={() => copyField(itemId, field.id, label)}>
            <Copy size={15} />
          </IconButton>
        </span>
      </div>
      {field.strength !== null && field.type === "concealed" && (
        <div className={s.strength}>
          <StrengthMeter
            compact
            strength={{ score: field.strength, label: "", guessesLog10: 0, crackTime: "", warning: null, suggestions: [] }}
          />
          <span>{["Muito fraca", "Fraca", "Razoável", "Forte", "Muito forte"][field.strength]}</span>
        </div>
      )}
    </div>
  );
}

function TotpRow({ itemId, field, label, revision }: { itemId: Uuid; field: FieldView; label: string; revision: number }) {
  const [code, setCode] = useState<TotpCode | null>(null);
  const [now, setNow] = useState(Date.now());
  const fetchedAt = useRef(0);

  useEffect(() => {
    let alive = true;
    let timer = 0;
    const load = async () => {
      try {
        const c = await api.totp(itemId, field.id);
        if (!alive) return;
        setCode(c);
        fetchedAt.current = Date.now();
        timer = window.setTimeout(load, c.remaining * 1000 + 50);
      } catch {
        if (alive) setCode(null);
      }
    };
    void load();
    const tick = window.setInterval(() => setNow(Date.now()), 250);
    return () => {
      alive = false;
      window.clearTimeout(timer);
      window.clearInterval(tick);
    };
  }, [itemId, field.id, revision]);

  if (!code) {
    return (
      <div className={s.row}>
        <span className={s.label}>{label}</span>
        <span className={s.invalid}>Chave de 2FA inválida. Edite o item para corrigir.</span>
      </div>
    );
  }
  const left = Math.max(0, code.remaining - (now - fetchedAt.current) / 1000);
  const frac = left / code.period;
  const r = 7;
  const c = 2 * Math.PI * r;
  const half = Math.ceil(code.code.length / 2);
  return (
    <div className={s.row}>
      <span className={s.label}>{label}</span>
      <div className={s.valueLine}>
        <button className={s.value} onClick={() => copyField(itemId, field.id, "Código")} title="Clique para copiar">
          <span className={s.totp} data-ending={left < 6 || undefined}>
            {code.code.slice(0, half)}
            <span className={s.totpGap} />
            {code.code.slice(half)}
          </span>
          <svg className={s.ring} viewBox="0 0 18 18" aria-label={`${Math.ceil(left)} segundos`}>
            <circle cx="9" cy="9" r={r} className={s.ringTrack} />
            <circle
              cx="9"
              cy="9"
              r={r}
              className={s.ringFill}
              data-ending={left < 6 || undefined}
              strokeDasharray={c}
              strokeDashoffset={c * (1 - frac)}
              transform="rotate(-90 9 9)"
            />
          </svg>
          <span className={s.secs}>{Math.ceil(left)}s</span>
        </button>
        <span className={s.actions}>
          <IconButton small label="Copiar código" onClick={() => copyField(itemId, field.id, "Código")}>
            <Copy size={15} />
          </IconButton>
        </span>
      </div>
    </div>
  );
}
