import { useEffect, useState } from "react";
import { DownloadSimple, Eye, File, FileImage, FilePdf, Paperclip, QrCode, Trash, UploadSimple } from "@phosphor-icons/react";
import { Confirm, Dialog } from "../../components/ui/overlays";
import { Button, IconButton } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { bytes, relativeTime } from "../../lib/format";
import { api, errorMessage } from "../../lib/ipc";
import type { ItemView } from "../../lib/types";
import { useVault } from "../../state/vault";
import s from "./Attachments.module.css";

function Icon({ mime }: { mime: string }) {
  if (mime.startsWith("image/")) return <FileImage size={20} />;
  if (mime === "application/pdf") return <FilePdf size={20} />;
  return <File size={20} />;
}

export function Attachments({ item }: { item: ItemView }) {
  const refresh = useVault((st) => st.refresh);
  const [preview, setPreview] = useState<{ name: string; url: string } | null>(null);
  const [removing, setRemoving] = useState<ItemView["attachments"][number] | null>(null);
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);

  // Native drag-and-drop: the webview hands us file paths, Rust reads and encrypts them.
  useEffect(() => {
    let un: (() => void) | undefined;
    let alive = true;
    import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
      getCurrentWebview()
        .onDragDropEvent(async (e) => {
          const p = e.payload;
          if (p.type === "enter" || p.type === "over") setDragging(true);
          else if (p.type === "leave") setDragging(false);
          else if (p.type === "drop") {
            setDragging(false);
            if (!p.paths.length || item.trashedAt != null) return;
            setBusy(true);
            try {
              await api.attachmentAddPaths(item.id, p.paths);
              await refresh();
              toast(p.paths.length === 1 ? "Anexado e cifrado" : `${p.paths.length} arquivos anexados`, { tone: "success" });
            } catch (err) {
              toast(errorMessage(err), { tone: "danger" });
            } finally {
              setBusy(false);
            }
          }
        })
        .then((u) => (alive ? (un = u) : u())),
    );
    return () => {
      alive = false;
      un?.();
    };
  }, [item.id, item.trashedAt, refresh]);

  const add = async () => {
    setBusy(true);
    try {
      const r = await api.attachmentAdd(item.id);
      if (r) {
        await refresh();
        toast("Anexado e cifrado", { tone: "success" });
      }
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    } finally {
      setBusy(false);
    }
  };

  const save = async (id: string) => {
    try {
      const path = await api.attachmentSave(item.id, id);
      if (path) toast("Arquivo salvo", { tone: "success", detail: "fora do Mocó ele não tem proteção" });
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    }
  };

  const show = async (a: ItemView["attachments"][number]) => {
    try {
      setPreview({ name: a.name, url: await api.attachmentPreview(item.id, a.id) });
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    }
  };

  const hasFiles = item.attachments.length > 0;
  return (
    <section className={s.root} data-dragging={dragging || undefined}>
      <header className={s.head}>
        <h2>
          <Paperclip size={14} /> Anexos
        </h2>
        {item.trashedAt == null && (
          <Button size="sm" variant="ghost" icon={<UploadSimple size={14} />} loading={busy} onClick={add}>
            Anexar arquivo
          </Button>
        )}
      </header>
      {hasFiles ? (
        <ul className={s.list}>
          {item.attachments.map((a) => (
            <li key={a.id} className={s.file}>
              <span className={s.icon}>
                <Icon mime={a.mime} />
              </span>
              <button className={s.name} onClick={() => (a.mime.startsWith("image/") ? void show(a) : void save(a.id))}>
                <strong>{a.name}</strong>
                <span>
                  {bytes(a.size)} · {relativeTime(a.addedAt)}
                </span>
              </button>
              {a.mime.startsWith("image/") && (
                <IconButton small label="Ver" onClick={() => void show(a)}>
                  <Eye size={15} />
                </IconButton>
              )}
              <IconButton small label="Salvar como…" onClick={() => void save(a.id)}>
                <DownloadSimple size={15} />
              </IconButton>
              <IconButton small label="Remover" onClick={() => setRemoving(a)}>
                <Trash size={15} />
              </IconButton>
            </li>
          ))}
        </ul>
      ) : (
        <p className={s.empty}>{dragging ? "Solte aqui para anexar." : "Arraste arquivos para cá — foto do documento, contrato, comprovante. Eles ficam cifrados junto com o item (até 25 MB)."}</p>
      )}

      <Dialog open={!!preview} onOpenChange={(o) => !o && setPreview(null)} wide title={preview?.name ?? ""} footer={<Button onClick={() => setPreview(null)}>Fechar</Button>}>
        {preview && <img src={preview.url} alt={preview.name} className={s.preview} />}
      </Dialog>
      <Confirm
        open={!!removing}
        onOpenChange={(o) => !o && setRemoving(null)}
        title={`Remover “${removing?.name ?? ""}”?`}
        description="O arquivo sai deste item e é apagado do Mocó. Se precisar dele depois, salve uma cópia antes."
        confirmLabel="Remover anexo"
        danger
        onConfirm={async () => {
          const a = removing;
          setRemoving(null);
          if (!a) return;
          try {
            await api.attachmentRemove(item.id, a.id);
            await refresh();
            toast("Anexo removido");
          } catch (e) {
            toast(errorMessage(e), { tone: "danger" });
          }
        }}
      />
    </section>
  );
}

export function WifiShare({ item }: { item: ItemView }) {
  const [svg, setSvg] = useState<string | null>(null);
  const ssid = item.fields.find((f) => f.id === "ssid")?.value;
  return (
    <section className={s.wifi}>
      <div>
        <strong>Visitas?</strong>
        <span>Mostre um QR code: a câmera do celular conecta na rede sem ninguém digitar a senha.</span>
      </div>
      <Button
        icon={<QrCode size={16} />}
        onClick={async () => {
          try {
            setSvg(await api.wifiQr(item.id));
          } catch (e) {
            toast(errorMessage(e), { tone: "danger" });
          }
        }}
      >
        Mostrar QR code
      </Button>
      <Dialog
        open={!!svg}
        onOpenChange={(o) => !o && setSvg(null)}
        title={ssid ? `Wi-Fi ${ssid}` : "Wi-Fi"}
        description="Aponte a câmera do celular. O código contém a senha da rede — feche quando terminar."
        footer={<Button onClick={() => setSvg(null)}>Fechar</Button>}
      >
        {/* Generated by the qrcode crate from escaped data: only rects and paths, no user markup. */}
        {svg && <div className={s.qr} dangerouslySetInnerHTML={{ __html: svg }} />}
      </Dialog>
    </section>
  );
}
