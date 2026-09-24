import type { ItemView } from "../../lib/types";
import s from "./CardFace.module.css";

const BRAND_GLAZE: Record<string, string> = {
  Visa: "cobalt",
  Mastercard: "clay",
  Elo: "ink",
  "American Express": "sky",
  Hipercard: "clay",
  "Diners Club": "slate",
  Discover: "ochre",
  JCB: "moss",
};

/** The card as an object: issuer, brand, last digits, name and expiry. */
export function CardFace({ item }: { item: ItemView }) {
  const v = (id: string) => item.fields.find((f) => f.id === id)?.value ?? "";
  const [brandPart, lastPart] = item.subtitle.includes("•") ? item.subtitle.split(" · ") : [item.subtitle, ""];
  const brand = lastPart ? brandPart : "";
  const last4 = (lastPart || item.subtitle).replace(/\D/g, "").slice(-4);
  const glaze = BRAND_GLAZE[brand] ?? "slate";
  return (
    <div className={s.card} style={{ "--g": `var(--glaze-${glaze})` } as React.CSSProperties} aria-hidden="true">
      <svg className={s.arcs} viewBox="0 0 340 214" preserveAspectRatio="xMaxYMid slice">
        <path d="M340 0 A150 150 0 0 1 190 150 L190 214 L340 214 Z" />
        <path d="M340 80 A70 70 0 0 1 270 150" fill="none" strokeWidth="14" />
      </svg>
      <div className={s.top}>
        <span className={s.issuer}>{v("issuer") || item.title}</span>
        <span className={s.brand}>{brand}</span>
      </div>
      <div className={s.chip} />
      <div className={s.number}>
        <span>••••</span>
        <span>••••</span>
        <span>••••</span>
        <span>{last4 || "••••"}</span>
      </div>
      <div className={s.bottom}>
        <span className={s.name}>{v("cardholder") || " "}</span>
        {v("expiry") && (
          <span className={s.exp}>
            <small>válido até</small>
            {v("expiry")}
          </span>
        )}
      </div>
    </div>
  );
}
