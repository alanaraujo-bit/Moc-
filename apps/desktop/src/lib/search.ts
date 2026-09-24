// Instant, accent-insensitive, typo-tolerant search over item summaries.
// Runs in memory on already-decrypted overviews; nothing is indexed on disk.

import { template, TEMPLATES } from "./templates";
import { hostOf } from "./tile";
import type { ItemKind, ItemSummary, Usage } from "./types";

export function fold(s: string): string {
  return s.normalize("NFD").replace(/\p{M}+/gu, "").toLowerCase();
}

interface Doc {
  item: ItemSummary;
  title: string;
  titleWords: string[];
  others: string[]; // folded words from subtitle, hosts, tags, keywords
  otherText: string;
  kindWords: string[];
  tags: string[];
}

const splitWords = (s: string) => fold(s).split(/[^\p{L}\p{N}@._-]+/u).filter(Boolean);

export function buildIndex(items: ItemSummary[]): Doc[] {
  return items.map((item) => {
    const hosts = item.urls.map(hostOf);
    const others = [item.subtitle, ...hosts, ...hosts.map((h) => h.split(".")[0]), ...item.tags, ...item.keywords];
    const t = template(item.kind);
    return {
      item,
      title: fold(item.title),
      titleWords: splitWords(item.title),
      others: others.flatMap(splitWords),
      otherText: fold(others.join(" ")),
      kindWords: [t.label, t.plural, ...t.aliases].flatMap(splitWords),
      tags: item.tags.map(fold),
    };
  });
}

/** Optimal string alignment distance with an early cut-off. */
function editDistance(a: string, b: string, max: number): number {
  if (Math.abs(a.length - b.length) > max) return max + 1;
  const prev2 = new Array(b.length + 1).fill(0);
  let prev = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    const cur = [i];
    let rowMin = i;
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      let v = Math.min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + cost);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) v = Math.min(v, prev2[j - 2] + 1);
      cur[j] = v;
      rowMin = Math.min(rowMin, v);
    }
    if (rowMin > max) return max + 1;
    for (let j = 0; j <= b.length; j++) prev2[j] = prev[j];
    prev = cur;
  }
  return prev[b.length];
}

function tokenScore(doc: Doc, token: string): number {
  if (doc.title.startsWith(token)) return 100;
  if (doc.titleWords.some((w) => w.startsWith(token))) return 80;
  if (doc.title.includes(token)) return 60;
  if (doc.others.some((w) => w.startsWith(token))) return 50;
  if (doc.otherText.includes(token)) return 35;
  if (doc.kindWords.some((w) => w.startsWith(token))) return 25;
  // Typos: "nubnak" → "nubank", "netflx" → "netflix".
  if (token.length >= 4) {
    const max = token.length >= 8 ? 2 : 1;
    const near = (w: string) => editDistance(token, w.slice(0, token.length + max), max) <= max;
    if (doc.titleWords.some(near)) return 30;
    if (doc.others.some(near)) return 15;
  }
  return 0;
}

export interface ParsedQuery {
  terms: string[];
  kinds: ItemKind[];
  tags: string[];
  favorite: boolean;
  totp: boolean;
}

const KIND_ALIASES: [string, ItemKind][] = TEMPLATES.flatMap((t) =>
  [t.kind, t.label, t.plural, ...t.aliases].map((a) => [fold(a).replace(/\s+/g, ""), t.kind] as [string, ItemKind]),
);

export function parseQuery(q: string): ParsedQuery {
  const out: ParsedQuery = { terms: [], kinds: [], tags: [], favorite: false, totp: false };
  for (const raw of q.trim().split(/\s+/).filter(Boolean)) {
    const m = raw.match(/^(tipo|kind|tag|#|is|é|tem):?(.*)$/i);
    const folded = fold(raw);
    if (raw.startsWith("#") && raw.length > 1) {
      out.tags.push(fold(raw.slice(1)));
    } else if (m && m[2] && raw.includes(":")) {
      const key = fold(m[1]);
      const val = fold(m[2]);
      if (key === "tipo" || key === "kind") {
        const k = KIND_ALIASES.find(([a]) => a.startsWith(val))?.[1];
        if (k) out.kinds.push(k);
      } else if (key === "tag") {
        out.tags.push(val);
      } else if ((key === "is" || key === "e") && val.startsWith("fav")) {
        out.favorite = true;
      } else if (key === "tem" && (val === "2fa" || val === "totp")) {
        out.totp = true;
      } else {
        out.terms.push(folded);
      }
    } else {
      out.terms.push(folded);
    }
  }
  return out;
}

export interface Hit {
  item: ItemSummary;
  score: number;
}

export function search(index: Doc[], q: string, usage: Record<string, Usage> = {}): Hit[] {
  const pq = parseQuery(q);
  const hits: Hit[] = [];
  for (const doc of index) {
    const it = doc.item;
    if (pq.kinds.length && !pq.kinds.includes(it.kind)) continue;
    if (pq.favorite && !it.favorite) continue;
    if (pq.totp && !it.hasTotp) continue;
    if (pq.tags.length && !pq.tags.every((t) => doc.tags.some((dt) => dt === t || dt.startsWith(t + "/") || dt.startsWith(t)))) continue;
    let score = 0;
    let ok = true;
    for (const term of pq.terms) {
      const s = tokenScore(doc, term);
      if (s === 0) {
        ok = false;
        break;
      }
      score += s;
    }
    if (!ok) continue;
    const u = usage[it.id];
    if (u) score += Math.min(12, u.count) + (Date.now() - u.lastUsedAt < 7 * 864e5 ? 6 : 0);
    if (it.favorite) score += 4;
    hits.push({ item: it, score });
  }
  hits.sort((a, b) => b.score - a.score || a.item.title.localeCompare(b.item.title, "pt-BR"));
  return hits;
}

/** Ranges in `text` to highlight for the query terms (accent-insensitive). */
export function highlightRanges(text: string, q: string): [number, number][] {
  const terms = parseQuery(q).terms.filter((t) => t.length > 0);
  if (!terms.length) return [];
  const folded = fold(text);
  // Folding can change length for some scripts; bail out if it did.
  if (folded.length !== text.length) return [];
  const ranges: [number, number][] = [];
  for (const t of terms) {
    const i = folded.indexOf(t);
    if (i >= 0) ranges.push([i, i + t.length]);
  }
  ranges.sort((a, b) => a[0] - b[0]);
  const merged: [number, number][] = [];
  for (const r of ranges) {
    const last = merged[merged.length - 1];
    if (last && r[0] <= last[1]) last[1] = Math.max(last[1], r[1]);
    else merged.push([...r]);
  }
  return merged;
}
