# Progresso do Mocó

Diário de bordo para retomar o trabalho a qualquer momento. Leia também `DECISIONS.md`,
`BLOCKERS.md` e `PRODUCT.md`.

## Como rodar

```bash
pnpm install
pnpm dev            # app desktop (Tauri) em modo desenvolvimento
pnpm build          # instalador NSIS em ../moco-target/release/bundle/nsis/
cargo test --workspace
# testes de ponta a ponta do servidor (sem a variável, são pulados em silêncio):
D:/pg/pgsql/bin/pg_ctl -D D:/pg/data -o "-p 55432" start   # Postgres portátil local
TEST_DATABASE_URL=postgres://postgres@localhost:55432/moco_test cargo test -p moco-server
```

### QA automatizado do app real
`bash ../moco-qa/run-dev.sh` sobe o app com DevTools (porta 9222), perfil descartável
(`MOCO_DATA_DIR`) e caminhos fixos para os diálogos nativos (`MOCO_QA_PICK`,
`MOCO_QA_SAVE_DIR`, só em debug). Depois: `node scripts/dev/qa.mjs <script.mjs>` —
scripts de exemplo em `../moco-qa/s*.mjs`, capturas em `../moco-qa/shots/`.
Dados de demonstração: `window.__TAURI_INTERNALS__.invoke("dev_seed")` (só em debug; cria
38 itens pelas APIs reais, com criptografia real).

## Estado

### Feito (validado no app real)
- Núcleo `moco-core`: 2SKD (Argon2id + Chave Secreta), envelope autenticado com
  compromisso de chave, hierarquia de chaves, SQLite cifrado, histórico de versões,
  lixeira com tombstone, outbox, gerador (caracteres/frase PT-BR/PIN), zxcvbn PT-BR, TOTP
  (vetores RFC 6238), saúde do cofre, importadores (Chrome, Firefox, Bitwarden JSON/CSV,
  1Password 1PUX, LastPass, Proton Pass, KeePass, CSV genérico PT/EN), backup `.moco`
  cifrado, CSV à prova de injeção de fórmula. 51 testes.
- Desktop: DPAPI para a Chave Secreta, clipboard fora do histórico/nuvem com limpeza
  condicional, auto-lock (inatividade, bloqueio do Windows, suspensão, minimizar),
  bandeja, instância única, proteção contra captura de tela, throttling de tentativas,
  Windows Hello (TPM, assinatura dupla na ativação, senha a cada N dias), atalho global
  + janela de Acesso Rápido, iniciar com o Windows (na bandeja), verificação HIBP opcional.
- UI (mundo "Painel"/azulejo): onboarding com Kit de Emergência e folha de recuperação
  separada, tela de bloqueio com parede de azulejos, recuperação, três painéis com lista
  virtualizada e busca tolerante a erros, detalhe por tipo (cartão como objeto, TOTP ao
  vivo), editor por tipo com máscaras BR (CPF/CNPJ/CEP/telefone/cartão com validação),
  gerador, Central de Segurança, Configurações, importação guiada, exportação.

- Sincronização real: servidor Rust (axum + Postgres) no Railway
  (`https://server-production-975b.up.railway.app`), login por assinatura Ed25519, 2FA TOTP,
  dispositivos, anexos cifrados na nuvem; motor de sync no núcleo com proteção contra
  rollback/forja e resolução de conflito sem perda. Validado entre dois dispositivos reais.
- Compartilhamento no servidor: busca por e-mail, membros (convite assinado conferido pelo servidor,
  papel e geração de chave), cofres compartilhados com pull/push/anexos por participação; leitor não
  escreve, escrita com chave antiga é recusada, anexos presos ao cofre, apagar o cofre encerra o
  compartilhamento. 7 testes de ponta a ponta contra Postgres real (`TEST_DATABASE_URL`).
- Releases: v0.1.0 publicada pelo pipeline (instalador assinado + latest.json). v0.2.0 em build.
- Escala: 10 mil itens — criar 0,26 s, desbloquear 0,13 s, listar 6 ms.
- Revisão de design independente aplicada (parede vidrada, estrutura por rejunte, azulejos
  bicolores, cobalto real no escuro).
- Android (Tauri mobile, mesmo núcleo Rust): Keystore no lugar do DPAPI, área de transferência
  sensível, FLAG_SECURE, interface de celular (lista → item → edição, folha de tipos, voltar do
  Android, trancar em segundo plano), impressão nativa do Kit. Validado no emulador: criar conta,
  destrancar lendo a Chave Secreta do Keystore, editar, e ida e volta real com o servidor
  (instalação limpa → "Já uso o Mocó" → itens de volta). APK assinado:
  `D:PROJETOSmoco-releasesMoco-0.2.0-android.apk`. Build: `pnpm tauri android build --apk`
  (NDK_HOME, ANDROID_HOME, CARGO_TARGET_DIR=D:moco-target).
- Site público: https://moco-one.vercel.app (`apps/web`, `/baixar` → instalador mais recente; páginas de segurança e novidades geradas por `node scripts/web/build.mjs`).
- Atualização real validada: 0.1.0 instalado → buscou o feed, baixou, conferiu a assinatura, instalou e reabriu como 0.2.0.
- DESIGN.md do app (`apps/desktop/DESIGN.md` + `.impeccable/design.json`).

### Próximos (ordem)
0. Android: app no ar em debug e release (ver Feito). Faltam: desbloqueio por biometria ligado à
   chave (BiometricPrompt + CryptoObject, não um sim/não), Serviço de Preenchimento Automático,
   anexos/importação/exportação no celular, compartilhamento e publicação (BLOCKERS 8).
1. Compartilhamento: núcleo e servidor prontos (no ar). Falta o desktop — cuidados:
   - anexos de itens compartilhados vão por `/v1/shared/{dono}/{cofre}/attachments` (hoje
     `cloud.rs` usa sempre `/v1/attachments`); cofre de leitor não envia nem apaga anexos;
   - erros por cofre (409 chave antiga, Integrity entre rotação e novo convite, chave mudou,
     404 `not_member` → esquecer) não podem derrubar a sync da própria conta;
   - reemitir convites no laço do dono: membro com `keyGen` < `vault_key_gen` ganha convite
     novo (sobrevive a fechar o app entre rotação e convites); papel vindo do que o app sabe;
   - `/v1/people` tem limite de 30/h por conta: buscar só ao confirmar, nunca ao digitar;
   - validar com duas instâncias reais (`MOCO_DATA_DIR`) contra produção, com capturas.
   Depois: famílias/equipes, planos/billing (bloqueado: provedor).
2. Acessibilidade (leitor de tela, foco), verificação de e-mail (bloqueado: provedor).
3. Recuperação por código em dispositivo novo (verificador no servidor).

## Log
- 2026-09-23 — Repositório criado, pipeline de build/instalador validado, núcleo cripto.
- 2026-09-24 — UI completa do cofre, Hello, importação/exportação, segurança, acesso rápido.
- 2026-09-24 — Atualizações assinadas, instalador com identidade, sincronização na nuvem, anexos, v0.1.0/v0.2.0.
- 2026-09-24 — Site no ar (Vercel), DESIGN.md, release 0.2.0 publicada.
