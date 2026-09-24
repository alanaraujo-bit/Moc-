// Printable Emergency Kit and Recovery Sheet. Rendered into #print-root and printed with
// the system dialog (which also offers "Salvar como PDF"). Nothing is written to disk by us.

import { createPortal } from "react-dom";
import { MocoMark } from "../../components/brand/Brand";
import s from "./Printable.module.css";

function today() {
  return new Intl.DateTimeFormat("pt-BR", { dateStyle: "long" }).format(new Date());
}

export function EmergencyKitSheet({ secretKey }: { secretKey: string }) {
  return (
    <article className={s.sheet}>
      <header className={s.head}>
        <MocoMark size={40} />
        <div>
          <h1>Kit de Emergência</h1>
          <p>Criado em {today()}</p>
        </div>
      </header>
      <section className={s.block}>
        <h2>Sua Chave Secreta</h2>
        <p className={s.code}>{secretKey}</p>
        <p className={s.note}>
          Você vai precisar dela para abrir o Mocó em um computador novo, junto com a sua senha mestra.
        </p>
      </section>
      <section className={s.block}>
        <h2>Sua senha mestra</h2>
        <div className={s.writeLine} />
        <p className={s.note}>Se quiser, anote à mão. Não é obrigatório — mas, se anotar, este papel vira tão valioso quanto o próprio cofre.</p>
      </section>
      <section className={s.cols}>
        <div>
          <h3>Onde guardar</h3>
          <p>Impresso, longe do computador. Junto dos documentos importantes da casa costuma ser um bom lugar.</p>
        </div>
        <div>
          <h3>O que ele não faz</h3>
          <p>Sozinha, a Chave Secreta não abre nada: sempre precisa da senha mestra também.</p>
        </div>
      </section>
      <footer className={s.foot}>
        O Mocó não guarda cópia da sua senha mestra nem da sua Chave Secreta. Se as duas se perderem, não há como recuperar o
        acesso.
      </footer>
    </article>
  );
}

export function RecoverySheetPrint({ code }: { code: string }) {
  return (
    <article className={s.sheet}>
      <header className={s.head}>
        <MocoMark size={40} />
        <div>
          <h1>Folha de Recuperação</h1>
          <p>Criada em {today()}</p>
        </div>
      </header>
      <section className={s.block}>
        <h2>Código de Recuperação</h2>
        <p className={s.code}>{code}</p>
        <p className={s.note}>
          Serve para criar uma senha mestra nova se você esquecer a atual. Funciona uma vez; depois de usado, o Mocó gera outro.
        </p>
      </section>
      <section className={s.cols}>
        <div>
          <h3>Guarde separado</h3>
          <p>Não deixe junto do Kit de Emergência. Em lugares diferentes, perder um não compromete o outro.</p>
        </div>
        <div>
          <h3>Para usar</h3>
          <p>Na tela de desbloqueio, escolha “Esqueci a senha mestra” e digite este código.</p>
        </div>
      </section>
    </article>
  );
}

export function PrintPortal({ children }: { children: React.ReactNode }) {
  const host = document.getElementById("print-root");
  return host ? createPortal(children, host) : null;
}
