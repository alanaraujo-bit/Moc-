// Quick Access: a search box over any app. Type, Enter copies the password, done.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ArrowRight, Fingerprint, MagnifyingGlass } from "@phosphor-icons/react";
import { MocoMark } from "../../components/brand/Brand";
import { ItemTile } from "../../components/tile/Tile";
import { Kbd, Spinner } from "../../components/ui/primitives";
import { api, errorCode, errorMessage, on } from "../../lib/ipc";
import { buildIndex, search } from "../../lib/search";
import { template } from "../../lib/templates";
import type { AppInfo, ItemSummary, Usage } from "../../lib/types";
import { PRIMARY_FIELD } from "../main/ItemDetail";
import s from "./QuickAccess.module.css";

type Mode = "loading" | "locked" | "ready" | "empty";

export function QuickAccess() {
  const [mode, setMode] = useState<Mode>("loading");
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [items, setItems] = useState<ItemSummary[]>([]);
  const [usage, setUsage] = useState<Record<string, Usage>>({});
  const [q, setQ] = useState("");
  const [active, setActive] = useState(0);
  const [flash, setFlash] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const pw = useRef<HTMLInputElement>(null);

  const load = useCallback(async () => {
    try {
      const i = await api.appInfo();
      setInfo(i);
      if (!i.initialized) return setMode("empty");
      if (!i.unlocked) return setMode("locked");
      const p = await api.items();
      setItems(p.items.filter((x) => x.trashedAt == null));
      setUsage(p.usage);
      setMode("ready");
    } catch {
      setMode("locked");
    }
  }, []);

  useEffect(() => {
    void load();
    const subs = [
      on("moco://quick-shown", () => {
        setQ("");
        setFlash(null);
        setError(null);
        void load();
        window.setTimeout(() => (input.current ?? pw.current)?.focus(), 20);
      }),
      on("moco://unlocked", () => void load()),
    ];
    return () => subs.forEach((p) => p.then((u) => u()));
  }, [load]);

  useEffect(() => {
    window.setTimeout(() => (mode === "locked" ? pw.current : input.current)?.focus(), 20);
  }, [mode]);

  const index = useMemo(() => buildIndex(items), [items]);
  const results = useMemo(() => {
    if (q.trim()) return search(index, q, usage).slice(0, 8).map((h) => h.item);
    // Nothing typed: recently used first, then favorites.
    const recent = [...items].filter((i) => usage[i.id]).sort((a, b) => usage[b.id].lastUsedAt - usage[a.id].lastUsedAt);
    const favs = items.filter((i) => i.favorite && !usage[i.id]);
    return [...recent, ...favs].slice(0, 8);
  }, [q, index, items, usage]);

  useEffect(() => setActive(0), [q]);

  const hide = () => void api.quickHide();

  const copy = async (item: ItemSummary, which: "primary" | "username" | "totp") => {
    const field = which === "username" ? "username" : which === "totp" ? "totp" : PRIMARY_FIELD[item.kind] || "notes";
    try {
      const r = await api.copyField(item.id, field);
      const what = which === "username" ? "Usuário" : which === "totp" ? "Código" : field === "password" ? "Senha" : "Copiado";
      setFlash(`${what} de ${item.title} copiado${r.clearsIn ? ` · some em ${r.clearsIn}s` : ""}`);
      window.setTimeout(hide, 650);
    } catch (e) {
      setFlash(errorMessage(e));
    }
  };

  const unlock = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.unlock(pw.current?.value ?? "");
      if (pw.current) pw.current.value = "";
      await load();
    } catch (e) {
      setError(errorCode(e) === "secret_key_missing" ? "Abra o Mocó para digitar sua Chave Secreta." : errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const hello = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.unlockHello();
      await load();
    } catch (e) {
      if (errorCode(e) !== "hello_canceled") setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      if (q) setQ("");
      else hide();
      return;
    }
    if (mode !== "ready") return;
    const cur = results[active];
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((a) => Math.min(results.length - 1, a + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(0, a - 1));
    } else if (e.key === "Enter" && cur) {
      e.preventDefault();
      if (e.ctrlKey && e.shiftKey) void api.quickOpenInMain(cur.id);
      else if (e.shiftKey) void copy(cur, "username");
      else if (e.ctrlKey) void copy(cur, "totp");
      else if (e.altKey && cur.urls[0]) {
        void api.openUrl(cur.urls[0]);
        hide();
      } else void copy(cur, "primary");
    }
  };

  return (
    <div className={s.root} onKeyDown={onKey}>
      {mode === "locked" ? (
        <form
          className={s.locked}
          onSubmit={(e) => {
            e.preventDefault();
            void unlock();
          }}
        >
          <MocoMark size={28} />
          <div className={s.lockedText}>
            <strong>Seu Mocó está trancado.</strong>
            <span>{error ?? "Digite a senha mestra para buscar."}</span>
          </div>
          <input ref={pw} type="password" className={s.lockInput} placeholder="Senha mestra" aria-label="Senha mestra" disabled={busy} />
          {info?.helloEnrolled && !info.passwordDue && (
            <button type="button" className={s.iconBtn} onClick={hello} aria-label="Usar o Windows Hello" title="Windows Hello">
              <Fingerprint size={18} />
            </button>
          )}
          <button type="submit" className={s.go} aria-label="Abrir" disabled={busy}>
            {busy ? <Spinner size={15} /> : <ArrowRight size={17} weight="bold" />}
          </button>
        </form>
      ) : (
        <>
          <div className={s.searchRow}>
            <MagnifyingGlass size={20} className={s.searchIcon} />
            <input
              ref={input}
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder={mode === "empty" ? "Crie seu Mocó na janela principal primeiro" : "Buscar no Mocó"}
              aria-label="Buscar no Mocó"
              spellCheck={false}
              disabled={mode !== "ready"}
            />
            <MocoMark size={22} className={s.mark} />
          </div>
          <ul className={s.results} role="listbox" aria-label="Resultados">
            {results.map((it, i) => (
              <li
                key={it.id}
                role="option"
                aria-selected={i === active}
                className={s.result}
                data-active={i === active || undefined}
                onMouseEnter={() => setActive(i)}
                onClick={() => void copy(it, "primary")}
              >
                <ItemTile kind={it.kind} title={it.title} urls={it.urls} size={30} />
                <span className={s.text}>
                  <strong>{it.title}</strong>
                  <span>{it.subtitle || template(it.kind).label}</span>
                </span>
                {i === active && (
                  <span className={s.hint}>
                    <Kbd keys="Enter" /> {PRIMARY_FIELD[it.kind] === "password" ? "copia a senha" : "copia"}
                  </span>
                )}
              </li>
            ))}
            {mode === "ready" && results.length === 0 && <li className={s.none}>{q ? `Nada com “${q}”.` : "Busque por nome, site ou usuário."}</li>}
          </ul>
          <footer className={s.foot}>
            {flash ? (
              <span className={s.flash}>{flash}</span>
            ) : (
              <>
                <span>
                  <Kbd keys="Shift+Enter" /> usuário
                </span>
                <span>
                  <Kbd keys="Ctrl+Enter" /> código 2FA
                </span>
                <span>
                  <Kbd keys="Alt+Enter" /> abre o site
                </span>
                <span>
                  <Kbd keys="Esc" /> fecha
                </span>
              </>
            )}
          </footer>
        </>
      )}
    </div>
  );
}
