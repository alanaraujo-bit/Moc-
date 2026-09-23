# Product

<!-- impeccable:product-schema 1 -->

> Written from the founder's brief (2026-09-23). The founder delegated every product,
> design and engineering decision and asked not to be interrupted; facts marked
> *(inferido)* were inferred by the builder from the brief, not confirmed in an interview.

## Platform

web

Desktop app for Windows 11/10 built on Tauri 2 (WebView2). The interface is web technology
inside a native shell, but it must behave like a native Windows app: real titlebar
behavior, keyboard-first, tray, global shortcuts, Windows Hello. Browser extension,
Android and iOS come later and are first-class products, not reduced versions.

## Stack

delegated: Tauri 2 + Rust core (`crates/moco-core`, owns all cryptography) + React 19 +
TypeScript + Vite. Chosen for small footprint, fast launch, keys never in JavaScript, and a
core reusable by extension (WASM) and mobile (UniFFI). Sync server: Rust (axum) + Postgres on
Railway. Website and update feed: Vercel.

## Users

- Brazilians of any technical level who today keep passwords in a notebook, a notes app,
  the browser, or their memory — and know it isn't right. They open Mocó many times a day
  for a few seconds: find, copy, go back to what they were doing.
- Power users (developers, sysadmins, freelancers) who also store servers, databases, API
  keys, SSH keys, licenses and want keyboard speed. *(inferido: split between these two
  audiences comes from the brief's "acessível para não técnicos / poderoso para avançados")*
- Families and small teams sharing some items (later phase).

## Product Purpose

A place where important things are kept and can be found immediately: passwords,
documents, cards, identities, notes, keys. Success is the feeling "minhas coisas
importantes estão guardadas aqui e eu sei exatamente como encontrá-las", a first vault
created in minutes without help, and daily use that takes seconds.

## Positioning

- Zero-knowledge by construction: master password + device-generated Secret Key (2SKD);
  everything, including titles and URLs, encrypted before it touches disk or network.
- Local-first: the vault is fully useful offline; sync is additive.
- Made for Brazil in the details: CPF/CNH/RG, Pix keys, agência/conta, convênio, placa and
  RENAVAM are first-class fields; copy written in natural Brazilian Portuguese; accents in
  master passwords handled correctly (NFKD).
- Honest about recovery: says plainly what can and cannot be recovered.

## Operating Context

- Used alongside a browser and other apps, often mid-task (login screen open, form half
  filled). Quick copy, auto-clearing clipboard, global shortcut and a quick-access window
  matter more than browsing.
- Windows desktop at typical DPI (100–150%), light and dark system themes both common.
- First contact: installer → onboarding → create vault → import from browser/other manager.
- Printed Emergency Kit and a separate recovery sheet are physical artifacts of the product.

## Capabilities and Constraints

- Item kinds: login, password, card, identity, document, secure note, Wi-Fi, software
  license, bank account, server, database, API credential, SSH key, crypto wallet, health
  plan, vehicle, custom — each with its own fields and actions.
- Organization: vaults, nested tags, favorites, recents, archive, trash (30 days), smart
  collections, instant fuzzy accent-insensitive search, command palette.
- Password generator (characters, passphrases in Portuguese, PINs), TOTP, security center
  (weak, reused, old, breached via k-anonymity, missing 2FA), import/export, Windows Hello,
  auto-lock, clipboard protection, professional installer and auto-updates.
- Terminology (PT-BR): "Mocó" (the whole thing), "cofre" (vault), "item", "senha mestra",
  "Chave Secreta", "Kit de Emergência", "Código de Recuperação", "lixeira", "arquivados".
- Undecided: pricing, plan limits, billing provider, domain (see BLOCKERS.md).

## Brand Commitments

- Name: Mocó — hideout, a safe little corner where you keep what matters. Also the name of
  a small rock cavy of the Brazilian Caatinga that lives in rock crevices.
- Voice: natural Brazilian Portuguese; clear, human, short, occasionally funny ("Tá
  guardado.", "Pode esquecer. O Mocó lembra."). Humor never touches security, loss,
  authentication or recovery messages — those are plain and professional.
- Must avoid: neon, cyberpunk, HUD, hacker aesthetic, giant padlocks, exaggerated
  gradients, gratuitous glassmorphism, particles, futuristic UI, SaaS-template look,
  dashboards of useless charts, excess borders, Brazilian caricature (flags, green/yellow,
  tropical clichés, forced humor).
- Security should feel like calm, not paranoia.

## Evidence on Hand

- No customers, testimonials, press, benchmarks or prices exist yet. None may be
  fabricated. Demonstration data is allowed only in the separate demo mode and is labeled.

## Product Principles

1. Simple on the surface, powerful underneath — complexity appears only when asked for.
2. Seconds matter: find → copy → back to work is the core loop; optimize it above all.
3. Security is the foundation, not a feature page; never trade it for convenience.
4. Tell the truth: what we can see, what we can't, what can be recovered, what can't.
5. Never trap the user: import is easy and export is complete.

## Accessibility & Inclusion

Keyboard-complete operation, visible focus, screen-reader labels, WCAG AA contrast in both
themes, reduced-motion support, status never conveyed by color alone, text that survives
Windows text scaling. *(inferido from the brief's accessibility section)*
