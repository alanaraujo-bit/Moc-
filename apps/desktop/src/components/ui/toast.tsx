import { useEffect, useState, type ReactNode } from "react";
import { create } from "zustand";
import { CheckCircle, Info, WarningCircle } from "@phosphor-icons/react";
import s from "./ui.module.css";

export interface Toast {
  id: number;
  message: ReactNode;
  detail?: ReactNode;
  tone?: "default" | "success" | "danger";
  /** Seconds shown as a draining ring (e.g. clipboard auto-clear). */
  countdown?: number;
  action?: { label: string; run: () => void };
  duration?: number;
}

interface ToastState {
  toasts: (Toast & { leaving?: boolean })[];
  push: (t: Omit<Toast, "id">) => number;
  dismiss: (id: number) => void;
}

let seq = 1;

export const useToasts = create<ToastState>((set, get) => ({
  toasts: [],
  push: (t) => {
    const id = seq++;
    // Keep at most three on screen; same message replaces the previous one.
    set((st) => ({ toasts: [...st.toasts.filter((x) => x.message !== t.message).slice(-2), { ...t, id }] }));
    const duration = t.duration ?? (t.action ? 6000 : 2600);
    window.setTimeout(() => get().dismiss(id), duration);
    return id;
  },
  dismiss: (id) => {
    set((st) => ({ toasts: st.toasts.map((x) => (x.id === id ? { ...x, leaving: true } : x)) }));
    window.setTimeout(() => set((st) => ({ toasts: st.toasts.filter((x) => x.id !== id) })), 200);
  },
}));

export const toast = (message: ReactNode, opts: Omit<Toast, "id" | "message"> = {}) => useToasts.getState().push({ message, ...opts });

function Countdown({ seconds }: { seconds: number }) {
  const [left, setLeft] = useState(seconds);
  useEffect(() => {
    const started = Date.now();
    const t = window.setInterval(() => setLeft(Math.max(0, seconds - (Date.now() - started) / 1000)), 200);
    return () => window.clearInterval(t);
  }, [seconds]);
  const r = 6;
  const c = 2 * Math.PI * r;
  return (
    <svg className={s.countdown} viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r={r} stroke="currentColor" strokeOpacity=".25" />
      <circle
        cx="8"
        cy="8"
        r={r}
        stroke="currentColor"
        strokeDasharray={c}
        strokeDashoffset={c * (1 - left / seconds)}
        transform="rotate(-90 8 8)"
        strokeLinecap="round"
        style={{ transition: "stroke-dashoffset 200ms linear" }}
      />
    </svg>
  );
}

export function Toaster() {
  const toasts = useToasts((st) => st.toasts);
  const dismiss = useToasts((st) => st.dismiss);
  return (
    <div className={s.toasts} role="status" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`${s.toast} ${t.tone === "danger" ? s.toastDanger : ""}`} data-leaving={t.leaving || undefined}>
          {t.tone === "success" && <CheckCircle size={18} weight="fill" />}
          {t.tone === "danger" && <WarningCircle size={18} weight="fill" />}
          {!t.tone || t.tone === "default" ? t.countdown ? null : <Info size={18} /> : null}
          <span>
            {t.message}
            {t.detail && <span className={s.toastDetail}> · {t.detail}</span>}
          </span>
          {t.countdown ? <Countdown seconds={t.countdown} /> : null}
          {t.action && (
            <button
              className={s.toastAction}
              onClick={() => {
                t.action!.run();
                dismiss(t.id);
              }}
            >
              {t.action.label}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
