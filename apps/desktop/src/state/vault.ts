import { create } from "zustand";
import { api } from "../lib/ipc";
import { buildIndex, search } from "../lib/search";
import type { ItemKind, ItemSummary, Usage, Uuid, VaultInfo } from "../lib/types";

export type View =
  | { type: "all" }
  | { type: "favorites" }
  | { type: "recents" }
  | { type: "vault"; id: Uuid }
  | { type: "kind"; kind: ItemKind }
  | { type: "tag"; tag: string }
  | { type: "archive" }
  | { type: "trash" };

export type Screen = "vault" | "security" | "generator" | "settings" | "import";

export type Sort = "updated" | "title" | "created" | "used";

export type Editing = null | { mode: "new"; kind: ItemKind; vaultId: Uuid } | { mode: "edit"; id: Uuid };

interface VaultStore {
  loaded: boolean;
  vaults: VaultInfo[];
  items: ItemSummary[];
  usage: Record<Uuid, Usage>;
  index: ReturnType<typeof buildIndex>;
  screen: Screen;
  view: View;
  query: string;
  sort: Sort;
  selectedId: Uuid | null;
  editing: Editing;
  /** Bumped when the selected item's content changed and the detail should refetch. */
  itemVersion: number;
  load: () => Promise<void>;
  refresh: () => Promise<void>;
  setScreen: (s: Screen) => void;
  setView: (v: View) => void;
  setQuery: (q: string) => void;
  setSort: (s: Sort) => void;
  select: (id: Uuid | null) => void;
  edit: (e: Editing) => void;
  bumpItem: () => void;
}

export const useVault = create<VaultStore>((set, get) => ({
  loaded: false,
  vaults: [],
  items: [],
  usage: {},
  index: [],
  screen: "vault",
  view: { type: "all" },
  query: "",
  sort: "updated",
  selectedId: null,
  editing: null,
  itemVersion: 0,
  load: async () => {
    const [vaults, payload] = await Promise.all([api.vaults(), api.items()]);
    set({ vaults, items: payload.items, usage: payload.usage, index: buildIndex(payload.items), loaded: true });
  },
  refresh: async () => {
    await get().load();
    set((s) => ({ itemVersion: s.itemVersion + 1 }));
  },
  setScreen: (screen) => set({ screen, editing: null }),
  setView: (view) => set({ view, screen: "vault", editing: null }),
  setQuery: (query) => set({ query }),
  setSort: (sort) => set({ sort }),
  select: (selectedId) => set({ selectedId, editing: null }),
  edit: (editing) => set({ editing }),
  bumpItem: () => set((s) => ({ itemVersion: s.itemVersion + 1 })),
}));

export function inView(it: ItemSummary, view: View, usage: Record<Uuid, Usage>): boolean {
  const trashed = it.trashedAt != null;
  switch (view.type) {
    case "trash":
      return trashed;
    case "archive":
      return !trashed && it.archived;
    case "recents":
      return !trashed && !!usage[it.id];
    default:
      if (trashed || it.archived) return false;
  }
  switch (view.type) {
    case "all":
      return true;
    case "favorites":
      return it.favorite;
    case "vault":
      return it.vaultId === view.id;
    case "kind":
      return it.kind === view.kind;
    case "tag":
      return it.tags.some((t) => t === view.tag || t.startsWith(view.tag + "/"));
  }
}

/** Items for the current view, query and sort. */
export function selectVisible(s: Pick<VaultStore, "items" | "index" | "view" | "query" | "sort" | "usage">): ItemSummary[] {
  const q = s.query.trim();
  if (q) {
    // Searching looks everywhere except the trash (unless you're in it).
    const scope = s.view.type === "trash" ? s.view : s.view.type === "archive" ? s.view : null;
    return search(s.index, q, s.usage)
      .map((h) => h.item)
      .filter((it) => (scope ? inView(it, scope, s.usage) : it.trashedAt == null));
  }
  const list = s.items.filter((it) => inView(it, s.view, s.usage));
  const by = s.view.type === "recents" ? "used" : s.sort;
  const collator = new Intl.Collator("pt-BR", { sensitivity: "base", numeric: true });
  list.sort((a, b) => {
    switch (by) {
      case "title":
        return collator.compare(a.title, b.title);
      case "created":
        return b.createdAt - a.createdAt;
      case "used":
        return (s.usage[b.id]?.lastUsedAt ?? 0) - (s.usage[a.id]?.lastUsedAt ?? 0);
      default:
        return b.updatedAt - a.updatedAt;
    }
  });
  return list;
}

export interface TagNode {
  name: string;
  path: string;
  count: number;
  children: TagNode[];
}

export function tagTree(items: ItemSummary[]): TagNode[] {
  const root: TagNode = { name: "", path: "", count: 0, children: [] };
  for (const it of items) {
    if (it.trashedAt != null || it.archived) continue;
    for (const tag of it.tags) {
      let node = root;
      let path = "";
      for (const part of tag.split("/")) {
        path = path ? `${path}/${part}` : part;
        let child = node.children.find((c) => c.name.toLowerCase() === part.toLowerCase());
        if (!child) {
          child = { name: part, path, count: 0, children: [] };
          node.children.push(child);
        }
        child.count++;
        node = child;
      }
    }
  }
  const sortRec = (n: TagNode) => {
    n.children.sort((a, b) => a.name.localeCompare(b.name, "pt-BR"));
    n.children.forEach(sortRec);
  };
  sortRec(root);
  return root.children;
}
