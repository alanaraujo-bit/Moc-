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
    version: "0.2.0",
    date: "2026-09-24",
    title: "Seu Mocó em todos os seus computadores",
    highlights: [
      { title: "Sincronização", body: "Ative em Configurações › Sincronização. Tudo sai daqui já cifrado: o servidor guarda, mas não consegue ler." },
      { title: "Entrar em outro computador", body: "Na primeira tela, escolha “Já uso o Mocó em outro computador” e use seu e-mail, a senha mestra e a Chave Secreta." },
      { title: "Anexos", body: "Arraste fotos de documentos, contratos e comprovantes para dentro de um item. Eles ficam cifrados junto com ele." },
      { title: "Wi-Fi para visitas", body: "Itens de Wi-Fi mostram um QR code: a câmera do celular conecta sem ninguém digitar a senha." },
    ],
  },
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
