# Progresso do Mocó

Diário de bordo para retomar o trabalho a qualquer momento. Leia também `DECISIONS.md`,
`BLOCKERS.md` e `PRODUCT.md`.

## Como rodar

```bash
pnpm install
pnpm dev            # app desktop (Tauri) em modo desenvolvimento
pnpm build          # instalador NSIS em ../moco-target/release/bundle/nsis/
cargo test --workspace
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
- Releases: v0.1.0 publicada pelo pipeline (instalador assinado + latest.json). v0.2.0 em build.
- Escala: 10 mil itens — criar 0,26 s, desbloquear 0,13 s, listar 6 ms.
- Revisão de design independente aplicada (parede vidrada, estrutura por rejunte, azulejos
  bicolores, cobalto real no escuro).

### Próximos (ordem)
1. Teste real de atualização 0.1.0 → 0.2.0 pelo updater.
2. DESIGN.md (documentador) após o veredito da revisão.
3. Compartilhamento (HPKE + assinatura), famílias/equipes, planos/billing (bloqueado: provedor).
4. Site/downloads/changelog (Vercel). Whitepaper de segurança em `docs/`.
5. Acessibilidade (leitor de tela, foco), verificação de e-mail (bloqueado: provedor).
6. Recuperação por código em dispositivo novo (verificador no servidor).

## Log
- 2026-09-23 — Repositório criado, pipeline de build/instalador validado, núcleo cripto.
- 2026-09-24 — UI completa do cofre, Hello, importação/exportação, segurança, acesso rápido.
- 2026-09-24 — Atualizações assinadas, instalador com identidade, sincronização na nuvem, anexos, v0.1.0/v0.2.0.
