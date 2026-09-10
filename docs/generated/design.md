<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: share/design/tokens.yaml, the one declaration of the design; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.4.0 -->
# The design system

One declaration, `share/design/tokens.yaml`, projected into every surface. Fingerprint `433c061480590fc5e22c2b870f9a3fe3ba879e7cdad91a0d459d0afd88e1790b` (`--mj-design: "433c06148059"` on every page that carries it). Explain any token with `majordomus_design_explain`, `GET /api/v1/design/explain?token=<name>` or the Cockpit's Design page.

## Type

| stack | value |
|---|---|
| `--font-sans` | `ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, "Noto Sans", sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"` |
| `--font-mono` | `ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace` |

| step | size / leading | utility | for |
|---|---|---|---|
| `--text-meta` | 10px / 1.4 | `text-meta` | the smallest legible step; counts, provenance, table headers |
| `--text-label` | 11px / 1.4 | `text-label` | labels, badges, breadcrumbs, keys of a fact list |
| `--text-small` | 12px / 1.5 | `text-small` | secondary text at density; notes, log lines, monospace inline |
| `--text-dense` | 13.5px / 1.6 | `text-dense` | the body text of a control plane |
| `--tracking-caps` | 0.12em | `tracking-caps` | the letter-spacing of an uppercase label |

## Roles

Each role is one custom property; its value follows the theme.

| role | light | dark | Flowbite names | for |
|---|---|---|---|---|
| `--mj-bg` | `white` | `gray-950` | `--color-neutral-primary` | the page |
| `--mj-raised` | `white` | `gray-900` | `--color-neutral-primary-soft`, `--color-neutral-primary-medium`, `--color-neutral-primary-strong` | a card, a panel, a header bar that sits on the page |
| `--mj-sunken` | `gray-50` | `gray-800` | `--color-neutral-secondary`, `--color-neutral-secondary-soft`, `--color-neutral-secondary-medium`, `--color-neutral-secondary-strong` | a well, a table stripe, a code block, a header cell |
| `--mj-line` | `gray-200` | `gray-800` | `--color-default`, `--color-default-subtle`, `--color-default-medium`, `--color-default-strong` | every border and rule |
| `--mj-fg` | `gray-900` | `white` | `--color-heading` | headings, identifiers, values; the text that carries the page |
| `--mj-body` | `gray-600` | `gray-400` | `--color-body` | reading text |
| `--mj-muted` | `gray-500` | `gray-400` | `--color-body-subtle` | secondary text; labels, captions, provenance |
| `--mj-faint` | `gray-400` | `gray-500` | — | a third level below muted; placeholders, disabled text |
| `--mj-accent` | `blue-700` | `blue-500` | `--color-fg-brand` | links, the focus ring, the one colour that means interactive |
| `--mj-accent-strong` | `blue-900` | `blue-400` | `--color-fg-brand-strong` | the accent where it must read as text on the accent's soft ground |
| `--mj-accent-soft` | `blue-50` | `blue-950` | `--color-brand-softer` | the accent as a ground; a selected row, the current navigation entry |
| `--mj-accent-line` | `blue-200` | `blue-900` | `--color-brand-subtle` | the accent as a border around its soft ground |
| `--mj-accent-fill` | `blue-700` | `blue-600` | `--color-brand` | a primary action's background; always carries on-accent text |
| `--mj-on-accent` | `white` | `white` | — | text on a primary action |

## Status

Five meanings; every state word any surface renders is filed under one of them and coloured by it alone.

| status | text (light / dark) | ground | border | words |
|---|---|---|---|---|
| `--mj-ok` | `emerald-900` / `emerald-300` | `--mj-ok-bg` | `--mj-ok-line` | `ok` `pass` `passed` `succeeded` `completed` `done` `current` `declared` `covered` `guaranteed` `healthy` `live` `present` `synced` `stable` `read-only` `connected` `accepted` `enforced` `wired` |
| `--mj-warn` | `orange-900` / `orange-300` | `--mj-warn-bg` | `--mj-warn-line` | `warn` `warning` `stale` `partial` `advisory` `cancelled` `cancelling` `not-generated` `reconnecting` `verify` `executable` `ahead` `behind` `dirty` `proposed` `deprecated` `degraded` |
| `--mj-bad` | `rose-900` / `rose-300` | `--mj-bad-bg` | `--mj-bad-line` | `bad` `fail` `failed` `error` `missing` `blocked` `rejected` `blocking` `refused` `disconnected` `offline` `lost` `stderr` `superseded` `unwired` |
| `--mj-info` | `blue-900` / `blue-400` | `--mj-info-bg` | `--mj-info-line` | `info` `running` `queued` `active` `verified` `generated` `cached` `runtime` `resource` `pending` `loading` `streaming` |
| `--mj-neutral` | `fg` → `oklch(21% 0.034 264.665)` / `fg` → `#fff` | `--mj-neutral-bg` | `--mj-neutral-line` | `neutral` `unknown` `external` `planned` `described` `draft` `ready` `inactive` `stopped` `idle` `query` `command` |

## Layout, radius, motion

| token | value | for |
|---|---|---|
| `--mj-measure` | 62rem | the widest a reading column gets |
| `--mj-cockpit` | 1600px | the widest the Cockpit's layout gets before it centres |
| `--mj-topbar` | 2.75rem | the Cockpit's top bar; the sidebar and a sticky header sit under it |
| `--radius-sm` | 0.25rem | a badge, a chip, an input |
| `--radius-md` | 0.375rem | a button, a code block |
| `--radius-lg` | 0.5rem | a card, a panel |
| `--mj-motion-fast` | 150ms | a progress bar, a hover; anything a state change animates |

## Theme

The class `dark` on the root element means dark; the choice is stored under `color-theme`. Every surface runs the same pre-paint statement, generated from those two values.

## Audit widths

320px, 390px, 1024px, 1280px, 1600px

## Palette

Internal. A role or a status names an entry; nothing else does.

| entry | value |
|---|---|
| `white` | `#fff` |
| `gray-50` | `oklch(98.5% 0.002 247.839)` |
| `gray-100` | `oklch(96.7% 0.003 264.542)` |
| `gray-200` | `oklch(92.8% 0.006 264.531)` |
| `gray-400` | `oklch(70.7% 0.022 261.325)` |
| `gray-500` | `oklch(55.1% 0.027 264.364)` |
| `gray-600` | `oklch(44.6% 0.03 256.802)` |
| `gray-800` | `oklch(27.8% 0.033 256.848)` |
| `gray-900` | `oklch(21% 0.034 264.665)` |
| `gray-950` | `oklch(13% 0.028 261.692)` |
| `blue-50` | `oklch(97% 0.014 254.604)` |
| `blue-200` | `oklch(88.2% 0.059 254.128)` |
| `blue-400` | `oklch(70.7% 0.165 254.624)` |
| `blue-500` | `oklch(62.3% 0.214 259.815)` |
| `blue-600` | `oklch(54.6% 0.245 262.881)` |
| `blue-700` | `oklch(48.8% 0.243 264.376)` |
| `blue-900` | `oklch(37.9% 0.146 265.522)` |
| `blue-950` | `oklch(28.2% 0.091 267.935)` |
| `emerald-50` | `oklch(97.9% 0.021 166.113)` |
| `emerald-200` | `oklch(90.5% 0.093 164.15)` |
| `emerald-300` | `oklch(84.5% 0.143 164.978)` |
| `emerald-900` | `oklch(37.8% 0.077 168.94)` |
| `emerald-950` | `oklch(26.2% 0.051 172.552)` |
| `orange-50` | `oklch(98% 0.016 73.684)` |
| `orange-200` | `oklch(90.1% 0.076 70.697)` |
| `orange-300` | `oklch(83.7% 0.128 66.29)` |
| `orange-900` | `oklch(40.8% 0.123 38.172)` |
| `orange-950` | `oklch(26.6% 0.079 36.259)` |
| `rose-50` | `oklch(96.9% 0.015 12.422)` |
| `rose-200` | `oklch(89.2% 0.058 10.001)` |
| `rose-300` | `oklch(81% 0.117 11.638)` |
| `rose-900` | `oklch(41% 0.159 10.272)` |
| `rose-950` | `oklch(27.1% 0.105 12.094)` |
