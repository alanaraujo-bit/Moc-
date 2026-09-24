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

### Próximos (ordem)
1. QA: tema escuro, Acesso Rápido, estados vazios, janela estreita; paleta de comandos.
2. Instalador com identidade (imagens NSIS), atualizações automáticas assinadas,
   "Novidades no Mocó", pipeline de release no GitHub Actions.
3. Anexos cifrados; QR code do Wi-Fi; favicons opcionais.
4. Servidor de sincronização (Railway) + contas + dispositivos + 2FA.
5. Compartilhamento (HPKE), famílias/equipes, planos.
6. Site/downloads/changelog (Vercel). Whitepaper de segurança em `docs/`.
7. Revisão de acessibilidade, performance com 10 mil itens, DESIGN.md.

## Log
- 2026-09-23 — Repositório criado, pipeline de build/instalador validado, núcleo cripto.
- 2026-09-24 — UI completa do cofre, Hello, importação/exportação, segurança, acesso rápido.
