---
name: dtop
description: Docker container monitoring in your terminal.
colors:
  bg: "#0a0e17"
  bg-elevated: "#111827"
  bg-card: "#141c2b"
  surface: "#1a2332"
  border: "#1e293b"
  border-bright: "#2d3d52"
  text: "#e2e8f0"
  text-muted: "#8899aa"
  text-dim: "#4a5e73"
  accent: "#00ff88"
  accent-dim: "rgba(0, 255, 136, 0.15)"
  terminal-bg: "#0d1117"
  tag-blue: "#38bdf8"
  tag-purple: "#a78bfa"
  tag-orange: "#fb923c"
  tag-cyan: "#22d3ee"
typography:
  display:
    fontFamily: "Orbitron, sans-serif"
    fontSize: "clamp(2rem, 5vw, 4rem)"
    fontWeight: 800
    lineHeight: "0.9"
    letterSpacing: "-0.02em"
  hero:
    fontFamily: "Orbitron, sans-serif"
    fontSize: "clamp(2.5rem, 6vw, 5rem)"
    fontWeight: 800
    lineHeight: "1"
    letterSpacing: "-0.02em"
  title:
    fontFamily: "Orbitron, sans-serif"
    fontSize: "clamp(1.6rem, 2.8vw, 2.4rem)"
    fontWeight: 700
    lineHeight: "0.95"
    letterSpacing: "-0.01em"
  body:
    fontFamily: "DM Sans, sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: "1.65"
    letterSpacing: "normal"
  mono:
    fontFamily: "JetBrains Mono, monospace"
    fontSize: "0.875rem"
    fontWeight: 500
    lineHeight: "1.5"
    letterSpacing: "normal"
  label:
    fontFamily: "JetBrains Mono, monospace"
    fontSize: "0.7rem"
    fontWeight: 500
    lineHeight: "1.4"
    letterSpacing: "0.22em"
rounded:
  none: "0"
  sm: "2px"
  demo: "12px"
spacing:
  xs: "0.5rem"
  sm: "1rem"
  md: "1.5rem"
  lg: "3rem"
  xl: "6rem"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.bg}"
    rounded: "{rounded.none}"
    padding: "0.5rem 1.25rem"
  button-ghost:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.text-muted}"
    rounded: "{rounded.none}"
    padding: "0.5rem 0.75rem"
  card:
    backgroundColor: "{colors.bg-card}"
    textColor: "{colors.text}"
    rounded: "{rounded.none}"
    padding: "1.25rem"
  card-hover:
    backgroundColor: "{colors.bg-card}"
    textColor: "{colors.text}"
    rounded: "{rounded.none}"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.none}"
    padding: "0.625rem 0.625rem"
  kbd:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.none}"
    padding: "0.375rem 0.625rem"
---

# Design System: dtop

## 1. Overview

**Creative North Star: "The Documented Binary"**

`dtop` is a terminal program. The docs page is the program's manual page with a video clip pasted in. Every visual choice answers to that. The page is dense, left-aligned where it can be, sharp-edged everywhere (no rounded corners except the demo video frame), and runs at a low visual idle, the same way the binary runs at a low CPU idle.

The system rejects the developer-tool marketing reflex: gradient-mesh heroes, three identical icon-cards, glassmorphism navbars, "loved by teams at" logo strips, neon-on-black drenched in glow shadows. It also rejects the opposite reflex, the retro-terminal costume: VT323 pixel display fonts, drenched amber CRT pastiche, ASCII art that decorates rather than informs. `dtop` is a real terminal tool, not a mood board for one. Visual choices have to follow from the binary, not be applied to it.

Density beats spectacle. Show the demo video, list the flags, ship the install lines. Prose is connective tissue.

**Key Characteristics:**
- Sharp-edged surfaces (radius 0). The only rounded element on the page is the demo video frame.
- Restrained color: tinted dark navy neutrals plus one terminal-green accent.
- Sectioned like a printed reference: numbered `§ NN / Topic` kickers, one repeating header pattern.
- Display type from Orbitron at extreme weight (800) for headlines; mono for everything technical.
- Motion is restrained: fade-up reveals on scroll, blinking cursor, glowing status dot. Nothing choreographed, nothing scroll-driven, nothing parallaxed.

### The Page Shell

Behind every section sits a fixed-position dot-grid (`@utility dot-grid` in `app.css`): a 32×32 radial-gradient pattern in `--c-border` color, masked to an ellipse centered roughly on the hero. Opacity is theme-tuned: `0.4` in dark, `0.2` in light. It's the page's only persistent decorative element. Reads as graph paper or a CRT scan field, depending on how hard you squint, and is intentional: it gives section content something subtle to sit on top of without ever being seen.

A film-grain overlay (`body::before`) sits above everything at very low opacity (`0.028` dark / `0.015` light) for warmth on flat surfaces. Both shells stay; new sections inherit them automatically.

## 2. Colors

A restrained dark-navy system tinted toward blue, with one terminal-green accent and a small categorical palette used only for list-item differentiation in CLI / Install / Config blocks.

### Primary

- **Terminal Green** (`#00ff88`): The single load-bearing accent. Used for the prompt cursor, kicker labels (`§ NN / Topic`), italicized words inside section headlines, hover affordances, the install `$ dtop_` strip, and feature-row numerals. Light theme darkens it to `#00a85a` for AA contrast on cream.

### Neutral

- **Deep Navy** (`#0a0e17`): The page background. Tinted blue, not pure black. Carries the dot-grid radial pattern.
- **Elevated Navy** (`#111827`): One step up. Used for kbd chips' resting surface and hover surfaces on shortcut rows.
- **Card Navy** (`#141c2b`): Container surface for install / flag / config blocks.
- **Surface Navy** (`#1a2332`): Chip-internal surface (kbd interiors, inline code chips inside paragraphs).
- **Border** (`#1e293b`): The primary divider weight. Used for section borders, card borders, header rules.
- **Bright Border** (`#2d3d52`): The emphasized weight. Used under section kicker rows and around the install final CTA.
- **Bone** (`#e2e8f0`): Primary text. Reads warm against the navy.
- **Slate** (`#8899aa`): Secondary text, paragraph copy in offset asides.
- **Dim** (`#4a5e73`): Tertiary text. Mono labels, footer links at rest, decorative `$` prompt chars.

### Tertiary (categorical only — not a brand palette)

- **Tag Blue** (`#38bdf8`), **Tag Purple** (`#a78bfa`), **Tag Orange** (`#fb923c`), **Tag Cyan** (`#22d3ee`): Used exclusively as 8px dots on install-method rows, CLI flag rows, and config block headers, to give visual differentiation in long repeating lists. They are not used for type, borders, fills, or icons. They do not appear in the hero, the features, the navbar, or the footer.

### Named Rules

**The One Voice Rule.** Terminal Green (`#00ff88`) is the single accent. It carries the cursor, the kickers, the italicized headline word, and hover/focus states. No other color does headline duty.

**The Categorical Palette Stays in Lists.** Tag colors (blue / purple / orange / cyan) appear only as 8px dots inside repeating list items. They never escape into headings, body type, borders, or backgrounds. They are role markers, not part of the brand.

**Syntax-highlighting carve-out.** Inside a code preview (`<pre>` block rendering YAML, JSON, shell, etc.), the four tag colors may carry token roles (keys / strings / booleans / numbers / comments). This is the only sanctioned use of categorical color on type. Inline code chips and code lines outside a preview block must still resolve to `text-(--c-text)` or `text-(--c-accent)`.

**The Tinted-Neutral Rule.** Every background, border, and "gray" is tinted blue (hue ≈ 220–225). Pure black, pure white, and pure gray are forbidden. The light theme tints toward warm cream (`#f4f6f9`), not paper white.

## 3. Typography

**Display Font:** Orbitron (with sans-serif fallback)
**Body Font:** DM Sans (with sans-serif fallback)
**Mono / Label Font:** JetBrains Mono (with monospace fallback)

**Character:** Orbitron is geometric and angular at heavy weight (800), reading as confident-but-technical. DM Sans does the unobtrusive body work. JetBrains Mono carries everything that wants to feel like terminal output: kickers, kbd chips, code, file paths, command lines. The pairing leans technical without becoming brutalist.

### Hierarchy

- **Hero** (Orbitron 800, `clamp(2.5rem, 6vw, 5rem)`, line-height 1): The page-top headline ("Docker Monitoring / In Your Terminal"). Used once.
- **Display** (Orbitron 800, `clamp(2rem, 5vw, 4rem)`, line-height 0.9): Section headlines under the kicker pattern. Two-line shape with the second line italicized in accent color.
- **Title** (Orbitron 700, `clamp(1.6rem, 2.8vw, 2.4rem)`, line-height 0.95): Feature row titles ("Multi-Host Support."), config table heading.
- **Numeral** (Orbitron 800, `clamp(3rem, 7vw, 5.5rem)`, line-height 1, accent color): The `01 / 02 / 03` markers in the features section. Treated as display-scale even though it's two characters.
- **Body** (DM Sans 400, 1rem, line-height 1.65): Paragraph copy. Constrained to ≤75ch via column spans (md:col-span-3 within max-w-300 → ~50ch in offset asides; md:col-span-7 → ~70ch in feature bodies).
- **Mono Code** (JetBrains Mono 500, 0.875rem, line-height 1.5): Inline code, command lines in install methods, YAML highlight in config blocks.
- **Label** (JetBrains Mono 500, 0.7rem, uppercase, letter-spacing 0.22em): Kickers (`§ 01 / Capabilities`), tag/meta straplines (`topology / ssh · tcp · tls · local`), kbd content, footer links.

### Named Rules

**The Italic-Accent Rule.** In every section headline, the second line is italicized in Terminal Green. The break comes from a `<br />` and the italic is paired with `text-(--c-accent)`. Example: "Three things, *done well.*". This is the system's only sanctioned use of italic display type.

**The Uppercase-Mono Rule.** Anything secondary, navigational, or metadata-shaped is set in JetBrains Mono, uppercase, with letter-spacing 0.22em, at 0.7rem. This includes section kickers, tag/meta straplines, kbd text, and footer link labels. Never use this style for sentence-shaped copy.

**The No-Restated-Subhead Rule.** Section headlines are not followed by a paragraph that re-states them in plain language. The right-column aside under each headline must add information, not paraphrase the heading.

## 4. Elevation

The system is **flat by default**. No drop shadows, no card lift, no glassmorphism. Depth is built from tinted-navy layers stepping up: `bg → bg-elevated → bg-card → surface`. A border is preferred over a shadow.

### Shadow Vocabulary (limited, purposeful)

- **kbd shadow** (`box-shadow: 0 2px 0 var(--c-border)`): A flat 2px-down keycap shadow on `<kbd>` elements only. Suggests a physical key, not a 3D card.
- **Demo frame shadow** (`box-shadow: 0 0 0 1px rgba(0,0,0,0.3), 0 24px 80px -12px rgba(0,0,0,0.6), 0 0 120px -40px var(--c-accent-dim)`): Used once, on the hero demo video container. The accent-dim outer glow is the only place the page emits accent-colored light. Reserved exclusively for that element.
- **Glow ring** (`shadow-[0_0_40px_-10px_var(--c-accent-dim)]` or similar): Forbidden on buttons, install CTAs, navbars, and pills. The glow ring is the demo frame's signature. Reusing it elsewhere dilutes it.

### Named Rules

**The Flat-By-Default Rule.** Surfaces are flat. Containers are bordered, not elevated. The kbd 2px keycap shadow and the hero demo frame are the only sanctioned shadows on the page.

**The Glow-Goes-on-the-Demo Rule.** The accent-dim outer glow is reserved for the demo video frame. It will never appear on a button, a CTA strip, a navbar, or a pill.

## 5. Components

### Buttons

- **Shape:** Sharp rectangles (radius 0). No pill buttons, no rounded buttons.
- **Primary** (Install CTA): Solid `bg-(--c-accent)`, text `text-(--c-bg)`, `font-mono` uppercase letter-spacing 0.22em, `px-5 py-2`. Hover transitions to `bg-(--c-accent)/90`. No glow, no lift.
- **Ghost** (Theme toggle, mobile menu, copy buttons): Bordered (`border-(--c-border-bright)`), `text-(--c-text-muted)`, no fill at rest. Hover swaps border to `border-(--c-text-muted)` and text to `text-(--c-text)`. Same `transition-colors`, never `transition-all`.
- **Link buttons** (Install / Reference text links): No background, accent underline 2px, `font-mono` uppercase 0.22em letter-spacing. Hover changes color only.

### Section Header (signature component)

Every section below the hero uses the same shape:
- 12-column grid, `gap-x-4 md:gap-x-6`, `border-b border-(--c-border-bright) pb-6 mb-12 md:mb-16`
- **Left column** (col-span-12 md:col-span-2): Mono uppercase kicker `§ NN / Topic` in `text-(--c-accent)`, letter-spacing 0.22em, font-size 0.7rem.
- **Center column** (col-span-12 md:col-span-7): Display headline, two lines, second line italicized in accent: `Two-word lead,<br /><em>third-word punchline.</em>`
- **Right column** (col-span-12 md:col-span-3): Body-sized aside (`text-sm leading-relaxed text-(--c-text-muted)`) that adds information instead of restating the headline.

### Cards / Containers

- **Corner Style:** Square (radius 0). No rounded cards.
- **Background:** `bg-(--c-bg-card)` (`#141c2b`), one step above the page background.
- **Border:** Always bordered (`border border-(--c-border)`). Never relies on shadow for definition.
- **Hover:** Border brightens to `border-(--c-border-bright)` via `transition-colors`. Background is not lifted.
- **Internal padding:** `p-5` for list-item cards (install methods, CLI flags, config blocks), `px-8 py-6 md:px-10` for table-shaped containers (config locations, keyboard shortcuts).
- **Header strip:** Where a card has a title strip, it's separated by a 1px `border-b border-(--c-border)` and uses a 8px colored dot + uppercase mono label.

### Install / CLI / Config List Blocks

A single repeating shape powers the three reference sections. Each row is a bordered card with:
- A 1-px-rule top strip carrying a colored 8px dot (one of the four tag colors) and a label in uppercase mono.
- A body row with `font-mono` content (a shell command, a flag declaration, or a YAML excerpt) and a copy-to-clipboard ghost button at the right edge.

### kbd (keyboard chip)

- **Background:** `bg-(--c-surface)` (`#1a2332`).
- **Text:** `text-(--c-text)`, JetBrains Mono 500, 0.75rem, `font-medium`.
- **Border:** `border border-(--c-border-bright)`.
- **Padding:** `min-w-8 px-2.5 py-1.5` (the `min-w-8` keeps single-character keys square).
- **Shape:** Rectangular (radius 0).
- **Shadow:** `shadow-[0_2px_0_var(--c-border)]` — the only flat 2D-keycap shadow allowed.

### Inputs

The page itself has no form inputs. The TUI does. If inputs are added to this docs surface, they should follow the same shape: bordered, `bg-(--c-surface)`, square corners, no glow, focus ring uses `outline: 2px solid var(--c-accent); outline-offset: 2px` from the global rule.

### Navigation

- **Style:** Sticky, top-of-viewport, **solid** `bg-(--c-bg)`. Bordered with `border-b border-(--c-border)`. No backdrop-blur, no backdrop-saturate. (Glassmorphism is explicitly removed.)
- **Logo:** `font-mono`, prompt-shaped (`$ dtop_`) with the underscore blinking.
- **Links:** Mono 0.8rem, `text-(--c-text-muted)`, hover `text-(--c-text)`.
- **Theme / menu buttons:** Ghost-button shape (see Buttons).
- **Install CTA:** Primary button shape.

### Demo Frame (signature component)

The only rounded element (`rounded-xl`, 12px). Carries:
- A faux-titlebar strip with three `size-3` colored circles (`#ff5f57 #febc2e #28c840`) and a centered "dtop" label. (This is the *one* costume element the page allows itself, because the demo IS being captured from a real terminal window.)
- The `demo.mp4` autoplaying loop.
- Boxed in by the only sanctioned glow shadow on the page (see Elevation).

### Footer

- Single `border-t border-(--c-border)` divider. No gradient line, no decorative rule.
- Same `$ dtop` mono logo, separated from the credit line with a `·` middot.
- Links in dim mono, hover-to-text on color only.

## 6. Do's and Don'ts

### Do:

- **Do** use Terminal Green (`#00ff88`) as the only headline-italic / kicker / cursor color. Light theme uses `#00a85a`.
- **Do** keep every container, button, kbd, card, and input at radius 0. The demo frame at 12px is the only exception.
- **Do** open every section below the hero with `§ NN / Topic` in mono uppercase 0.22em letter-spacing 0.7rem accent.
- **Do** italicize the second line of every section headline in accent color, via `<span class="italic text-(--c-accent)">…</span>`.
- **Do** keep paragraphs in DM Sans at body line-height 1.65, capped at ~75ch by column span.
- **Do** keep type tags / kickers / kbd / footer links / file paths in JetBrains Mono.
- **Do** rely on borders, not shadows, to define cards. Hover should shift `border` color, not lift.
- **Do** put the categorical tag colors only on 8px dots inside repeating list-item header strips.
- **Do** use `transition-colors`, not `transition-all`, for hover states.

### Don't:

- **Don't** ship a gradient-mesh hero, a "loved by teams at" logo strip, three identical icon-and-heading cards, or any other AI-driven landing-page archetype. *(From PRODUCT.md: "If the page could be re-skinned for any other dev tool by swapping the noun, it has failed.")*
- **Don't** dress the page in retro-terminal costume: VT323, pixel bitmap display fonts, drenched amber/green CRT pastiche, ASCII art that decorates rather than documents real behavior. *(From PRODUCT.md: "`dtop` is a real terminal tool, not a mood board for one.")*
- **Don't** use `background-clip: text` with a gradient. Headline emphasis comes from italic + accent color, never from gradient text.
- **Don't** add glassmorphism (`backdrop-blur`, `backdrop-saturate`) to the navbar, modals, or cards. The navbar is solid `bg-(--c-bg)`.
- **Don't** use `border-left` greater than 1px as a colored side stripe on cards or callouts. Full 1px borders only.
- **Don't** wrap the install-final `$ dtop_` strip in a glow shadow. The accent-dim glow is reserved for the hero demo frame.
- **Don't** introduce rounded corners on buttons, cards, kbd chips, install rows, flag rows, or config blocks. Only the demo frame is rounded.
- **Don't** restate the section headline in the right-column aside. The aside adds information; it doesn't paraphrase.
- **Don't** use the categorical tag colors (blue / purple / orange / cyan) for type, headings, borders, fills, or icons. They live on 8px dots inside lists.
- **Don't** ship ASCII diagrams, screenshots, or feature lists that imply behavior the binary doesn't have. *(From PRODUCT.md: "Better to ship less copy than to ship copy the code can't back up.")*
- **Don't** add bounce, elastic, or scroll-driven choreographed motion. Motion vocabulary is fade-up reveal, blink, glow-pulse. That's it.
- **Don't** center every section. The numbered specimen pattern is left-aligned grid. The hero is the only intentionally centered surface.
