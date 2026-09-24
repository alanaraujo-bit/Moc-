import { useEffect, useMemo, useRef, useState } from "react";
import { Plus, Trash, X } from "@phosphor-icons/react";
import { ItemTile } from "../../../components/tile/Tile";
import { Confirm, Menu } from "../../../components/ui/overlays";
import { Button, IconButton, Kbd, TextArea, TextField } from "../../../components/ui/primitives";
import { toast } from "../../../components/ui/toast";
import { api, errorMessage } from "../../../lib/ipc";
import { CUSTOM_FIELD_TYPES, FIELD_TYPE_LABELS, template, type FieldTemplate } from "../../../lib/templates";
import { brandOf, hostOf } from "../../../lib/tile";
import type { Field, FieldType, ItemInput, ItemKind, Uuid } from "../../../lib/types";
import { tagTree, useVault, type Editing } from "../../../state/vault";
import { FieldInput } from "./inputs";
import s from "./Editor.module.css";

interface CustomField {
  id: string;
  label: string;
  type: FieldType;
  value: string;
}

interface Draft {
  kind: ItemKind;
  vaultId: Uuid;
  title: string;
  values: Record<string, string>;
  custom: CustomField[];
  urls: string[];
  tags: string[];
  notes: string;
  favorite: boolean;
  icon: string | null;
}

const newId = () => Math.random().toString(36).slice(2, 12);

function titleFromUrl(url: string): string {
  const b = brandOf(hostOf(url));
  return b ? b.charAt(0).toUpperCase() + b.slice(1) : "";
}

export function ItemEditor({ editing }: { editing: Exclude<Editing, null> }) {
  const vaults = useVault((st) => st.vaults);
  const items = useVault((st) => st.items);
  const load = useVault((st) => st.load);
  const select = useVault((st) => st.select);
  const edit = useVault((st) => st.edit);
  const bumpItem = useVault((st) => st.bumpItem);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [initial, setInitial] = useState<string>("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [discard, setDiscard] = useState(false);
  const titleRef = useRef<HTMLInputElement>(null);

  // Load or initialize the draft.
  useEffect(() => {
    let alive = true;
    if (editing.mode === "new") {
      const d: Draft = {
        kind: editing.kind,
        vaultId: editing.vaultId,
        title: "",
        values: {},
        custom: [],
        urls: template(editing.kind).urls ? [""] : [],
        tags: [],
        notes: "",
        favorite: false,
        icon: null,
      };
      setDraft(d);
      setInitial(JSON.stringify(d));
      window.setTimeout(() => titleRef.current?.focus(), 30);
    } else {
      api
        .itemForEdit(editing.id)
        .then((full) => {
          if (!alive) return;
          const t = template(full.kind);
          const known = new Set(t.fields.map((f) => f.id));
          const values: Record<string, string> = {};
          const custom: CustomField[] = [];
          for (const f of full.details.fields) {
            if (known.has(f.id)) values[f.id] = f.value;
            else custom.push({ id: f.id, label: f.label, type: f.type, value: f.value });
          }
          const d: Draft = {
            kind: full.kind,
            vaultId: full.vaultId,
            title: full.overview.title,
            values,
            custom,
            urls: full.overview.urls.length ? full.overview.urls : t.urls ? [""] : [],
            tags: full.overview.tags,
            notes: full.details.notes,
            favorite: full.overview.favorite,
            icon: full.overview.icon ?? null,
          };
          setDraft(d);
          setInitial(JSON.stringify(d));
        })
        .catch((e) => setError(errorMessage(e)));
    }
    return () => {
      alive = false;
    };
  }, [editing]);

  const dirty = draft !== null && JSON.stringify(draft) !== initial;
  const t = draft ? template(draft.kind) : null;
  const context = useMemo(() => (draft ? [draft.title, draft.values.username ?? "", ...draft.urls.map(hostOf)] : []), [draft]);
  const allTags = useMemo(() => {
    const out: string[] = [];
    const walk = (nodes: ReturnType<typeof tagTree>) => nodes.forEach((n) => (out.push(n.path), walk(n.children)));
    walk(tagTree(items));
    return out;
  }, [items]);

  const cancel = () => {
    if (dirty) setDiscard(true);
    else edit(null);
  };

  const save = async () => {
    if (!draft || !t) return;
    const title = draft.title.trim() || (draft.urls[0] ? titleFromUrl(draft.urls[0]) : "");
    if (!title) {
      setError("Dê um nome para encontrar depois.");
      titleRef.current?.focus();
      return;
    }
    const fields: Field[] = [
      ...t.fields.filter((f) => (draft.values[f.id] ?? "").trim() !== "").map((f) => ({ id: f.id, label: "", type: f.type, value: draft.values[f.id], section: f.section ?? null })),
      ...draft.custom.filter((c) => c.value.trim() !== "" || c.label.trim() !== "").map((c) => ({ id: c.id, label: c.label.trim() || FIELD_TYPE_LABELS[c.type] || "Campo", type: c.type, value: c.value, section: null })),
    ];
    const input: ItemInput = {
      kind: draft.kind,
      title,
      urls: draft.urls.map((u) => u.trim()).filter(Boolean),
      tags: draft.tags,
      favorite: draft.favorite,
      icon: draft.icon,
      fields,
      sections: t.sections ?? [],
      notes: draft.notes,
    };
    setSaving(true);
    setError(null);
    try {
      if (editing.mode === "new") {
        const created = await api.createItem(draft.vaultId, input);
        await load();
        select(created.id);
        toast("Tá guardado.", { tone: "success" });
      } else {
        await api.updateItem(editing.id, input);
        if (draft.vaultId !== JSON.parse(initial).vaultId) await api.move(editing.id, draft.vaultId);
        await load();
        edit(null);
        bumpItem();
        toast("Alterações salvas", { tone: "success" });
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  // Ctrl+S saves, Esc cancels.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        void save();
      } else if (e.key === "Escape" && !document.querySelector("[role=dialog],[data-radix-popper-content-wrapper]")) {
        e.preventDefault();
        cancel();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  if (!draft || !t) {
    return <div className={s.loading}>{error ?? ""}</div>;
  }

  const set = (patch: Partial<Draft>) => setDraft((d) => (d ? { ...d, ...patch } : d));
  const setValue = (id: string, v: string) => setDraft((d) => (d ? { ...d, values: { ...d.values, [id]: v } } : d));

  const sections = t.sections ?? [];
  const fieldsIn = (sid: string | null) => t.fields.filter((f) => (f.section ?? null) === sid);
  const renderField = (f: FieldTemplate) => (
    <FieldInput key={f.id} tpl={f} value={draft.values[f.id] ?? ""} onChange={(v) => setValue(f.id, v)} context={context} />
  );

  return (
    <div className={s.root}>
      <div className={s.scroll}>
        <header className={s.header}>
          <ItemTile kind={draft.kind} title={draft.title || t.label} urls={draft.urls.filter(Boolean)} size={52} />
          <div className={s.headFields}>
            <input
              ref={titleRef}
              className={s.titleInput}
              value={draft.title}
              onChange={(e) => set({ title: e.target.value })}
              placeholder={t.titlePlaceholder}
              aria-label="Nome do item"
              maxLength={200}
              spellCheck={false}
            />
            <div className={s.meta}>
              <span>{editing.mode === "new" ? `Novo ${t.label.toLowerCase()}` : `Editando ${t.label.toLowerCase()}`}</span>
              <span className={s.dot}>·</span>
              <label className={s.vaultPick}>
                em
                <select value={draft.vaultId} onChange={(e) => set({ vaultId: e.target.value })} aria-label="Cofre">
                  {vaults.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.name}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          </div>
        </header>

        {error && (
          <p className={s.error} role="alert">
            {error}
          </p>
        )}

        <div className={s.body}>
          {draft.kind === "note" ? (
            <TextArea
              label="Nota"
              value={draft.notes}
              onChange={(e) => set({ notes: e.target.value })}
              placeholder="Escreva aqui. Só você vai ler."
              rows={12}
              className={s.noteArea}
            />
          ) : (
            <>
              {fieldsIn(null).length > 0 && <section className={s.section}>{fieldsIn(null).map(renderField)}</section>}
              {sections.map((sec) => (
                <section key={sec.id} className={s.section}>
                  <h2 className={s.sectionTitle}>{sec.label}</h2>
                  {fieldsIn(sec.id).map(renderField)}
                </section>
              ))}
            </>
          )}

          {t.urls && (
            <section className={s.section}>
              <h2 className={s.sectionTitle}>{draft.urls.length > 1 ? "Sites" : "Site"}</h2>
              {draft.urls.map((u, i) => (
                <div key={i} className={s.urlRow}>
                  <TextField
                    value={u}
                    aria-label={`Site ${i + 1}`}
                    placeholder="exemplo.com.br"
                    onChange={(e) => set({ urls: draft.urls.map((x, j) => (j === i ? e.target.value : x)) })}
                    onBlur={() => {
                      if (i === 0 && !draft.title.trim() && u.trim()) set({ title: titleFromUrl(u) });
                    }}
                  />
                  {draft.urls.length > 1 && (
                    <IconButton label="Remover site" onClick={() => set({ urls: draft.urls.filter((_, j) => j !== i) })}>
                      <X size={15} />
                    </IconButton>
                  )}
                </div>
              ))}
              <Button size="sm" variant="ghost" icon={<Plus size={14} />} onClick={() => set({ urls: [...draft.urls, ""] })} className={s.addBtn}>
                Adicionar outro site
              </Button>
            </section>
          )}

          {(draft.custom.length > 0 || draft.kind === "custom") && (
            <section className={s.section}>
              <h2 className={s.sectionTitle}>{draft.kind === "custom" ? "Campos" : "Campos extras"}</h2>
              {draft.custom.map((c, i) => (
                <div key={c.id} className={s.customRow}>
                  <input
                    className={s.customLabel}
                    value={c.label}
                    placeholder={`Nome do campo (${(FIELD_TYPE_LABELS[c.type] ?? "texto").toLowerCase()})`}
                    aria-label="Nome do campo"
                    onChange={(e) => set({ custom: draft.custom.map((x, j) => (j === i ? { ...x, label: e.target.value } : x)) })}
                  />
                  <div className={s.customValue}>
                    <FieldInput
                      tpl={{ id: c.id, label: "", type: c.type }}
                      value={c.value}
                      context={context}
                      onChange={(v) => set({ custom: draft.custom.map((x, j) => (j === i ? { ...x, value: v } : x)) })}
                    />
                    <IconButton label="Remover campo" onClick={() => set({ custom: draft.custom.filter((_, j) => j !== i) })}>
                      <Trash size={15} />
                    </IconButton>
                  </div>
                </div>
              ))}
              {draft.kind === "custom" && draft.custom.length === 0 && (
                <p className={s.hintText}>Monte do seu jeito: adicione os campos que esse item precisa.</p>
              )}
            </section>
          )}

          <Menu
            align="start"
            trigger={
              <Button size="sm" variant="ghost" icon={<Plus size={14} />} className={s.addBtn}>
                Adicionar campo
              </Button>
            }
            items={CUSTOM_FIELD_TYPES.map((ft) => ({
              label: FIELD_TYPE_LABELS[ft] ?? ft,
              onSelect: () => set({ custom: [...draft.custom, { id: newId(), label: "", type: ft, value: "" }] }),
            }))}
          />

          {draft.kind !== "note" && (
            <TextArea label="Observações" value={draft.notes} onChange={(e) => set({ notes: e.target.value })} rows={3} placeholder="Qualquer detalhe que ajude depois." />
          )}

          <TagInput tags={draft.tags} onChange={(tags) => set({ tags })} suggestions={allTags} />
        </div>
      </div>

      <footer className={s.footer}>
        <span className={s.footHint}>
          <Kbd keys="Ctrl+S" /> salva · <Kbd keys="Esc" /> cancela
        </span>
        <Button onClick={cancel}>Cancelar</Button>
        <Button variant="primary" onClick={save} loading={saving}>
          {editing.mode === "new" ? "Guardar" : "Salvar"}
        </Button>
      </footer>

      <Confirm
        open={discard}
        onOpenChange={setDiscard}
        title="Descartar alterações?"
        description="O que você mudou neste item ainda não foi salvo."
        confirmLabel="Descartar"
        danger
        onConfirm={() => {
          setDiscard(false);
          edit(null);
        }}
      />
    </div>
  );
}

function TagInput({ tags, onChange, suggestions }: { tags: string[]; onChange: (t: string[]) => void; suggestions: string[] }) {
  const [text, setText] = useState("");
  const add = (raw: string) => {
    const tag = raw.trim().replace(/^#/, "");
    if (tag && !tags.some((t) => t.toLowerCase() === tag.toLowerCase())) onChange([...tags, tag]);
    setText("");
  };
  const matches = text ? suggestions.filter((s) => s.toLowerCase().includes(text.toLowerCase()) && !tags.includes(s)).slice(0, 5) : [];
  return (
    <div className={s.tags}>
      <span className={s.selectLabel}>Etiquetas</span>
      <div className={s.tagBox}>
        {tags.map((t) => (
          <span key={t} className={s.tagChip}>
            #{t}
            <button aria-label={`Remover ${t}`} onClick={() => onChange(tags.filter((x) => x !== t))}>
              <X size={11} weight="bold" />
            </button>
          </span>
        ))}
        <input
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder={tags.length ? "" : "trabalho, família/escola…"}
          aria-label="Nova etiqueta"
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === ",") {
              e.preventDefault();
              add(text);
            } else if (e.key === "Backspace" && !text && tags.length) {
              onChange(tags.slice(0, -1));
            }
          }}
          onBlur={() => text && add(text)}
        />
      </div>
      {matches.length > 0 && (
        <div className={s.tagSuggest}>
          {matches.map((m) => (
            <button key={m} onMouseDown={(e) => (e.preventDefault(), add(m))}>
              #{m}
            </button>
          ))}
        </div>
      )}
      <span className={s.hintText}>Use “/” para criar níveis, como pastas: trabalho/clientes.</span>
    </div>
  );
}
