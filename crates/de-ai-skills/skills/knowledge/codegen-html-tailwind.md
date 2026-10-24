---
name: codegen-html-tailwind
description: HTML + Tailwind CSS v4 code generation rules — semantic HTML with utility classes
phase: [generation]
trigger:
  flags: [isCodeGen]
priority: 20
budget: 2000
category: knowledge
---

# HTML + Tailwind v4 Code Generation

Generate semantic HTML5 styled only with Tailwind v4 utility classes. No inline styles.

## Output Format

- HTML5 (`.html`), Tailwind v4
- In a full page, load Tailwind with `<script src="https://cdn.jsdelivr.net/npm/@tailwindcss/browser@4.1.13"></script>`; in a build setup use `@import "tailwindcss";` (never the v3 `@tailwind` directives)
- Design variables go in `:root { --name: value; }` inside `<style type="text/tailwindcss">`, not `@theme`
- Font stacks are NOT variables: define `.font-<name> { font-family: "Name", sans-serif; }` in `@layer base` and use the class
- Never add manual resets: Preflight is included

## Layout Mapping

- `layout: "vertical"` → `flex flex-col`; `layout: "horizontal"` → `flex`
- `layout: "none"` → `grid`, each child `[grid-area:1/1]`; children with `x`/`y` → `absolute left-[Xpx] top-[Ypx]`
- Inside auto layout, ignore `x`/`y` unless the child has `role: "overlay"` (then `absolute`)
- Always set `items-start` / `items-center` / `items-end` (CSS defaults to stretch, the design to start)
- `gap: N` → `gap-[Npx]`; `padding: [v, h]` → `py-[Vpx] px-[Hpx]`; four values → `pt- pr- pb- pl-`
- `justifyContent` → `justify-center` / `justify-end` / `justify-between` / `justify-around`
- `clipContent: true` → `overflow-hidden`; a container with absolute children → `relative`

## Sizing

- Fixed → `w-[Npx]` / `h-[Npx]`; a fixed main-axis size also gets `shrink-0`
- `fill_container` on the main axis → `flex-1 min-w-0` (row) or `grow` (column); on the cross axis → `self-stretch`
- `fit_content` → leave the size unset (hugs content)
- Text `textGrowth: "auto"` → `whitespace-pre`, no width; `fixed-width` → width + `whitespace-pre-wrap`

## Paint

- Solid fill → `bg-[#hex]` or `bg-[var(--name)]`; text colour → `text-[color:var(--name)]`
- Gradients → `[background:linear-gradient(90deg,_#f00_0%,_#00f_100%)]` (spaces become `_`)
- Stroke → `[outline:1px_solid_#e5e7eb] [outline-offset:-1px]` (inside), so the box keeps its size
- Corner radius → `rounded-[Npx]`, ellipse → `rounded-full`
- Shadow → `[box-shadow:0_4px_12px_0_rgba(0,0,0,.1)]`; blur → `[filter:blur(Npx)]`; background blur → `[backdrop-filter:blur(Npx)]`
- `opacity` → `opacity-[N]`; `blendMode` → `[mix-blend-mode:multiply]`; `rotation` → `[transform:rotate(Ndeg)]`

## Typography

- `font-<name>` class, `text-[length:Npx]`, `font-[700]`, `leading-[1.2]`, `tracking-[Npx]`, `italic`, `text-center`, `underline`, `line-through`

## Links and Images

- A text with an `open_url` tap action, or a segment with `href`, → `<a href="…">`
- `<img src alt>` with `object-cover` / `object-contain`; images go in an `assets/` folder next to the HTML
