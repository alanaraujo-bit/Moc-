import { useEffect, useState, type ReactNode } from "react";
import { ArrowRight, CaretDown, CheckCircle, Fingerprint, Globe, Key, Lifebuoy, LockSimple, Password, Recycle, ShieldWarning, Timer, Warning } from "@phosphor-icons/react";
import { ItemTile } from "../../components/tile/Tile";
import { Button, Spinner } from "../../components/ui/primitives";
import { toast } from "../../components/ui/toast";
import { relativeTime } from "../../lib/format";
import { api, errorMessage } from "../../lib/ipc";
import type { BreachReport, HealthItemRef, HealthReport, SecurityStatus } from "../../lib/types";
import { useApp } from "../../state/app";
import { useVault } from "../../state/vault";
import s from "./SecurityScreen.module.css";

function openItem(id: string, edit = false) {
  const st = useVault.getState();
  st.setView({ type: "all" });
  st.setQuery("");
  st.select(id);
  if (edit) st.edit({ mode: "edit", id });
}

function ItemRow({ item, detail, action = "Trocar senha" }: { item: HealthItemRef; detail?: ReactNode; action?: string }) {
  return (
    <li className={s.row}>
      <ItemTile kind={item.kind} title={item.title} urls={item.urls} size={32} />
      <button className={s.rowMain} onClick={() => openItem(item.id)}>
        <strong>{item.title}</strong>
        <span>{detail ?? item.subtitle}</span>
      </button>
      <div className={s.rowActions}>
        {item.urls[0] && (
          <Button size="sm" variant="ghost" onClick={() => api.openUrl(item.urls[0])}>
            Abrir site
          </Button>
        )}
        <Button size="sm" onClick={() => openItem(item.id, true)}>
          {action}
        </Button>
      </div>
    </li>
  );
}

function Section({
  icon,
  title,
  count,
  tone,
  why,
  children,
  defaultOpen = true,
}: {
  icon: ReactNode;
  title: string;
  count: number;
  tone: "danger" | "warn" | "info";
  why: ReactNode;
  children: ReactNode;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  if (count === 0) return null;
  return (
    <section className={s.section} data-tone={tone}>
      <button className={s.sectionHead} onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        <span className={s.sectionIcon}>{icon}</span>
        <span className={s.sectionTitle}>
          {title}
          <span className={s.badge}>{count}</span>
        </span>
        <CaretDown size={14} className={s.caret} data-open={open || undefined} />
      </button>
      {open && (
        <>
          <p className={s.why}>{why}</p>
          <ul className={s.list}>{children}</ul>
        </>
      )}
    </section>
  );
}

export function SecurityScreen() {
  const [report, setReport] = useState<HealthReport | null>(null);
  const [status, setStatus] = useState<SecurityStatus | null>(null);
  const [breach, setBreach] = useState<BreachReport | null>(null);
  const [breachBusy, setBreachBusy] = useState(false);
  const settings = useApp((st) => st.settings);
  const updateSettings = useApp((st) => st.updateSettings);
  const setScreen = useVault((st) => st.setScreen);
  const items = useVault((st) => st.items);

  useEffect(() => {
    let alive = true;
    Promise.all([api.health(), api.security()])
      .then(([r, st]) => {
        if (!alive) return;
        setReport(r);
        setStatus(st);
      })
      .catch((e) => toast(errorMessage(e), { tone: "danger" }));
    return () => {
      alive = false;
    };
  }, [items]);

  const runBreach = async (enable: boolean) => {
    setBreachBusy(true);
    try {
      if (enable && !settings?.breachCheck) await updateSettings({ breachCheck: true });
      setBreach(await api.breachCheck());
    } catch (e) {
      toast(errorMessage(e), { tone: "danger" });
    } finally {
      setBreachBusy(false);
    }
  };

  if (!report || !status || !settings) {
    return (
      <div className={s.loading}>
        <Spinner size={18} /> Olhando seu Mocó…
      </div>
    );
  }

  const attention = report.weak.length + report.reused.length + report.insecureSites.length + (breach?.hits.length ?? 0) + report.expiring.filter((f) => (f.days ?? 0) < 0).length;
  const accountIssues = [!status.recoveryEnabled, settings.autoLockMinutes === 0 || settings.autoLockMinutes > 60, !settings.screenCaptureProtection].filter(Boolean).length;
  const byId = new Map(items.map((i) => [i.id, i]));

  return (
    <div className={s.root}>
      <header className={s.head}>
        <h1 className={s.title}>Central de segurança</h1>
        <p className={s.summary}>
          {attention + accountIssues === 0 ? (
            <>
              <CheckCircle size={20} weight="fill" className={s.ok} /> Tudo certo por aqui.
            </>
          ) : (
            <>
              {attention + accountIssues === 1 ? "Uma coisa merece" : `${attention + accountIssues} coisas merecem`} sua atenção. Comece pelas de cima.
            </>
          )}
        </p>
        <p className={s.meta}>
          {report.passwords.toLocaleString("pt-BR")} senhas em {report.totalItems.toLocaleString("pt-BR")} itens · verificado {relativeTime(report.checkedAt)} · tudo analisado neste computador
        </p>
      </header>

      <div className={s.columns}>
        <div className={s.findings}>
          {breach && breach.hits.length > 0 && (
            <Section
              icon={<ShieldWarning size={18} weight="fill" />}
              title="Senhas que já vazaram"
              count={breach.hits.length}
              tone="danger"
              why="Estas senhas aparecem em vazamentos de dados públicos. Quem tenta invadir contas testa essas listas primeiro — troque-as, mesmo que o vazamento tenha sido em outro site."
            >
              {breach.hits.map((h) => {
                const it = byId.get(h.itemId);
                return it ? (
                  <ItemRow
                    key={h.itemId + h.field}
                    item={{ id: it.id, title: it.title, kind: it.kind, subtitle: it.subtitle, urls: it.urls }}
                    detail={`Apareceu ${h.count.toLocaleString("pt-BR")} ${h.count === 1 ? "vez" : "vezes"} em vazamentos`}
                  />
                ) : null;
              })}
            </Section>
          )}

          <Section
            icon={<Recycle size={18} />}
            title="Senhas repetidas"
            count={report.reused.length}
            tone="danger"
            why="Quando um site vaza, a mesma senha abre todos os outros. Cada conta precisa da sua. O gerador resolve isso em segundos."
          >
            {report.reused.map((g, i) => (
              <li key={i} className={s.group}>
                <p className={s.groupLabel}>Mesma senha em {g.items.length} itens</p>
                <ul className={s.list}>
                  {g.items.map((it) => (
                    <ItemRow key={it.id} item={it} />
                  ))}
                </ul>
              </li>
            ))}
          </Section>

          <Section
            icon={<Password size={18} />}
            title="Senhas fracas"
            count={report.weak.length}
            tone="warn"
            why="Senhas curtas, comuns ou com padrões previsíveis caem rápido em tentativas automáticas."
          >
            {report.weak.map((f) => (
              <ItemRow key={f.item.id + f.field} item={f.item} detail={f.detail} />
            ))}
          </Section>

          <Section
            icon={<Globe size={18} />}
            title="Sites sem conexão segura"
            count={report.insecureSites.length}
            tone="warn"
            why="O endereço salvo começa com http://. Se o site aceitar https://, atualize o endereço; se não aceitar, evite digitar senhas nele em redes públicas."
          >
            {report.insecureSites.map((f) => (
              <ItemRow key={f.item.id} item={f.item} detail={f.detail} action="Editar endereço" />
            ))}
          </Section>

          <Section
            icon={<Timer size={18} />}
            title="Vencendo ou vencidos"
            count={report.expiring.length}
            tone="info"
            why="Documentos, cartões e licenças com validade nos próximos 90 dias."
          >
            {report.expiring.map((f) => (
              <ItemRow key={f.item.id} item={f.item} detail={f.detail} action="Atualizar" />
            ))}
          </Section>

          <Section
            icon={<Timer size={18} />}
            title="Senhas antigas"
            count={report.old.length}
            tone="info"
            defaultOpen={false}
            why="Faz mais de três anos que estas senhas não mudam. Se forem fortes e únicas, não há urgência — vale trocar as de contas importantes."
          >
            {report.old.map((f) => (
              <ItemRow key={f.item.id} item={f.item} detail={f.detail} />
            ))}
          </Section>

          {report.weak.length + report.reused.length + report.insecureSites.length + report.expiring.length === 0 && (
            <div className={s.clear}>
              <CheckCircle size={22} weight="fill" className={s.ok} />
              <div>
                <strong>Nenhuma senha fraca ou repetida.</strong>
                <p>Cada conta com sua própria senha forte. É exatamente assim que deve ser.</p>
              </div>
            </div>
          )}

          <section className={s.breachCard}>
            <div>
              <h2>Vazamentos de dados</h2>
              {settings.breachCheck && breach ? (
                <p>
                  {breach.hits.length === 0
                    ? `Nenhuma das suas ${breach.checked} senhas aparece em vazamentos conhecidos.`
                    : `${breach.hits.length} de ${breach.checked} senhas aparecem em vazamentos.`}
                </p>
              ) : (
                <p>
                  Confira se alguma senha já apareceu em vazamentos públicos. Só os 5 primeiros caracteres de uma impressão digital
                  de cada senha saem do computador — a senha em si nunca sai. Consulta feita no Have I Been Pwned.
                </p>
              )}
            </div>
            <Button variant={settings.breachCheck ? "secondary" : "primary"} loading={breachBusy} onClick={() => runBreach(true)} icon={<ShieldWarning size={16} />}>
              {settings.breachCheck ? (breach ? "Verificar de novo" : "Verificar agora") : "Verificar vazamentos"}
            </Button>
          </section>
        </div>

        <aside className={s.account}>
          <h2>Proteção da conta</h2>
          <ul>
            <Check
              ok={status.recoveryEnabled}
              icon={<Lifebuoy size={18} />}
              title={status.recoveryEnabled ? "Código de recuperação ativo" : "Sem código de recuperação"}
              detail={
                status.recoveryEnabled
                  ? `Criado ${status.recoveryCreatedAt ? relativeTime(status.recoveryCreatedAt) : ""}. Guarde longe do Kit de Emergência.`
                  : "Se esquecer a senha mestra, não haverá como recuperar seus dados."
              }
              action={!status.recoveryEnabled ? { label: "Criar código", run: () => setScreen("settings") } : undefined}
            />
            <Check
              ok={settings.autoLockMinutes > 0 && settings.autoLockMinutes <= 60}
              icon={<LockSimple size={18} />}
              title={settings.autoLockMinutes === 0 ? "Nunca tranca sozinho" : `Tranca após ${settings.autoLockMinutes} min sem uso`}
              detail={settings.lockOnSessionLock ? "Também tranca quando o Windows é bloqueado." : "Não tranca quando o Windows é bloqueado."}
              action={settings.autoLockMinutes === 0 || settings.autoLockMinutes > 60 ? { label: "Ajustar", run: () => setScreen("settings") } : undefined}
            />
            <Check
              ok={status.deviceKeys.includes("windows-hello")}
              neutral
              icon={<Fingerprint size={18} />}
              title={status.deviceKeys.includes("windows-hello") ? "Windows Hello ativado" : "Windows Hello desativado"}
              detail="Abra o Mocó com rosto, digital ou PIN, sem enfraquecer a senha mestra."
              action={!status.deviceKeys.includes("windows-hello") ? { label: "Configurar", run: () => setScreen("settings") } : undefined}
            />
            <Check
              ok={settings.screenCaptureProtection}
              icon={<Key size={18} />}
              title={settings.screenCaptureProtection ? "Protegido contra captura de tela" : "Captura de tela permitida"}
              detail="Esconde o Mocó de prints, gravações e compartilhamento de tela."
            />
            <Check
              ok={!!status.passwordChangedAt}
              neutral
              icon={<Password size={18} />}
              title="Senha mestra"
              detail={status.passwordChangedAt ? `Definida ${relativeTime(status.passwordChangedAt)}.` : ""}
              action={{ label: "Trocar", run: () => setScreen("settings") }}
            />
          </ul>
        </aside>
      </div>
    </div>
  );
}

function Check({
  ok,
  neutral,
  icon,
  title,
  detail,
  action,
}: {
  ok: boolean;
  neutral?: boolean;
  icon: ReactNode;
  title: string;
  detail: string;
  action?: { label: string; run: () => void };
}) {
  return (
    <li className={s.check} data-state={ok ? "ok" : neutral ? "neutral" : "warn"}>
      <span className={s.checkIcon}>{ok ? <CheckCircle size={18} weight="fill" /> : neutral ? icon : <Warning size={18} weight="fill" />}</span>
      <div className={s.checkText}>
        <strong>{title}</strong>
        <span>{detail}</span>
        {action && (
          <button className={s.checkAction} onClick={action.run}>
            {action.label} <ArrowRight size={12} weight="bold" />
          </button>
        )}
      </div>
    </li>
  );
}
