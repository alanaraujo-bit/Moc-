import type { Strength } from "../../lib/types";
import s from "./StrengthMeter.module.css";

const TONES = ["danger", "danger", "warn", "ok", "ok"] as const;

/** Four tiles that fill as the password gets stronger. Label text carries the meaning,
 *  so color is never the only signal. */
export function StrengthMeter({ strength, empty, compact }: { strength: Strength | null; empty?: boolean; compact?: boolean }) {
  const score = empty || !strength ? -1 : strength.score;
  const tone = score < 0 ? "none" : TONES[score];
  return (
    <div className={s.root} data-compact={compact || undefined}>
      <div className={s.tiles} data-tone={tone} aria-hidden="true">
        {[0, 1, 2, 3].map((i) => (
          <span key={i} className={s.tile} data-on={score >= 0 && i < Math.max(1, score)} />
        ))}
      </div>
      {!compact && (
        <span className={s.label} aria-live="polite">
          {score < 0 ? "Força da senha" : strength!.label}
          {score >= 0 && strength!.crackTime && <span className={s.time}> · quebraria em {strength!.crackTime}</span>}
        </span>
      )}
    </div>
  );
}
