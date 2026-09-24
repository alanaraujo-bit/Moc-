import { memo, type ReactElement } from "react";
import type { Glaze } from "../../lib/templates";
import { tileFor, type TileSpec } from "../../lib/tile";
import type { ItemKind } from "../../lib/types";
import styles from "./Tile.module.css";

// Kind glyphs, drawn in the panel's grammar: arches, quarter arcs, bands. 24-unit grid.
const GLYPHS: Record<ItemKind, ReactElement> = {
  login: (
    <>
      <path d="M7 20.5V11a5 5 0 0 1 10 0v9.5" />
      <circle cx="12" cy="14.6" r="1.25" fill="currentColor" stroke="none" />
    </>
  ),
  password: (
    <>
      <circle cx="6.5" cy="12" r="1.7" fill="currentColor" stroke="none" />
      <circle cx="12" cy="12" r="1.7" fill="currentColor" stroke="none" />
      <circle cx="17.5" cy="12" r="1.7" fill="currentColor" stroke="none" />
    </>
  ),
  card: (
    <>
      <rect x="3" y="5.5" width="18" height="13" rx="2.2" />
      <path d="M3 9.8h18M6.5 15h4" />
    </>
  ),
  identity: (
    <>
      <circle cx="12" cy="8.8" r="3.3" />
      <path d="M5.2 19.5a6.8 6.8 0 0 1 13.6 0" />
    </>
  ),
  document: (
    <>
      <path d="M6.5 3.5h7.3l3.7 3.7v13.3h-11z" />
      <path d="M13.5 3.5v4h4M9 12.5h6M9 16h4" />
    </>
  ),
  note: (
    <>
      <rect x="4.5" y="4" width="15" height="16" rx="2" />
      <path d="M8 8.5h8M8 12h8M8 15.5h5" />
    </>
  ),
  wifi: (
    <>
      <path d="M3.8 10.8a11.7 11.7 0 0 1 16.4 0" />
      <path d="M7 14.1a7.1 7.1 0 0 1 10 0" />
      <circle cx="12" cy="17.8" r="1.35" fill="currentColor" stroke="none" />
    </>
  ),
  license: (
    <>
      <circle cx="12" cy="9.5" r="5.2" />
      <path d="M9.2 13.9 8.4 20.5l3.6-1.9 3.6 1.9-.8-6.6" />
    </>
  ),
  bankAccount: (
    <>
      <path d="M3.5 9 12 4.2 20.5 9z" />
      <path d="M6 11.5v5.5M10 11.5v5.5M14 11.5v5.5M18 11.5v5.5M3.5 19.8h17" />
    </>
  ),
  server: (
    <>
      <rect x="4" y="4.5" width="16" height="6.3" rx="1.6" />
      <rect x="4" y="13.2" width="16" height="6.3" rx="1.6" />
      <circle cx="7.6" cy="7.65" r=".9" fill="currentColor" stroke="none" />
      <circle cx="7.6" cy="16.35" r=".9" fill="currentColor" stroke="none" />
    </>
  ),
  database: (
    <>
      <ellipse cx="12" cy="6.2" rx="7" ry="2.7" />
      <path d="M5 6.2v11.6c0 1.5 3.1 2.7 7 2.7s7-1.2 7-2.7V6.2" />
      <path d="M5 12c0 1.5 3.1 2.7 7 2.7s7-1.2 7-2.7" />
    </>
  ),
  apiCredential: (
    <>
      <path d="M9 4.5c-1.9 0-2.5 1-2.5 2.5v2.3c0 1.3-.8 2.7-2 2.7 1.2 0 2 1.4 2 2.7V17c0 1.5.6 2.5 2.5 2.5" />
      <path d="M15 4.5c1.9 0 2.5 1 2.5 2.5v2.3c0 1.3.8 2.7 2 2.7-1.2 0-2 1.4-2 2.7V17c0 1.5-.6 2.5-2.5 2.5" />
    </>
  ),
  sshKey: (
    <>
      <path d="m5.5 7.5 4.5 4.5-4.5 4.5" />
      <path d="M12.5 17h6" />
    </>
  ),
  cryptoWallet: (
    <>
      <path d="M12 3.2 19.6 7.6v8.8L12 20.8 4.4 16.4V7.6z" />
      <path d="M12 3.2v17.6M4.4 7.6 19.6 16.4" strokeOpacity=".45" />
    </>
  ),
  healthPlan: <path d="M9.6 4.5h4.8v5.1h5.1v4.8h-5.1v5.1H9.6v-5.1H4.5V9.6h5.1z" />,
  vehicle: (
    <>
      <path d="M3.5 16v-3.6l2.1-4.2a2 2 0 0 1 1.8-1.1h9.2a2 2 0 0 1 1.8 1.1l2.1 4.2V16" />
      <path d="M3.5 12.4h17M9.7 16h4.6" />
      <circle cx="7" cy="16.4" r="1.9" />
      <circle cx="17" cy="16.4" r="1.9" />
    </>
  ),
  custom: (
    <>
      <rect x="4" y="4" width="6.8" height="6.8" rx="1.6" />
      <rect x="13.2" y="4" width="6.8" height="6.8" rx="1.6" />
      <rect x="4" y="13.2" width="6.8" height="6.8" rx="1.6" />
      <path d="M16.6 13.4v6.4M13.4 16.6h6.4" />
    </>
  ),
};

export function KindGlyph({ kind, size = 18, className }: { kind: ItemKind; size?: number; className?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.7}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      {GLYPHS[kind]}
    </svg>
  );
}

const ARC_PATHS = [
  "M0 55 A55 55 0 0 1 55 0", // top-left
  "M45 0 A55 55 0 0 1 100 55", // top-right
  "M100 45 A55 55 0 0 1 45 100", // bottom-right
  "M55 100 A55 55 0 0 1 0 45", // bottom-left
];

interface ItemTileProps {
  kind: ItemKind;
  title: string;
  urls?: string[];
  size?: number;
  glaze?: Glaze;
  className?: string;
}

function TileBase({ spec, size, className }: { spec: TileSpec; size: number; className?: string }) {
  const g = spec.glaze;
  return (
    <span
      className={`${styles.tile} ${className ?? ""}`}
      style={
        {
          width: size,
          height: size,
          "--t-ground": `var(--glaze-${g}-g)`,
          "--t-ink": `var(--glaze-${g})`,
          borderRadius: Math.round(size * 0.24),
        } as React.CSSProperties
      }
      aria-hidden="true"
    >
      <svg className={styles.arc} viewBox="0 0 100 100" preserveAspectRatio="none">
        <path d={ARC_PATHS[spec.corner]} />
      </svg>
      {spec.monogram ? (
        <span className={styles.monogram} style={{ fontSize: Math.round(size * 0.46) }}>
          {spec.monogram}
        </span>
      ) : (
        <KindGlyph kind={spec.kind} size={Math.round(size * 0.56)} className={styles.glyph} />
      )}
    </span>
  );
}

export const ItemTile = memo(function ItemTile({ kind, title, urls, size = 36, glaze, className }: ItemTileProps) {
  const spec = tileFor(kind, title, urls);
  if (glaze) spec.glaze = glaze;
  return <TileBase spec={spec} size={size} className={className} />;
});

/** A kind tile without an item (pickers, empty states). */
export function KindTile({ kind, size = 36, glaze }: { kind: ItemKind; size?: number; glaze?: Glaze }) {
  const spec = tileFor(kind, "", []);
  spec.monogram = null;
  if (glaze) spec.glaze = glaze;
  return <TileBase spec={spec} size={size} />;
}
