import { GeneratorPanel } from "../../components/generator/GeneratorPanel";
import s from "./GeneratorScreen.module.css";

export function GeneratorScreen() {
  return (
    <div className={s.root}>
      <header className={s.head}>
        <h1 className={s.title}>Gerador de senhas</h1>
        <p className={s.sub}>
          Uma senha forte em um clique. Tudo é gerado aqui no seu computador, com o gerador aleatório do próprio Windows.
        </p>
      </header>
      <div className={s.panel}>
        <GeneratorPanel big />
      </div>
      <aside className={s.tips}>
        <h2>Qual escolher?</h2>
        <dl>
          <dt>Caracteres</dt>
          <dd>Para sites e apps. Você nunca vai digitar: o Mocó copia para você.</dd>
          <dt>Frase</dt>
          <dd>Para o que precisa ser digitado ou lembrado — o Wi-Fi da visita, o computador do trabalho.</dd>
          <dt>PIN</dt>
          <dd>Para cartões, cadeados e apps que só aceitam números.</dd>
        </dl>
      </aside>
    </div>
  );
}
