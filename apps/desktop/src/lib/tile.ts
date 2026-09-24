// Every item gets its own azulejo: a deterministic tile derived from its identity.
// No favicon fetching needed (privacy), yet each item is recognizable at a glance.

import type { Glaze } from "./templates";
import { template } from "./templates";
import type { ItemKind } from "./types";

export const GLAZES: Glaze[] = ["cobalt", "sky", "moss", "ochre", "clay", "plum", "slate", "ink"];

export function hash(s: string): number {
  // FNV-1a 32-bit
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

export function hostOf(url: string): string {
  const s = url.trim().replace(/^[a-z][a-z0-9+.-]*:\/\//i, "");
  return s.split(/[/?#]/)[0].replace(/^www\./i, "").toLowerCase();
}

/** The "brand" part of a host: "app.nubank.com.br" → "nubank". */
export function brandOf(host: string): string {
  const parts = host.split(".").filter(Boolean);
  if (parts.length <= 1) return parts[0] ?? "";
  const secondLevel = new Set(["com", "net", "org", "gov", "edu", "co", "app", "blog"]);
  let i = parts.length - 2;
  if (parts.length >= 3 && secondLevel.has(parts[i]) && parts[parts.length - 1].length === 2) i -= 1;
  return parts[Math.max(0, i)];
}

export interface TileSpec {
  glaze: Glaze;
  /** Which corner the quarter-arc signature sits in (0 top-left, clockwise). */
  corner: 0 | 1 | 2 | 3;
  /** Monogram for logins/passwords; null → draw the kind glyph. */
  monogram: string | null;
  kind: ItemKind;
}

const MONOGRAM_KINDS = new Set<ItemKind>(["login", "password", "custom"]);

export function tileFor(kind: ItemKind, title: string, urls: string[] = []): TileSpec {
  const host = urls.length ? hostOf(urls[0]) : "";
  const seed = (host ? brandOf(host) : title.trim().toLowerCase()) || kind;
  const h = hash(seed);
  const monogramSource = (host ? brandOf(host) : title).trim();
  const letter = monogramSource.match(/[\p{L}\p{N}]/u)?.[0]?.toUpperCase() ?? null;
  const useMonogram = MONOGRAM_KINDS.has(kind) && letter !== null;
  return {
    glaze: useMonogram ? GLAZES[h % GLAZES.length] : template(kind).glaze,
    corner: ((h >>> 8) % 4) as TileSpec["corner"],
    monogram: useMonogram ? letter : null,
    kind,
  };
}
