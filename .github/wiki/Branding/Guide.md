# Brand Guide

## Name

**ClawDE** -- always written with a capital C, lowercase l-a-w, capital D-E. Never "Clawde", "CLAWDE", or "claw-de". The "DE" stands for Developer Environment.

In HTML: `Claw<span class="text-red-600">DE</span>` (the DE is red).

## Colors

| Name | Hex | Tailwind | Use |
| --- | --- | --- | --- |
| ClawDE Red | `#dc2626` | `red-600` | Primary accent, logo DE, CTAs, active states |
| Dark Red | `#991b1b` | `red-800` | Hover states, secondary emphasis |
| Light Red | `#fca5a5` | `red-300` | Badges, subtle highlights |
| Black | `#000000` | `black` | Page backgrounds |
| Near Black | `#060608` | -- | Nav, card backgrounds |
| Zinc 100 | `#f4f4f5` | `zinc-100` | Primary text on dark |
| Zinc 400 | `#a1a1aa` | `zinc-400` | Secondary text |
| Zinc 500 | `#71717a` | `zinc-500` | Muted text, borders |
| Zinc 800 | `#27272a` | `zinc-800` | Card borders, dividers |
| Zinc 900 | `#18181b` | `zinc-900` | Card backgrounds |

## Typography

| Use | Font | Weight | Tailwind |
| --- | --- | --- | --- |
| Headings | Inter | 800 (Extra Bold) | `font-extrabold` |
| Subheadings | Inter | 700 (Bold) | `font-bold` |
| Body | Inter | 400 (Regular) | `font-normal` |
| Code / Terminal | JetBrains Mono | 400-600 | `font-mono` |

## Logo usage

### Horizontal logo (`logo.png`)

The primary logo is a horizontal lockup: claw icon + "ClawDE" wordmark. Use this in navbars, headers, and any context where horizontal space is available.

- Minimum height: 24px on screen
- Clear space: at least 0.5x the icon height on all sides
- On dark backgrounds only (the logo has no light-background variant yet)

### Icon (`icon.png`)

The standalone red claw mark. Use for favicons, app icons, social avatars, and anywhere a square mark is needed.

- Always on a transparent background
- Never stretch, rotate, or recolor
- Never add drop shadows or outlines

### Maskable icon (`icon-maskable.png`)

Has extra safe-zone padding for platforms that crop to circles (Android adaptive icons). Use only where the platform docs require a maskable icon.

## Don'ts

- Don't place the logo on busy backgrounds
- Don't use the red claw on a red background
- Don't retype the wordmark in a different font
- Don't combine the ClawDE logo with other logos in a single lockup
- Don't animate the logo without approval
