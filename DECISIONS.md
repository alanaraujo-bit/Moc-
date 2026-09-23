# Decisões do Mocó

Registro das decisões de arquitetura, segurança e produto. Cada decisão tem contexto,
escolha e consequência. Decisões novas entram no fim; decisões revistas são marcadas,
nunca apagadas.

---

## D-001 · Stack do desktop: Tauri 2 + núcleo em Rust + React/TypeScript

**Contexto.** Gerenciador de senhas precisa de: abertura rápida, pouco consumo de memória,
integração real com Windows (Hello, bandeja, clipboard, atalhos globais, instalador),
e um núcleo criptográfico que depois rode em extensão (WASM), Android e iOS (UniFFI).

**Escolha.** Tauri 2 (WebView2) para a casca desktop. Todo segredo e toda criptografia vivem
no processo Rust (`crates/moco-core`), nunca no JavaScript. A interface React só recebe o que
precisa exibir; senhas só atravessam o IPC quando o usuário revela/edita, e a cópia para o
clipboard é feita pelo Rust.

**Consequências.** Binário pequeno (~poucos MB), sem Node no runtime, superfície de ataque
menor que Electron. O mesmo `moco-core` será reaproveitado pelos outros clientes.

## D-002 · Monorepo

```
apps/desktop          React + Vite (UI) e src-tauri (casca nativa)
crates/moco-core      criptografia, modelo de itens, armazenamento, gerador, importadores
apps/server           (futuro próximo) API de sincronização zero-knowledge — Rust/axum + Postgres (Railway)
apps/web              (futuro) site, downloads, changelog, endpoint de atualização (Vercel)
docs/                 whitepaper de segurança, modelo de ameaças, guias
```
Build do Cargo vai para `../moco-target` (caminho ASCII; a pasta do repo tem "ó").

## D-003 · Modelo de chaves (2SKD: senha mestra + Chave Secreta)

**Modelo de ameaças considerado.**
1. *Arquivo do cofre copiado* (backup, disco roubado, pasta sincronizada). O atacante tem o
   ciphertext e pode tentar força bruta offline na senha mestra.
2. *Vazamento do servidor* (quando houver sincronização). Mesmo cenário, em escala.
3. *Servidor malicioso/comprometido* adulterando ou trocando dados.
4. *Malware no dispositivo desbloqueado* — fora do alcance de qualquer gerenciador;
   mitigamos (clipboard, auto-bloqueio, proteção de captura), sem prometer o impossível.

**Escolha.** Derivação em duas partes, o desenho publicado e auditado do 1Password (2SKD):

- `Chave Secreta`: 128 bits aleatórios gerados no dispositivo na criação do cofre
  (formato `A1-XXXXXX-XXXXXX-XXXXX-XXXXX-XXXXX-XXXXX`, com versão e checksum). Guardada no
  Windows protegida por DPAPI (ligada ao usuário do Windows). Nunca vai ao servidor.
- `pw = Argon2id(NFKD(senha_mestra), salt, m=64 MiB, t=3, p=4)` (parâmetros salvos junto do
  salt; podem ser elevados depois).
- `prk = HKDF-Extract(salt = chave_secreta, ikm = pw)`
- `MUK = HKDF-Expand(prk, "moco/v1/unlock")` — desembrulha a Chave da Conta.
- `AUTH = HKDF-Expand(prk, "moco/v1/auth")` — prova de login no servidor (futuro). Separação
  total entre autenticação e criptografia: o servidor nunca recebe algo que decifre dados.

Com isso, quem roubar só o arquivo do cofre (cenário 1) ou o banco do servidor (cenário 2)
precisa adivinhar 128 bits aleatórios além da senha: força bruta impraticável mesmo para
senhas fracas.

**Custo aceito.** Em um dispositivo novo o usuário precisa da Chave Secreta (Kit de
Emergência ou QR code de outro dispositivo). No uso diário ela é invisível. Se a DPAPI falhar
(perfil do Windows recriado), o Mocó pede a Chave Secreta do Kit, com mensagem clara.

**Hierarquia.**
- `Chave da Conta` (AK, 256 bits aleatórios) — raiz. Embrulhada por: MUK; Código de
  Recuperação (se ativo); chave derivada do Windows Hello (por dispositivo).
- `Par X25519` da conta (compartilhamento futuro) — privada cifrada pela AK.
- `Chave do Cofre` (VK, 256 bits por cofre) — embrulhada pela AK (cofres próprios) ou por
  *sealed box* para a chave pública do destinatário (cofres compartilhados).
- Itens cifrados com a VK. Cada item tem dois blobs: *overview* (título, subtítulo, URLs,
  tags, favorito — para lista e busca) e *details* (campos, notas, TOTP, histórico).

## D-004 · Primitivas criptográficas (nada inventado)

- AEAD: XChaCha20-Poly1305 (nonce aleatório de 192 bits) — crate `chacha20poly1305`.
- KDF de senha: Argon2id — crate `argon2`. KDF de chaves: HKDF-SHA256 — `hkdf`, `sha2`.
- Assimétrico: X25519 + sealed box compatível com libsodium — `crypto_box`.
- Aleatoriedade: `OsRng` (BCryptGenRandom no Windows).
- Segredos em memória: `zeroize`/`Zeroizing`; tipos secretos com `Debug` redigido.
- **Dados associados (AD)** em todo AEAD: versão do formato + propósito + IDs (cofre, item,
  revisão). Impede trocar o blob de um item por outro, reverter tipo, ou mover entre cofres.
- Todo envelope cifrado começa com `versão || suite`, permitindo migração futura.
- Crates escolhidos na geração estável e amplamente auditada (RustCrypto 0.10/0.12/0.5), não
  nas versões recém-lançadas.

## D-005 · Código de Recuperação e Kit de Emergência

- O **Kit de Emergência** (PDF gerado localmente) contém a Chave Secreta e, se o usuário
  mantiver ativo, o **Código de Recuperação** (160 bits, Base32 legível). O código embrulha
  uma cópia da AK e permite redefinir a senha mestra.
- O kit diz claramente: *com ele, dá para abrir seu Mocó — guarde como um passaporte*.
- Usuários mais cautelosos podem desativar o código: então esquecer a senha significa perder
  os dados, e o Mocó diz isso sem rodeios.
- Nunca afirmamos que "o Mocó recupera sua senha". Não recupera; não consegue ver.

## D-006 · Armazenamento local e offline-first

- SQLite (`rusqlite` bundled) em `%LOCALAPPDATA%\app.moco.desktop\`.
- Colunas em claro apenas: IDs, revisão, timestamps, tombstone. Títulos e URLs também são
  cifrados.
- Cada escrita incrementa a revisão do item e grava uma entrada na *outbox* — a
  sincronização futura só consome a outbox; não haverá migração de esquema para ligá-la.
- O app funciona 100% sem internet. Falta de rede é estado previsto.

## D-007 · Clipboard

Cópia feita no Rust com os formatos do Windows que excluem o conteúdo do histórico
(Win+V) e da nuvem: `ExcludeClipboardContentFromMonitorProcessing`,
`CanIncludeInClipboardHistory = 0`, `CanUploadToCloudClipboard = 0`. Limpeza automática
(padrão 90 s) somente se o clipboard ainda contém o valor copiado pelo Mocó.

## D-008 · Webview endurecida

CSP estrita sem origens remotas; nenhum conteúdo remoto carregado na webview; capacidades
Tauri mínimas; devtools desativadas em release. Ícones de sites (opcional, desligado por
padrão) são baixados pelo Rust diretamente do domínio do item, nunca por serviço de terceiros.

## D-009 · Ao bloquear, esquecer tudo

Bloquear zera as chaves no Rust (zeroize) e recarrega a webview, descartando todo estado JS.
Não basta esconder a interface.

## D-010 · Dados de demonstração separados

Para QA visual no navegador existe um backend simulado em TypeScript, incluído somente com
`VITE_MOCO_DEMO=1` em modo de desenvolvimento. Builds de produção não contêm esse código.
