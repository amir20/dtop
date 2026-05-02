# Product

## Register

brand

## Users

Developers who run Docker and would rather stay in a terminal than open a browser dashboard. Comfortable with SSH, multiple hosts, keyboard-driven workflows. They install CLI tools by reflex (Homebrew, Cargo, Nix) and judge a tool by how fast it starts and how little it gets in the way. They will read the GitHub README before they read marketing copy.

The design surface this file describes is the docs site at `./docs/` — a SvelteKit landing page for `dtop`. The actual product is a Rust TUI; it has no web UI of its own.

## Product Purpose

`dtop` is a single-binary, real-time terminal monitor for Docker containers across one or many hosts. CPU, memory, network, logs, container actions, all from inside `tmux`. It exists because the alternatives are either browser dashboards (heavy, configurable, slow) or `docker stats` (raw, single-host, no UI). The landing page exists to convert "I have a terminal open right now" into "I have `dtop` running right now" in under a minute.

Success: a visitor lands, sees the demo, picks an install method, and never has to scroll back up.

## Brand Personality

Fast, terse, easy to use.

The voice is the same as a good `--help` page: confident, technically literate, no marketing throat-clearing. Sentences are short. Words like "blazing-fast" are tolerable because they're true and self-aware; words like "empower," "delightful," "seamless" are not. Show the demo, list the flags, get out of the way.

## Anti-references

- **The AI-driven landing-page archetype.** Gradient-mesh hero, three identical icon-and-heading cards, "loved by teams at" logo strip, "built different" subhead, generic dashboard mockup floating at a 12° angle. If the page could be re-skinned for any other dev tool by swapping the noun, it has failed.
- **Retro-terminal costume.** VT323, pixel bitmap display fonts, drenched amber/green CRT pastiche. `dtop` is a real terminal tool, not a mood board for one. The aesthetic should follow from the product, not be applied to it.
- **SaaS marketing paint.** Hero metrics with fake numbers, comparison tables vs. "Tool A / Tool B / Tool C," pricing tiers (it's free), "Book a demo" CTAs.
- **Reference docs that lie.** ASCII diagrams, screenshots, or feature lists that imply behavior the binary doesn't actually have. Better to ship less copy than to ship copy the code can't back up.

## Positive references

- **htop.** Terminal-native, dense, functional, no chrome. The page should feel like it could have been written by the same person who wrote the tool.
- **Ghostty (ghostty.org).** Modern, fast, opinionated, polished without being loud. Confident defaults. Documentation as a first-class surface.

## Design Principles

1. **The docs page is `--help` with screenshots.** Information density beats hero spectacle. A developer reading this should be able to pick an install line and a flag list and leave.
2. **Show the binary, don't describe it.** The demo video, the live `--help` reference, the real `config.example.yaml` are the page's load-bearing content. Prose is connective tissue, not the substance.
3. **Confident defaults, no choices the visitor doesn't need.** Five install methods, one accent color, one sort order. Configurability lives in the tool, not on the marketing page.
4. **Match the tool's restraint.** `dtop` runs silently in a terminal at low CPU. The page should run quietly too: no auto-playing audio, no parallax, no animations that demand attention, no glow effects that compete with the demo.
5. **Don't impersonate the category.** The reflex for "Docker dev tool" is dark navy + neon green + monospace + glassmorphism. Some of those are honest for this product (mono is honest), others are reflex (glassmorphism, gradient mesh). Each visual choice has to justify itself against "is this what the binary actually feels like, or is it what a landing page is supposed to look like?"

## Accessibility & Inclusion

Not a stated priority. WCAG AA on color contrast and `prefers-reduced-motion` respect are kept as cheap-to-honor defaults, but no audit-driven rework or assistive-tech-specific work is planned.
