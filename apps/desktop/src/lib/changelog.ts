// "Novidades no Mocó" — shown once after an update. Newest first. Keep entries short,
// written for people, not for developers.

export interface Release {
  version: string;
  date: string; // ISO
  title: string;
  highlights: { title: string; body: string }[];
}

export const CHANGELOG: Release[] = [
  {
    version: "0.1.0",
    date: "2026-09-24",
    title: "O primeiro Mocó",
    highlights: [
      { title: "Seu cofre, só seu", body: "Tudo é cifrado neste computador com a sua senha mestra e a Chave Secreta. Nem nós conseguimos abrir." },
      { title: "Acesso rápido", body: "Ctrl + Shift + Espaço abre uma busca por cima de qualquer programa. Enter copia a senha." },
      { title: "Traga tudo", body: "Importe do navegador, Bitwarden, 1Password, LastPass, Proton Pass, KeePass ou de uma planilha." },
      { title: "Central de segurança", body: "Encontra senhas fracas, repetidas e vazadas, e mostra o que fazer." },
    ],
  },
];

export function releaseOf(version: string): Release | undefined {
  return CHANGELOG.find((r) => r.version === version);
}
