// Generates apps/web/seguranca.html (from docs/security.md) and apps/web/novidades.html
// (from the app's changelog), so the site never drifts from the product.
// Usage: node scripts/web/build.mjs
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const web = join(root, "apps", "web");

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const inline = (s) =>
  esc(s)
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, t, h) => {
      const href = h.startsWith("../") ? "https://github.com/alanaraujo-bit/Moc-/blob/main/" + h.slice(3) : h;
      return `<a href="${href}">${t}</a>`;
    });

function markdown(md) {
  const out = [];
  const lines = md.split(/\r?\n/);
  let para = [], list = null, table = null;
  const flush = () => {
    if (para.length) out.push(`<p>${inline(para.join(" "))}</p>`);
    para = [];
    if (list) out.push(`<${list.tag}>${list.items.map((i) => `<li>${inline(i)}</li>`).join("")}</${list.tag}>`);
    list = null;
    if (table) {
      const [head, , ...rows] = table;
      const cells = (r) => r.replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
      out.push(
        `<table><thead><tr>${cells(head).map((c) => `<th>${inline(c)}</th>`).join("")}</tr></thead><tbody>` +
          rows.map((r) => `<tr>${cells(r).map((c) => `<td>${inline(c)}</td>`).join("")}</tr>`).join("") +
          "</tbody></table>",
      );
    }
    table = null;
  };
  for (const line of lines) {
    let m;
    if (!line.trim()) { flush(); continue; }
    if ((m = line.match(/^(#{1,3}) (.*)$/))) { flush(); out.push(`<h${m[1].length}>${inline(m[2])}</h${m[1].length}>`); continue; }
    if (line.startsWith("|")) { if (!table) flush(); table ??= []; table.push(line); continue; }
    if ((m = line.match(/^(-|\d+\.) (.*)$/))) {
      if (para.length) flush();
      const tag = m[1] === "-" ? "ul" : "ol";
      if (!list || list.tag !== tag) { flush(); list = { tag, items: [] }; }
      list.items.push(m[2]);
      continue;
    }
    if (list && /^\s+/.test(line)) { list.items[list.items.length - 1] += " " + line.trim(); continue; }
    if (list) flush();
    para.push(line.trim());
  }
  flush();
  return out.join("\n");
}

const page = (title, description, body) => `<!doctype html>
<html lang="pt-BR">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${title}</title>
    <meta name="description" content="${description}" />
    <link rel="icon" href="/img/moco.svg" type="image/svg+xml" />
    <link rel="stylesheet" href="/style.css" />
  </head>
  <body>
    <header class="top">
      <div class="wrap">
        <a class="brand" href="/"><img src="/img/moco.svg" alt="" />mocó</a>
        <nav class="nav" aria-label="Principal">
          <a href="/seguranca" class="hide-sm">Segurança</a>
          <a href="/novidades" class="hide-sm">Novidades</a>
          <a href="https://github.com/alanaraujo-bit/Moc-" class="hide-sm">Código</a>
          <a class="btn small" href="/baixar">Baixar</a>
        </nav>
      </div>
    </header>
    <main class="doc">
      <div class="wrap">
${body}
      </div>
    </main>
    <footer>
      <div class="wrap">
        <span>© 2026 Mocó</span>
        <a href="/seguranca">Segurança e privacidade</a>
        <a href="/novidades">Novidades</a>
        <a href="https://github.com/alanaraujo-bit/Moc-/releases">Todas as versões</a>
      </div>
    </footer>
  </body>
</html>
`;

// Security
const sec = readFileSync(join(root, "docs", "security.md"), "utf8");
writeFileSync(
  join(web, "seguranca.html"),
  page(
    "Segurança e privacidade — Mocó",
    "Como o Mocó protege o que você guarda: o que é cifrado, o que o servidor vê e o que acontece em cada situação ruim.",
    markdown(sec),
  ),
);

// Changelog: pull the CHANGELOG literal out of the app's TS source.
const ts = readFileSync(join(root, "apps", "desktop", "src", "lib", "changelog.ts"), "utf8");
const literal = ts.slice(ts.indexOf("= [", ts.indexOf("CHANGELOG")) + 2, ts.indexOf("];", ts.indexOf("CHANGELOG")) + 1);
const releases = Function(`return ${literal}`)();
const fmt = (iso) => new Date(iso + "T12:00:00").toLocaleDateString("pt-BR", { day: "numeric", month: "long", year: "numeric" });
const body =
  `<h1>Novidades</h1>\n<p>O que mudou em cada versão do Mocó. O app se atualiza sozinho — atualizações de segurança chegam primeiro.</p>\n` +
  releases
    .map(
      (r) => `<section class="release" id="v${r.version}">
  <header><span class="v">${r.version}</span><h2 class="title">${esc(r.title)}</h2><time datetime="${r.date}">${fmt(r.date)}</time></header>
  <ul>${r.highlights.map((h) => `<li><strong>${esc(h.title)}.</strong> ${esc(h.body)}</li>`).join("")}</ul>
</section>`,
    )
    .join("\n");
writeFileSync(join(web, "novidades.html"), page("Novidades — Mocó", "O que mudou em cada versão do Mocó.", body));
console.log("ok: seguranca.html, novidades.html,", releases.length, "versões");
