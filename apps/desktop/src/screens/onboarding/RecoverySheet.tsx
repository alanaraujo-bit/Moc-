import { useState } from "react";
import { Copy, Printer } from "@phosphor-icons/react";
import { Button } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { api, errorMessage } from "../../lib/ipc";
import { PrintPortal, RecoverySheetPrint } from "./Printable";
import s from "./Onboarding.module.css";

export function CodeBlock({ value, label }: { value: string; label: string }) {
  return (
    <div className={s.codeBlock} aria-label={label}>
      <span className={`${s.codeText} mono selectable`}>
        {value.split("-").map((g, i, all) => (
          <span key={i} className={s.codeGroup}>
            {g}
            {i < all.length - 1 && <span className={s.codeDash}>-</span>}
          </span>
        ))}
      </span>
    </div>
  );
}

export function printNow(setPrinting: (v: boolean) => void) {
  setPrinting(true);
  const done = () => {
    window.removeEventListener("afterprint", done);
    setPrinting(false);
  };
  window.addEventListener("afterprint", done);
  // Let the portal render before opening the dialog.
  window.setTimeout(() => window.print(), 80);
}

export function RecoverySheet({ code, onDone, doneLabel = "Continuar" }: { code: string; onDone: () => void; doneLabel?: string }) {
  const [saved, setSaved] = useState(false);
  const [printing, setPrinting] = useState(false);
  return (
    <div className={s.stack}>
      <CodeBlock value={code} label="Código de recuperação" />
      <div className={s.actions}>
        <Button icon={<Printer size={16} />} onClick={() => printNow(setPrinting)}>
          Imprimir ou salvar em PDF
        </Button>
        <Button
          variant="ghost"
          icon={<Copy size={16} />}
          onClick={async () => {
            try {
              const r = await api.copyText(code, true);
              toast("Código copiado", { detail: r.clearsIn ? `some em ${r.clearsIn}s` : undefined, countdown: r.clearsIn || undefined });
            } catch (e) {
              toast(errorMessage(e), { tone: "danger" });
            }
          }}
        >
          Copiar
        </Button>
      </div>
      <label className={s.check}>
        <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
        <span>Guardei o código num lugar seguro, separado do Kit de Emergência.</span>
      </label>
      <Button variant="primary" size="lg" disabled={!saved} onClick={onDone}>
        {doneLabel}
      </Button>
      {printing && (
        <PrintPortal>
          <RecoverySheetPrint code={code} />
        </PrintPortal>
      )}
    </div>
  );
}
