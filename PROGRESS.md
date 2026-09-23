# Progresso do Mocó

Diário de bordo para retomar o trabalho a qualquer momento. Leia também `DECISIONS.md` e
`BLOCKERS.md`.

## Como rodar

```bash
pnpm install
pnpm dev            # app desktop (Tauri) em modo desenvolvimento
pnpm build          # instalador NSIS em ../moco-target/release/bundle/nsis/
cargo test --workspace
```

QA visual do app real: `pwsh scripts/dev/capture-window.ps1 -Process moco -Out shot.png`.

## Estado

### Feito
- [x] Pipeline provado: build release → instalador NSIS → instala em `%LOCALAPPDATA%\Mocó`
      → abre → IPC com o núcleo Rust → desinstala limpo (nome com "ó" ok em tudo).

### Em andamento
- [ ] Núcleo criptográfico (`moco-core`): KDF, AEAD, hierarquia de chaves, armazenamento.

### Próximos (ordem)
1. Fatia vertical: criar cofre → bloquear → desbloquear → adicionar login → buscar → copiar
   senha (Rust, com exclusão do histórico) → limpeza automática → auto-bloqueio.
2. Identidade visual + design system + shell (titlebar, sidebar, lista, item).
3. Onboarding completo + Kit de Emergência.
4. Todos os tipos de item, editor por tipo, campos personalizados, anexos.
5. Gerador de senhas, TOTP, busca avançada, paleta de comandos, Acesso Rápido global.
6. Central de Segurança, importação/exportação, configurações, Windows Hello, bandeja.
7. Instalador com identidade, atualizações automáticas, Novidades, pipeline de release.
8. Servidor de sincronização (Railway), contas, dispositivos, 2FA, compartilhamento.
9. Site/downloads/changelog (Vercel).
10. QA completo, acessibilidade, performance com milhares de itens.

## Log

- 2026-09-23 — Repositório criado, pipeline de build/instalador validado.
