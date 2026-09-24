import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowsClockwise, Copy } from "@phosphor-icons/react";
import { Button, IconButton, Segmented, Switch } from "../ui/primitives";
import { toast } from "../ui/toast";
import { StrengthMeter } from "../strength/StrengthMeter";
import { PasswordText } from "../../screens/main/fields";
import { api, errorMessage } from "../../lib/ipc";
import type { Generated, Recipe } from "../../lib/types";
import s from "./GeneratorPanel.module.css";

type Mode = Recipe["mode"];

export interface GeneratorPrefs {
  mode: Mode;
  length: number;
  words: number;
  pinLength: number;
  symbols: boolean;
  digits: boolean;
  uppercase: boolean;
  avoidAmbiguous: boolean;
  separator: string;
  capitalize: boolean;
  includeNumber: boolean;
  language: "pt" | "en";
}

const DEFAULTS: GeneratorPrefs = {
  mode: "characters",
  length: 20,
  words: 5,
  pinLength: 6,
  symbols: true,
  digits: true,
  uppercase: true,
  avoidAmbiguous: true,
  separator: "-",
  capitalize: true,
  includeNumber: true,
  language: "pt",
};

const KEY = "moco.generator.prefs";

function loadPrefs(): GeneratorPrefs {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(KEY) ?? "{}") };
  } catch {
    return DEFAULTS;
  }
}

function recipeOf(p: GeneratorPrefs): Recipe {
  if (p.mode === "passphrase")
    return { mode: "passphrase", words: p.words, separator: p.separator, capitalize: p.capitalize, includeNumber: p.includeNumber, language: p.language };
  if (p.mode === "pin") return { mode: "pin", length: p.pinLength };
  return {
    mode: "characters",
    length: p.length,
    lowercase: true,
    uppercase: p.uppercase,
    digits: p.digits,
    symbols: p.symbols,
    avoidAmbiguous: p.avoidAmbiguous,
    exclude: "",
  };
}

function verdict(g: Generated): string {
  if (g.entropyBits >= 100) return "Senha forte. Até demais.";
  if (g.entropyBits >= 75) return "Forte de verdade.";
  if (g.entropyBits >= 50) return "Boa para a maioria dos sites.";
  if (g.entropyBits >= 30) return "Serve para PINs e códigos curtos.";
  return "Curta demais para proteger algo importante.";
}

export function GeneratorPanel({
  onUse,
  useLabel = "Usar esta senha",
  big,
  initialMode,
}: {
  onUse?: (value: string) => void;
  useLabel?: string;
  big?: boolean;
  initialMode?: Mode;
}) {
  const [prefs, setPrefs] = useState<GeneratorPrefs>(() => ({ ...loadPrefs(), ...(initialMode ? { mode: initialMode } : {}) }));
  const [gen, setGen] = useState<Generated | null>(null);
  const [spin, setSpin] = useState(0);
  const seq = useRef(0);

  const regenerate = useCallback(async (p: GeneratorPrefs) => {
    const my = ++seq.current;
    try {
      const g = await api.generate(recipeOf(p));
      if (my === seq.current) setGen(g);
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    }
  }, []);

  useEffect(() => {
    void regenerate(prefs);
    try {
      localStorage.setItem(KEY, JSON.stringify(prefs));
    } catch {
      /* preferences are a convenience */
    }
  }, [prefs, regenerate]);

  const set = (patch: Partial<GeneratorPrefs>) => setPrefs((p) => ({ ...p, ...patch }));

  const copy = async () => {
    if (!gen) return;
    try {
      const r = await api.copyText(gen.value, true);
      toast("Senha copiada", { tone: "success", detail: r.clearsIn ? `some em ${r.clearsIn}s` : undefined });
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    }
  };

  return (
    <div className={s.root} data-big={big || undefined}>
      <div className={s.output}>
        <div className={s.value} aria-live="polite">
          {gen ? <PasswordText value={gen.value} /> : " "}
        </div>
        <div className={s.outputActions}>
          <IconButton
            tooltipSide="top"
            label="Gerar outra"
            shortcut={big ? "Ctrl+R" : undefined}
            onClick={() => {
              setSpin((n) => n + 1);
              void regenerate(prefs);
            }}
          >
            <ArrowsClockwise size={17} className={s.spin} style={{ transform: `rotate(${spin * 180}deg)` }} />
          </IconButton>
          <IconButton label="Copiar" onClick={copy} tooltipSide="top">
            <Copy size={17} />
          </IconButton>
        </div>
      </div>
      {gen && (
        <div className={s.quality}>
          <StrengthMeter compact strength={gen.strength} />
          <span>
            {verdict(gen)} <span className={s.bits}>{Math.round(gen.entropyBits)} bits</span>
          </span>
        </div>
      )}

      <Segmented<Mode>
        label="Tipo de senha"
        value={prefs.mode}
        onChange={(mode) => set({ mode })}
        options={[
          { value: "characters", label: "Caracteres" },
          { value: "passphrase", label: "Frase" },
          { value: "pin", label: "PIN" },
        ]}
      />

      {prefs.mode === "characters" && (
        <div className={s.controls}>
          <Slider label="Tamanho" min={8} max={64} value={prefs.length} onChange={(length) => set({ length })} />
          <Toggle label="Letras maiúsculas" checked={prefs.uppercase} onChange={(uppercase) => set({ uppercase })} />
          <Toggle label="Números" checked={prefs.digits} onChange={(digits) => set({ digits })} />
          <Toggle label="Símbolos" checked={prefs.symbols} onChange={(symbols) => set({ symbols })} />
          <Toggle
            label="Evitar caracteres parecidos"
            hint="Sem I, l, 1, O e 0 — bom para digitar olhando."
            checked={prefs.avoidAmbiguous}
            onChange={(avoidAmbiguous) => set({ avoidAmbiguous })}
          />
        </div>
      )}
      {prefs.mode === "passphrase" && (
        <div className={s.controls}>
          <Slider label="Palavras" min={3} max={10} value={prefs.words} onChange={(words) => set({ words })} />
          <div className={s.line}>
            <span>Idioma</span>
            <Segmented<"pt" | "en">
              label="Idioma"
              value={prefs.language}
              onChange={(language) => set({ language })}
              options={[
                { value: "pt", label: "Português" },
                { value: "en", label: "Inglês" },
              ]}
            />
          </div>
          <div className={s.line}>
            <span>Separador</span>
            <Segmented<string>
              label="Separador"
              value={prefs.separator}
              onChange={(separator) => set({ separator })}
              options={[
                { value: "-", label: "-" },
                { value: ".", label: "." },
                { value: " ", label: "espaço" },
                { value: "_", label: "_" },
              ]}
            />
          </div>
          <Toggle label="Iniciais maiúsculas" checked={prefs.capitalize} onChange={(capitalize) => set({ capitalize })} />
          <Toggle label="Incluir um número" checked={prefs.includeNumber} onChange={(includeNumber) => set({ includeNumber })} />
        </div>
      )}
      {prefs.mode === "pin" && (
        <div className={s.controls}>
          <Slider label="Dígitos" min={4} max={12} value={prefs.pinLength} onChange={(pinLength) => set({ pinLength })} />
          <p className={s.note}>Sequências óbvias como 1234 e 0000 nunca saem daqui.</p>
        </div>
      )}

      {onUse && gen && (
        <Button variant="primary" full onClick={() => onUse(gen.value)}>
          {useLabel}
        </Button>
      )}
    </div>
  );
}

function Slider({ label, min, max, value, onChange }: { label: string; min: number; max: number; value: number; onChange: (v: number) => void }) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <label className={s.slider}>
      <span className={s.line}>
        <span>{label}</span>
        <span className={s.sliderValue}>{value}</span>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ "--pct": `${pct}%` } as React.CSSProperties}
      />
    </label>
  );
}

function Toggle({ label, hint, checked, onChange }: { label: string; hint?: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className={s.toggle}>
      <span className={s.toggleText}>
        {label}
        {hint && <small>{hint}</small>}
      </span>
      <Switch checked={checked} onChange={onChange} label={label} />
    </label>
  );
}
