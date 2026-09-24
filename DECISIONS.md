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
  (formato `M1-XXXXXX-XXXXXX-XXXXXX-XXXXXX-XXXX`, Crockford Base32 com checksum). Guardada no
  Windows protegida por DPAPI (ligada ao usuário do Windows). Nunca vai ao servidor.
- `pw = Argon2id(NFKD(senha_mestra), salt, m=64 MiB, t=3, p=4)` (parâmetros salvos junto do
  salt; podem ser elevados depois).
- `prk = HKDF-Extract(salt = chave_secreta, ikm = pw)`
- `MUK = HKDF-Expand(prk, "moco/v1/unlock" ‖ account_id)` — desembrulha a Chave da Conta.
- Login no servidor (futuro): semente `HKDF-Expand(prk, "moco/v1/auth-ed25519" ‖ account_id)` →
  par Ed25519; o servidor guarda só a chave pública e o cliente assina um desafio
  (`"moco/v1/login" ‖ nonce ‖ account_id`). Nada que o servidor receba decifra dados, e não
  existe token de portador reutilizável derivado da senha.

Com isso, quem obtiver **só o arquivo do cofre** (backup, pasta sincronizada) ou **o banco do
servidor** precisa adivinhar 128 bits aleatórios além da senha: força bruta impraticável mesmo
para senhas fracas.

*Limite honesto (revisão M5):* a Chave Secreta fica no mesmo computador protegida pela DPAPI,
cuja força é a do login do Windows. Quem leva o disco **e** consegue o login do Windows (ou,
em máquinas de domínio, a chave de backup DPAPI do domínio) não enfrenta os 128 bits — só o
Argon2id. Evolução planejada: embrulhar a Chave Secreta com chave não exportável do TPM
(Microsoft Platform Crypto Provider), com DPAPI por fora; DPAPI pura só sem TPM, e a UI diz isso.

**Custo aceito.** Em um dispositivo novo o usuário precisa da Chave Secreta (Kit de
Emergência ou QR code de outro dispositivo). No uso diário ela é invisível. Se a DPAPI falhar
(perfil do Windows recriado), o Mocó pede a Chave Secreta do Kit, com mensagem clara.

**Hierarquia.**
- `Chave da Conta` (AK, 256 bits aleatórios) — raiz. Embrulhada por: MUK; Código de
  Recuperação (se ativo); chave derivada do Windows Hello (por dispositivo).
- `Identidade` da conta: X25519 (receber chaves de cofre via HPKE) + Ed25519 (assinar) —
  privadas cifradas pela AK, geradas já na criação da conta.
- `Chave do Cofre` (VK, 256 bits por cofre, com geração `key_gen`) — embrulhada pela AK
  (cofres próprios). Cofres compartilhados (futuro): HPKE (RFC 9180) com assinatura Ed25519 do
  remetente; chaves de cofre pessoal só são aceitas como embrulho pela AK.
- Itens cifrados com a VK. Cada item tem dois blobs: *overview* (título, subtítulo, URLs,
  tags, favorito — para lista e busca) e *details* (campos, notas, TOTP, histórico).

## D-004 · Primitivas criptográficas (nada inventado)

- AEAD: XChaCha20-Poly1305 (nonce aleatório de 192 bits) — crate `chacha20poly1305`.
- KDF de senha: Argon2id — crate `argon2`. KDF de chaves: HKDF-SHA256 — `hkdf`, `sha2`.
- Assimétrico: X25519 (`x25519-dalek` 2) e Ed25519 (`ed25519-dalek` 2, `verify_strict`);
  curve25519-dalek ≥ 4.1.3. HPKE (RFC 9180) entra com o compartilhamento.
- Aleatoriedade: `OsRng` (BCryptGenRandom no Windows).
- Segredos em memória: `zeroize`/`Zeroizing`; tipos secretos com `Debug` redigido.
- **Envelope autodescritivo** (revisão M3): todo ciphertext carrega um cabeçalho autenticado de
  96 bytes — formato, suíte, propósito, flags, conta, contêiner, objeto, id/geração da chave,
  versão do objeto e `write_id` — e o AD *é* esse cabeçalho. O leitor confere o cabeçalho
  contra o contexto esperado antes de decifrar. Colunas de sincronização (sequência do servidor,
  timestamps) nunca entram no AD: são cursores, não identidade.
- **Compromisso de chave**: `k_enc ‖ commit = HKDF(ikm=K, salt=nonce, info="moco/v1/aead"‖hdr[0..4])`;
  `commit` é verificado em tempo constante antes de decifrar.
- **Overview e details** de uma escrita compartilham `write_id` e versão — misturar blobs de
  escritas diferentes é rejeitado. A coluna `revision` é só cache da versão autenticada.
- **Lixeira definitiva** gera *tombstone* autenticado (propósito próprio), não só uma flag.
- **Padding**: plaintexts de itens vão para faixas de 256 B até 4 KiB, depois Padmé.
- Crates escolhidos na geração estável e amplamente auditada (RustCrypto 0.10/0.12/0.5), não
  nas versões recém-lançadas.

## D-005 · Código de Recuperação e Kit de Emergência

- O **Kit de Emergência** contém a Chave Secreta (e espaço para anotar a senha à mão, se a
  pessoa quiser). Sozinho, não abre nada.
- O **Código de Recuperação** (144 bits + checksum, Crockford Base32) vai numa **folha
  separada** (revisão M4). A chave que ele gera exige também a Chave Secreta:
  `HKDF-Extract(salt = Chave Secreta, ikm = código)` → `"moco/v1/recovery-wrap"‖account_id`.
  Assim a folha de recuperação perdida sozinha não abre nada.
- O código é de uso único: após uma recuperação, um novo é gerado e o antigo deixa de valer.
- Ao salvar PDFs, o Mocó avisa se a pasta é sincronizada com nuvem (OneDrive etc.).
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

## D-011 · Revisão de segurança do desenho (2026-09-23)

Uma revisão independente do desenho criptográfico apontou 5 itens obrigatórios e 9
recomendações. Estado:

| Item | Decisão |
|---|---|
| M1 compartilhamento autenticado | Identidade Ed25519 criada já; HPKE + assinatura quando o compartilhamento chegar |
| M2 rollback/remoção por servidor malicioso | Versão autenticada + tombstones autenticados agora; log encadeado por cofre (hash chain com MAC) e estado da conta com contador monotônico entram com a sincronização |
| M3 AD a partir de cabeçalho autenticado | Feito (envelope v1) |
| M4 kit com código de recuperação | Feito: folha separada + recuperação exige Chave Secreta + uso único |
| M5 afirmação exagerada sobre DPAPI | Corrigido no texto; TPM planejado |
| S1 login sem token de portador | Adotado (Ed25519 derivado); implementa com o servidor |
| S2 TPM | Planejado para a fase do Windows Hello |
| S3 compromisso de chave | Feito |
| S4 armadilhas do Windows Hello | Checklist na implementação: assinar 2×, HKDF com salt do dispositivo, fallback de senha sempre, senha mestra a cada 14 dias |
| S5 higiene de memória | Campo de senha não controlado e limpo após o IPC; ao bloquear, a webview é recarregada |
| S6 fluxos de ciclo de chave | Troca de senha com salt novo e upgrade de KDF no desbloqueio: feitos |
| S7 anexos e exportação | Chave por arquivo, streaming em blocos de 64 KiB; exportação cifrada por padrão |
| S8 vazamento de tamanho | Padding feito; busca só em memória |
| S9 crates | Ajustado (sem crypto_box; dalek 2; normalização fixada) |

## D-012 · Atualizações e releases

- `tauri-plugin-updater` com assinatura minisign: o app só instala pacotes assinados pela
  chave do Mocó; a chave pública está em `tauri.conf.json`.
- Canais: **estável** lê `releases/latest/download/latest.json`; **beta** lê o release
  contínuo `beta`, atualizado pelo workflow a cada tag `vX.Y.Z-beta.N`.
- Atualização **crítica** = notas do release contêm `[critica]`: aparece um diálogo; as
  demais são só uma faixa discreta na barra lateral. Nunca reinicia sem o usuário pedir.
- Instalar tranca o cofre antes de reiniciar. Falha no download ou na instalação não muda nada.
- Rollback: publicar uma versão maior com o código anterior (o updater não instala versões
  menores). Releases antigos ficam no GitHub para instalação manual.
- Pipeline: `.github/workflows/ci.yml` (testes no Windows a cada push) e `release.yml`
  (tag → testes → instalador NSIS + artefatos do updater → GitHub Release).

## D-013 · Acesso Rápido e integração com o Windows

- Atalho global (padrão Ctrl+Shift+Espaço) abre uma janela sem bordas, sempre no topo, que
  some ao perder o foco. Enter copia o segredo principal; Shift+Enter o usuário; Ctrl+Enter
  o código 2FA; Alt+Enter abre o site.
- "Abrir com o Windows" registra o app com `--hidden`: começa trancado, na bandeja.
- Fechar a janela esconde na bandeja (configurável); a bandeja oferece Abrir, Acesso rápido,
  Trancar agora e Sair.

## D-014 · Servidor de sincronização

- `apps/server` (Rust, axum + Postgres) no Railway: `https://server-production-975b.up.railway.app`
  (provisório até ter domínio). Guarda só o que os dispositivos já cifraram.
- **Login sem segredo reutilizável:** o cliente deriva, do mesmo Argon2id que abre o cofre, uma
  chave Ed25519; o servidor guarda a pública e o cliente assina um desafio de uso único. O
  token de sessão é aleatório, guardado no servidor só como hash, ligado a um dispositivo e
  revogável; no computador ele fica selado sob a Chave da Conta.
- **Pré-login** responde a e-mails inexistentes com valores falsos estáveis (não revela quem
  tem conta). Limites de taxa por IP e por conta.
- **Sincronização** por concorrência otimista (versão base) e cursor de sequência por conta;
  o cliente autentica cada linha antes de aplicar e recusa versões menores que a maior já
  vista (proteção contra rollback). Conflito: a edição mais recente vence, a outra vai para o
  histórico; edição vence exclusão.
- **Troca de senha** envia registro + nova chave de login numa transação e derruba as outras
  sessões. **Remover dispositivo** corta a sincronização dele — o que já estava salvo nele
  continua lá, cifrado (a interface diz isso).
- **2FA (TOTP)** opcional para entrar em dispositivos novos, com 8 códigos de emergência
  guardados como hash. O segredo TOTP fica cifrado no servidor com a chave do servidor.
- **Pendente:** verificação de e-mail (precisa de provedor de e-mail — BLOCKERS), anexos na
  nuvem, recuperação por código em dispositivo novo (hoje a recuperação é local).

## D-015 · Cofres compartilhados

- **Onde ficam os dados:** o cofre e os itens continuam na conta de quem criou. Cada pessoa
  recebe um convite (`ShareGrant`): a chave do cofre selada para a chave X25519 dela (HPKE
  simples: X25519 efêmero + HKDF + envelope) e assinada com a Ed25519 de quem compartilhou.
  O servidor guarda e repassa o convite, mas não consegue abri-lo nem forjá-lo.
- **Chaves fixadas no primeiro uso:** o servidor poderia entregar a chave pública errada.
  O app fixa a chave de cada pessoa na primeira vez e bloqueia o compartilhamento se ela
  mudar, até o usuário confirmar; os dois lados podem comparar o número de segurança.
- **Papéis:** editor e leitor. Todo membro lê (tem a chave); **a proibição de escrita do
  leitor é garantida pelo servidor**, que confere a assinatura do convite para aplicar
  exatamente o papel que o dono assinou.
- **Remover alguém troca a chave do cofre** (geração +1): tudo é cifrado de novo e quem fica
  recebe convite novo. O servidor recusa envios selados com geração abaixo da atual
  (cabeçalho do envelope, bytes 52..72) e convites antigos reenviados.
- **Mover item para o cofre de outra pessoa** cria uma cópia lá e apaga o original (os
  cabeçalhos ligam cada objeto à conta dona).
- **Servidor** (`apps/server/src/sharing.rs`): `GET /v1/people?email=` (revela se o e-mail
  tem conta — só logado, limitado por conta e por IP), membros em `/v1/vaults/{v}/members`,
  `GET /v1/shared` e `/v1/shared/{dono}/{cofre}/{pull,push,attachments}` autorizados por
  participação. Anexos só de itens do próprio cofre, na cota do dono. Apagar o cofre apaga
  as participações. Regras de referência: `FakeCloud` no núcleo.

## D-016 · Android

- **Mesmo app Tauri, não um projeto separado.** `apps/desktop` compila para Android
  (`src-tauri/gen/android`, pacote `app.moco.android`, Android 9+). O que só existe no
  computador (bandeja, instância única, atalho global, iniciar com o sistema, updater,
  Windows Hello) fica atrás de `cfg(desktop)`. O núcleo Rust é o mesmo, byte a byte.
- **Chave Secreta no aparelho:** Android Keystore, AES-256-GCM, chave que nunca sai do
  hardware, amarrada ao aparelho destravado quando há bloqueio de tela seguro (sem bloqueio
  de tela o Keystore recusa esse vínculo; aí a chave continua presa ao hardware). O
  identificador da conta vai como dado autenticado, como a entropia no DPAPI.
- **Interface:** um "casco" de celular (`src/mobile`) com uma tela por vez, reaproveitando
  detalhe, editor, login, sincronização e configurações. O botão voltar do Android percorre
  essa pilha; no topo, o app vai para segundo plano (fechar a activity derrubava o WebView).
- **Trancar:** não existe "ocioso" no celular; conta o tempo em segundo plano
  (`autoLockMinutes`). Capturas de tela bloqueadas (FLAG_SECURE) no release.
- **Fora desta etapa:** biometria (precisa ser ligada à chave com CryptoObject, como o
  Hello, nunca um sim/não), preenchimento automático (Autofill Service), anexos e
  importação/exportação no celular.
