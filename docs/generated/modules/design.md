<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `design` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `design` — Design system

The one declaration of how every surface of this tool looks — the semantic roles, the status vocabulary, the type scale, the theme contract — as this executable carries it: its fingerprint, its tokens, what any one of them means, and whether the colours it pairs are readable on one another. The stylesheets the site and the Cockpit load, the tokens the executable's own pages compile in, and the dataset the site's templates read are all projections of it; a page compares its `--mj-design` with this fingerprint to know whether it is wearing the design this executable was built with.

Stability: behaviorally_verified. Capabilities: 4.

## `design.contrast` — Whether the declared colours can be read

Every foreground the design puts on a ground, in both themes, measured against WCAG 2.1 AA: the pair, the palette entries behind it, the ratio and the threshold. The pairs are not a list — they are derived from the declaration, which files each status's text with its own ground, and from the primitives that consume it, where a rule that sets a colour and a background states a pair and a rule that sets only a colour states a foreground that lands on every ground a container sets. A pair below the threshold is a finding that names the role, the ground, the theme, the measured ratio and the required one.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_design_contrast` |
| HTTP | `GET /api/v1/design/contrast` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::design |
| tags | design, ui, accessibility, introspection |

Input: none.

Output: `ContrastReport`.

## `design.explain` — What a design token means

One token by name or by the custom property it becomes: what it is for, what it resolves to in each theme, which Flowbite names are synonyms of it, which state words it colours, and which generated files it reaches.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_design_explain` |
| HTTP | `GET /api/v1/design/explain` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::design |
| tags | design, ui, provenance, introspection |

| input | type | required | description |
|---|---|---|---|
| `token` | string | yes | A role (`fg`), a status (`ok`), a state word (`succeeded`), a type step (`meta`), a
palette entry (`gray-600`), or the custom property any of them becomes (`--mj-fg`). |

Output: `Token`.

## `design.system` — The design system

What the design is: the fingerprint every stylesheet carries, the identity, the type stacks, the theme contract with the pre-paint statement every surface runs, the audit widths, how many tokens of each kind, and every generated file the declaration is projected into.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_design` |
| MCP resource | `majordomus://design` |
| HTTP | `GET /api/v1/design` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::design |
| tags | design, ui, introspection |

Input: none.

Output: `DesignReport`.

## `design.tokens` — The design tokens

Every token of the design, explained: roles with their light and dark values, statuses with their text, ground and border and the state words filed under them, the type scale, the layout values, the theme contract, the palette. Narrow it to one kind.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_design_tokens` |
| HTTP | `GET /api/v1/design/tokens` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::design |
| tags | design, ui, introspection |

| input | type | required | description |
|---|---|---|---|
| `kind` | object | no | Only tokens of this kind. |

Output: `TokenList`.

