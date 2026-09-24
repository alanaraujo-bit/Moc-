---
name: Mocó
description: A password manager for Windows set like an azulejo panel, where every item is a tile in a calm glazed wall.
colors:
  cobalt: "#2340a8"
  cobalt-fill-hover: "#1c3591"
  cobalt-soft: "#e7ecfb"
  cobalt-soft-2: "#d6defa"
  cobalt-ink: "#1a2d77"
  focus: "#2f53d6"
  ink: "#111726"
  ink-2: "#454e60"
  ink-3: "#636c7d"
  ink-4: "#8e96a5"
  ink-inverse: "#ffffff"
  bg-app: "#f2f4f7"
  bg-surface: "#ffffff"
  bg-sunken: "#eceff3"
  bg-hover: "#e9ecf1"
  bg-active: "#e1e5ec"
  grout: "#e2e5eb"
  grout-strong: "#cbd1da"
  wall-ground: "#fbfcfe"
  wall-relief: "#e6eaf1"
  wall-edge: "#d9dee7"
  tile-field: "#ffffff"
  danger: "#b3261e"
  danger-soft: "#fcebea"
  warn: "#9a5b00"
  warn-soft: "#fff2d9"
  warn-ink: "#7a4800"
  ok: "#1d7a4b"
  glaze-cobalt: "#2340a8"
  glaze-sky: "#2f74b5"
  glaze-moss: "#2c6e4c"
  glaze-ochre: "#b07513"
  glaze-clay: "#b1452d"
  glaze-plum: "#6e3f84"
  glaze-slate: "#4d5870"
  pw-symbol: "#b1452d"
  dark-bg-app: "#0c0f15"
  dark-bg-surface: "#11151d"
  dark-bg-raised: "#171c26"
  dark-grout: "#222937"
  dark-grout-strong: "#313a4b"
  dark-ink: "#e9ecf3"
  dark-ink-3: "#8e97a9"
  dark-cobalt-fill: "#3552c7"
  dark-cobalt-fill-hover: "#4060d6"
  dark-sel-bg: "#2c46b0"
  dark-cobalt-text: "#8aa0f5"
  dark-focus: "#9fb2ff"
  dark-tile-field: "#161b25"
  dark-wall-ground: "#0e1219"
typography:
  display:
    fontFamily: "Segoe UI Variable Display, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "2.125rem"
    fontWeight: 650
    lineHeight: 1.12
    letterSpacing: "-0.03em"
  headline:
    fontFamily: "Segoe UI Variable Display, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "1.75rem"
    fontWeight: 650
    lineHeight: 1.18
    letterSpacing: "-0.025em"
  title-lg:
    fontFamily: "Segoe UI Variable Display, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "1.5rem"
    fontWeight: 650
    lineHeight: 1.2
    letterSpacing: "-0.02em"
  title:
    fontFamily: "Segoe UI Variable Display, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "1.25rem"
    fontWeight: 600
    letterSpacing: "-0.01em"
  title-sm:
    fontFamily: "Segoe UI Variable Text, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "1rem"
    fontWeight: 650
    letterSpacing: "-0.01em"
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.45
  body-sm:
    fontFamily: "Segoe UI Variable Text, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 400
    lineHeight: 1.45
  label:
    fontFamily: "Segoe UI Variable Text, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 550
  caption:
    fontFamily: "Segoe UI Variable Text, Segoe UI Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 400
    lineHeight: 1.4
  mono:
    fontFamily: "Cascadia Mono, Consolas, ui-monospace, monospace"
    fontSize: "13.5px"
    fontWeight: 400
    letterSpacing: "0.01em"
rounded:
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "12px"
  full: "999px"
spacing:
  s-1: "4px"
  s-2: "8px"
  s-3: "12px"
  s-4: "16px"
  s-5: "20px"
  s-6: "24px"
  s-8: "32px"
  s-10: "40px"
  s-12: "48px"
components:
  button-primary:
    backgroundColor: "{colors.cobalt}"
    textColor: "{colors.ink-inverse}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: "0 14px"
    height: "32px"
  button-primary-hover:
    backgroundColor: "{colors.cobalt-fill-hover}"
  button-primary-dark:
    backgroundColor: "{colors.dark-cobalt-fill}"
    textColor: "#ffffff"
  button-primary-dark-hover:
    backgroundColor: "{colors.dark-cobalt-fill-hover}"
  button-secondary:
    backgroundColor: "{colors.bg-surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    padding: "0 14px"
    height: "32px"
  button-secondary-hover:
    backgroundColor: "{colors.bg-hover}"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.sm}"
    padding: "0 14px"
    height: "32px"
  button-ghost-hover:
    backgroundColor: "{colors.bg-hover}"
    textColor: "{colors.ink}"
  button-danger:
    backgroundColor: "{colors.danger}"
    textColor: "{colors.ink-inverse}"
    rounded: "{rounded.sm}"
    height: "32px"
  input:
    backgroundColor: "{colors.bg-surface}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.sm}"
    padding: "0 11px"
    height: "34px"
  input-lg:
    height: "42px"
  search:
    backgroundColor: "{colors.bg-sunken}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    height: "34px"
  list-row:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    padding: "0 10px"
    height: "52px"
  list-row-hover:
    backgroundColor: "{colors.bg-hover}"
  list-row-selected:
    backgroundColor: "{colors.cobalt}"
    textColor: "#ffffff"
  list-row-selected-dark:
    backgroundColor: "{colors.dark-sel-bg}"
    textColor: "#f3f5fb"
  sidebar-item:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.sm}"
    height: "30px"
  sidebar-item-active:
    backgroundColor: "{colors.bg-surface}"
    textColor: "{colors.ink}"
  segmented:
    backgroundColor: "{colors.bg-sunken}"
    rounded: "{rounded.sm}"
    padding: "2px"
  segmented-active:
    backgroundColor: "{colors.bg-surface}"
    textColor: "{colors.ink}"
    height: "26px"
  tooltip:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ink-inverse}"
    typography: "{typography.caption}"
    rounded: "{rounded.xs}"
    padding: "5px 8px"
  toast:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ink-inverse}"
    rounded: "{rounded.md}"
    padding: "8px 8px 8px 14px"
    height: "40px"
  dialog:
    backgroundColor: "{colors.bg-surface}"
    rounded: "{rounded.lg}"
    padding: "24px 24px 20px"
    width: "460px"
---

# Design System: Mocó

## Overview

**Creative North Star: "The Azulejo Panel"**

Mocó is a wall of glazed tiles in the manner of Athos Bulcão: cool glazed white, oxide cobalt, blue-black ink, and 1px grout lines that are the only structure. Every item the person keeps is one tile set into that wall, a white field with a single solid quarter-disk of glaze in a corner chosen by hash, so each item keeps its own tile for life. You find things by rhythm and search, never through a dashboard of padlocks.

The app is a native Windows 11 tool first. It runs in Segoe UI Variable, it is dense the way a desktop list is dense (52px rows, 14px body), and it is keyboard-first: shortcuts are shown as `kbd` keycaps in the empty detail pane and inside the search field. The operate layout is three quiet panes (sidebar | list of tiles | item detail) separated only by grout. Cobalt is rare and means something: an action, the current selection, focus, or a set tile. Copy is calm, natural Brazilian Portuguese ("Seu Mocó está trancado.", "Escolha um item para ver os detalhes."); security moments are plain, never alarming.

Behind the lock screen and onboarding stands the relief wall: white-on-white tiles, motifs a single value step below the ground, grout between. During onboarding the wall glazes itself in cobalt, tile by tile, as each step is completed, so the person literally builds their panel. At unlock every tile turns a quarter, center outward. The system rejects the navy shield-and-padlock security dashboard and the cream-serif-terracotta "cozy" default, along with everything PRODUCT.md bans (neon, HUD, glassmorphism, gradients, tropical caricature).

**Key Characteristics:**
- Grout (1px rules in `grout` / `grout-strong`) is the only structure; panes and sections are divided, never boxed into rounded cards.
- Items are two-tone tiles: white field, one solid glaze quarter-disk, monogram or kind glyph in ink.
- Selection inverts: the selected row becomes a cobalt ground with white ink.
- The relief wall carries identity on lock, recovery and onboarding; onboarding fills it with cobalt step by step.
- Native Windows type; Cascadia Mono for secrets, codes and card numbers.
- Dark mode is the same panel at night: a real cobalt fill for surfaces, lifted glazes for text only.

## Colors

A restrained, near-monochrome glazed palette: cool white and blue-black ink carry almost everything, one oxide cobalt marks action, selection and state, and seven muted glazes exist only to give tiles their identity.

### Primary
- **Oxide Cobalt** (`cobalt`): the only accent. Primary buttons, the selected list row, the switch when on, the caret, links and URL hosts, active sidebar icons, focused field underline, set tiles in the wall, the brand mark. Hover steps darker (`cobalt-fill-hover`); press dims the fill slightly.
- **Cobalt Wash** (`cobalt-soft`, `cobalt-soft-2`): the 3px focus halo around fields and search, search-match `mark` highlights, text selection, the quarter-disk on the recovery code block, the center tile of the empty-state illustration.
- **Cobalt Ink** (`cobalt-ink`): cobalt-tinted informational text (update notices, sync status, import hints).
- **Focus Blue** (`focus`): the 2px `:focus-visible` outline, offset 2px.

### Tile Glazes
- **Cobalt, Sky, Moss, Ochre, Clay, Plum, Slate** (`glaze-*`): the quarter-disk on item tiles, vault chips in the sidebar, and the face of card items. Ochre is also the favorite star. Glazes are identity, never status and never atmosphere.
- **Clay for symbols** (`pw-symbol`) and cobalt for digits color character classes in revealed passwords.

### Neutral
- **Blue-Black Ink** (`ink`): primary text, tile monograms, tooltip and toast grounds.
- **Slate Ink** (`ink-2`): secondary text, field labels, ledes, ghost buttons, sidebar items at rest.
- **Quiet Ink** (`ink-3`): hints, counts, placeholders, icons at rest. Holds 4.5:1 on white.
- **Faint Ink** (`ink-4`): decorative only (row meta icons, separators in crumbs, field hover border). Never for readable text.
- **Glazed White** (`bg-surface`, `tile-field`): list and detail panes, tile fields, raised surfaces.
- **Second Layer** (`bg-app`): the sidebar, dialog footers, active settings index item.
- **Well** (`bg-sunken`): search field at rest, segmented control track, inline code.
- **Hover / Active** (`bg-hover`, `bg-active`): row and button interaction fills.
- **Grout** (`grout`) and **Strong Grout** (`grout-strong`): every divider, pane edge, tile outline and field border.
- **Wall Ground / Relief / Edge** (`wall-ground`, `wall-relief`, `wall-edge`): the relief wall only.

### Status
- **Danger** (`danger`, `danger-soft`), **Warn** (`warn`, `warn-soft`, `warn-ink`), **Ok** (`ok`): errors, the strength meter, banners, destructive menu items. Always paired with words or an icon.

### Dark mode
The same panel at night (`dark-*` keys, mapped onto the same CSS custom properties under `[data-theme="dark"]`). Surfaces go to deep blue-black (`dark-bg-surface`, `dark-bg-app`), grout darkens, ink lifts. Cobalt splits in two: a **real cobalt fill** (`dark-cobalt-fill`, selection `dark-sel-bg`) for anything that is a filled surface, and a **lifted cobalt** (`dark-cobalt-text`) and lifted glazes for text, icons and small marks only.

### Named Rules
**The One Glaze Rule.** Cobalt is the only accent in the chrome, and every use points at something: an action, the selection, focus, a link or URL host, an active icon, progress (onboarding step dots and bullet icons, set wall tiles), or the brand mark. Cobalt never fills space for atmosphere.

**The Fill vs. Text Rule.** In dark mode, filled cobalt surfaces (primary button, switch, selected row) use the true cobalt fill, never the lifted text cobalt. Card faces use the deep glaze (`color-mix(in srgb, glaze 42%, #0c0f15)`), because lifted glazes are for text only.

**The Not By Color Alone Rule.** Status always carries a word or icon: the strength meter says "Muito forte", field errors carry an icon and a sentence, lock states are named.

## Typography

**Display Font:** Segoe UI Variable Display (with Segoe UI, system-ui)
**Body Font:** Segoe UI Variable Text (with Segoe UI, system-ui)
**Label/Mono Font:** Cascadia Mono (with Consolas, ui-monospace)

**Character:** The native Windows 11 voice, used without costume, so Mocó reads as part of the operating system. Weight carries hierarchy (550 labels, 600 to 650 titles) and headings tighten slightly; Cascadia Mono marks anything secret or exact.

### Hierarchy
- **Display** (650, 34px, 1.12, -0.03em): the onboarding welcome hero only.
- **Headline** (650, 28px, 1.18, -0.025em): onboarding step titles ("Crie sua senha mestra").
- **Title Large** (650, 24px, 1.2, -0.02em): item detail title, lock screen line ("Seu Mocó está trancado."), settings page title.
- **Title** (600, 20px, -0.01em): dialog titles.
- **Title Small** (650, 16px): list header ("Tudo"), empty-state titles, settings section heads (which sit on a grout rule).
- **Body** (400, 14px, 1.45): all running UI text, list row titles at 600. Onboarding ledes run at 16px, 1.55, capped at 60ch.
- **Body Small** (400, 13px): row subtitles, crumbs, secondary buttons in small size.
- **Label** (550, 13px): field labels, button text (buttons use 550 at 14px).
- **Caption** (400 to 650, 12px): hints, errors, counts, sidebar group headings ("Cofres", "Tipos"), detail field-group titles, footers. Sentence case; never uppercase tracking.
- **Mono** (400, 13.5px, 0.01em, ligatures off): passwords, TOTP codes (20px, 0.08em, a half-em gap between triplets, turning danger as the code expires), recovery codes (19px, 0.04em), card numbers.

### Named Rules
**The Sentence Case Rule.** Labels, group headings and buttons are sentence case in the body face. The only uppercase in the app is the printed name and expiry on a card face, because that is how a real card looks.

**The Mono Means Exact Rule.** Cascadia Mono is reserved for secrets and codes the person may have to read or type character by character. It never sets prose or headings.

## Layout

A 4px spacing grid (`s-1` to `s-12`). The operate layout is a three-column grid: sidebar `clamp(208px, 19vw, 248px)`, list `clamp(290px, 29vw, 380px)`, detail takes the rest. Wide screens (settings, generator, security) span the list and detail columns. Settings uses a 220px index plus a scrolling column.

Density is desktop-list density: 52px list rows (40px compact), 30px sidebar items, 32px buttons, 34px fields (42px for the lock and master-password fields). Pane interiors breathe more: the detail pane pads 24 to 28px and caps content at 760px; dialogs pad 24px. Field groups in the detail pane stack as rows divided by grout, label above value.

The lock screen centers a 380px column in a plain square opening cut out of the relief wall (9 by 8 tiles), edged by grout. Onboarding splits into content (min 460px, left, max 480px step width) and the relief wall (right); below 960px the wall shrinks to a 200px strip.

The empty detail pane shows a small 3x3 tile motif and a list of keyboard shortcuts as `kbd` keycaps ("Ctrl F buscar", "Ctrl N novo item", "Ctrl K comandos", "Ctrl L trancar"). Keyboard hints are part of the layout, not a help page.

### Named Rules
**The Grout Is Structure Rule.** Panes, sections and field groups are separated by 1px grout rules (`grout`, or `grout-strong` for a group's top rule). They are never wrapped in rounded, shadowed cards.

## Elevation & Depth

Flat by default. The panes, list and detail are flat surfaces separated by grout, and depth between the sidebar and content comes from the tonal step `bg-app` to `bg-surface`. Shadows appear only on things that sit on top of or stand out from the wall: small lift on controls, and real elevation for floating layers. The one physical object, the card face, casts its own shadow.

### Shadow Vocabulary
- **Lift** (`box-shadow: 0 1px 2px rgb(17 23 38 / 6%), 0 1px 1px rgb(17 23 38 / 4%)`): primary and secondary buttons, the active sidebar item, the active segment.
- **Float** (`box-shadow: 0 4px 12px rgb(17 23 38 / 8%), 0 1px 3px rgb(17 23 38 / 6%)`): tooltips.
- **Overlay** (`box-shadow: 0 16px 40px rgb(17 23 38 / 14%), 0 4px 12px rgb(17 23 38 / 8%)`): menus, toasts, the command palette. Dialogs sit over a scrim (`rgb(17 23 38 / 32%)`).
- **Card object** (`box-shadow: 0 10px 22px rgb(17 23 38 / 18%), 0 2px 5px rgb(17 23 38 / 12%)`): the payment-card face only.

Dark mode uses black shadows at 40 to 55% opacity in the same roles.

### Named Rules
**The Flat Wall Rule.** Nothing set into the wall (panes, rows, field groups, tiles) casts a shadow. Only what floats above it does.

## Shapes

Tiles are square with a softened edge. Item tiles round at 24% of their size (about 8px on a 34px list tile) and draw a 1px inset grout outline; the quarter-disk motif has a radius of 46% of the tile. Controls use gently curved corners: 6px (`sm`) for buttons, fields, sidebar items and segments; 8px (`md`) for list rows, menus, toasts and banners; 12px (`lg`) for dialogs; 4px (`xs`) for keycaps, tooltips and menu items. Pills (999px) are reserved for tags, switches and scrollbar thumbs.

The recurring motif is the Athos Bulcão vocabulary: quarter-disks, half-arches, doorways, parallel bands, rings. It appears in the item tile, the brand mark (a cobalt tile with a white doorway and a pale quarter-arc), the relief wall, the empty-state illustration, the corner of the recovery code block, and the faint arcs on a card face. Kind glyphs are drawn on a 24-unit grid with 1.7 stroke and round caps in the same grammar.

### Named Rules
**The Quarter-Disk Signature Rule.** When a surface needs a mark of identity, it gets one solid quarter-disk in a corner, never a gradient, badge or icon medallion.

## Components

### Buttons
Quiet and native, with a firm cobalt primary.
- **Shape:** gently curved (6px), 32px tall (28px small, 40px large), 14px side padding, 550 weight.
- **Primary:** cobalt fill with white text, lift shadow plus a 1px inner top highlight. Hover steps to `cobalt-fill-hover`; press dims slightly and nudges down 0.5px.
- **Secondary:** white with a `grout-strong` border and lift shadow; hover `bg-hover`.
- **Ghost:** transparent, `ink-2` text; hover `bg-hover` and `ink`.
- **Danger / Danger ghost:** `danger` fill, or `danger` text on transparent with `danger-soft` hover.
- **Icon button:** 30px square (26px small), `ink-3` icon, hover `bg-hover`; active state turns the icon cobalt.
- **Transitions:** 120ms on the exponential ease-out (`cubic-bezier(0.16, 1, 0.3, 1)`). Disabled at 50% opacity.

### Inputs / Fields
- **Style:** white field, 1px `grout-strong` border plus a 1px inner bottom rule (the Windows 11 field underline), 6px radius, 34px tall (42px large), 11px text inset. Label above in 13px/550 `ink-2`; hint below in 12px `ink-3`.
- **Focus:** border turns cobalt, the underline thickens to 2px cobalt, and a 3px `cobalt-soft` halo appears. Bare inputs never draw a second ring.
- **Error:** `danger` border and 2px danger underline, with an icon and a sentence below ("Senha mestra incorreta."). The lock field also shakes once.
- **Adornments:** reveal, suggest and submit actions live inside the field on the right. The lock screen's submit is a 32px cobalt square inside the field.
- **Search:** a sunken well (`bg-sunken`) at rest that turns white with a cobalt border and halo on focus, with a `Ctrl F` keycap hint inside.
- **Selects:** drawn natively flat with a custom 12px chevron in `ink-3`.

### Navigation (sidebar)
- **Style:** the `bg-app` layer, 30px items with 18px icons in `ink-3`, grouped under 12px/600 headings ("Cofres", "Tipos", "Etiquetas"), with tabular counts on the right.
- **Active:** the item lifts onto a white chip (`bg-surface` plus lift shadow), text turns `ink` at 550, the icon turns cobalt.
- **Vault chips:** 12px squares at 3px radius in the vault's glaze.
- The brand row (mark plus "mocó" wordmark) and a lock icon button sit at the top; the full-width primary "Novo item" button sits below it; tools (security center, generator, settings) sit at the bottom above a grout rule.

### Item Tile (signature)
The item's identity. A white `tile-field` square with a 1px grout inset outline, 24% radius, one solid quarter-disk in the item's glaze in a corner picked by hash, and a centered monogram (display face, 700, 46% of size) or kind glyph (56% of size) in ink. It holds its grammar across the sizes the build uses, from 20px in compact pickers through 34px list rows (28px compact) to the 52px detail header. The brand mark is a separate component: a cobalt tile with a white doorway and a pale quarter-arc. In dark mode the field is `dark-tile-field` and the disk takes the lifted glaze.

### List Row
- 52px rows with an 8px radius, a 34px tile (28px compact), a 600-weight title and a 13px `ink-3` subtitle. Meta icons (attachment, key, favorite star in ochre) sit on the right.
- **Hover:** `bg-hover`.
- **Selected (inverted):** cobalt ground (`dark-sel-bg` in dark), white title, 78% white subtitle and meta. Search matches (`mark`) become a 22% white wash inside a selected row and a `cobalt-soft-2` wash elsewhere.
- **Keyboard focus:** a 2px inset focus ring on the selected row when the list has focus.

### Relief Wall (signature)
A canvas of 56 to 64px tiles in `wall-ground` with motifs a value step down (`wall-relief`), edged in `wall-edge`, with grout lines between; some tiles are left plain. It stands behind the lock screen, recovery and onboarding, and leaves a plain opening for content. Onboarding raises its `progress` step by step and each newly set tile glazes in cobalt with an animation. On unlock every tile turns a quarter, center outward (620ms). Reduced motion sets tiles without animating.

### Strength Meter
Four short bars (22 by 6px, 2px radius, 3px gap) in grout that fill with `danger`, `warn` or `ok`, always followed by a word ("Muito forte") and an optional time estimate ("quebraria em séculos").

### Card Face
A payment-card object (85.6 by 54 ratio, 14px radius, max 340px) filled with the item's glaze, white text, faint quarter-arc lines, a grid-drawn chip, a mono number and uppercase printed name. In dark mode it uses the deep glaze mix, not the lifted glaze.

### Overlays
- **Dialog:** 460px (640px wide), 12px radius, 1px grout border, over the scrim; 20px/600 title, `ink-2` description, and a `bg-app` footer strip divided by grout holding right-aligned actions.
- **Menu:** raised surface, 8px radius, grout border, overlay shadow, 32px items at 4px radius, `bg-hover` highlight, keycap shortcuts right-aligned, destructive items in danger.
- **Tooltip:** ink ground, inverse text, 12px, 4px radius, optional keycap.
- **Toast:** ink ground pill-card (8px radius) centered at the bottom, 14px/500, optional action and a countdown ring (clipboard clearing). Danger toasts take the danger fill.

### Controls
- **Switch:** 36 by 20px pill; off is a white track with an `ink-3` outline and `ink-2` thumb; on is cobalt fill with a white thumb.
- **Segmented:** a sunken track with a grout border and 2px padding; the active segment is white with lift shadow ("Sistema | Claro | Escuro").
- **Tag:** a pill with a `grout-strong` border and `ink-2` text ("#pessoal").
- **Keycap (`kbd`):** 11px/600, `grout-strong` border with a 2px bottom edge, 4px radius.

## Do's and Don'ts

### Do:
- **Do** separate panes, sections and field groups with 1px grout rules (`grout`, `grout-strong`) and a tonal step, never with boxes.
- **Do** give every item a tile: white field, one glaze quarter-disk in a hashed corner, monogram or kind glyph in ink.
- **Do** show selection by inversion: cobalt ground, white ink.
- **Do** use the true cobalt fill (`dark-cobalt-fill`, `dark-sel-bg`) for filled surfaces in dark mode, and lifted cobalt and glazes only for text, icons and small marks. Card faces take the deep glaze mix.
- **Do** keep keyboard hints visible as `kbd` keycaps where the action lives (search field, empty detail pane, menus, tooltips).
- **Do** use the relief wall behind lock, recovery and onboarding, and let onboarding glaze it in cobalt step by step.
- **Do** open destructive confirmations with the consequence: "Apagar “Gmail” de vez?" / "Não dá para desfazer. O item, o histórico de versões e as senhas antigas somem deste computador." Reversible actions skip the dialog and offer undo in a toast ("Movido para a lixeira").
- **Do** write calm, short PT-BR copy; plain and professional for security, loss and recovery ("Seu Mocó está trancado.", "Senha mestra incorreta.").
- **Do** pair every status color with a word or icon, and set secrets and codes in Cascadia Mono.
- **Do** keep motion short and state-carrying: 120/180/260ms on the exponential ease-out, and honor reduced motion.

### Don't:
- **Don't** wrap content in rounded, shadowed cards; the only shadowed rectangle in a pane is the payment-card face.
- **Don't** use cobalt or glazes as atmosphere (tinted backgrounds, washes, ornament). Glazes belong to tiles, vault chips, card faces, the favorite star and password character classes; cobalt belongs to the uses the One Glaze Rule lists.
- **Don't** fill surfaces in dark mode with the lifted cobalt (`dark-cobalt-text`) or lifted glazes; they fail as fills and read as pastel.
- **Don't** use shields, giant padlocks, neon, HUD, gradients, glassmorphism, particles, or green-and-yellow Brazilian clichés.
- **Don't** use uppercase tracked labels or eyebrows above headings; group headings are 12px sentence case.
- **Don't** use `ink-4` for readable text.
