<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `design` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.9 -->
# Module `design` — Design system

The one declaration of how every surface of this tool looks — the semantic roles, the status vocabulary, the type scale, the theme contract — as this executable carries it: its fingerprint, its tokens, what any one of them means, and whether the colours it pairs are readable on one another. The stylesheets the site and the Cockpit load, the tokens the executable's own pages compile in, and the dataset the site's templates read are all projections of it; a page compares its `--mj-design` with this fingerprint to know whether it is wearing the design this executable was built with.

Stability: behaviorally_verified. Capabilities: 6.

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

## `design.kit` — Whether the interface is built on the kit

The supervised repository's user interface judged against the kit profile its policy's `design_kit` block names (flowbite-pro when it names none), from markers in the tree and never the kit's own source: S0 whether there is an interface at all and its surfaces (the root's manifest or the one in assets/, every workspace the root manifest or pnpm-workspace.yaml declares, every nested app manifest with markup in its own subtree outside examples, fixtures, tests, vendor, docs, deps and priv; Phoenix LiveView in mix.exs, HEEx or HTML templates, a Next.js root layout; published pages: a root index.html, pages under site/, www/, public/, under a directory a Pages workflow or a deploy or build script publishes, and under docs/ when it is published), any other manifest with UI packages being a notice and never a finding; S1 the kit's package in each surface, its React binding for React and Next.js, and the kit in use in the surface's own files (an attribute the kit's JavaScript drives, a flowbite-react component, a call of the kit's API), because wiring alone passes nothing; S2 the Tailwind plugin and source (Tailwind 4's @plugin and @source, Tailwind 3's config), a Play CDN or a CDN-only kit being a finding; S3 the kit's JavaScript behind every data-drawer, dropdown, modal, tabs, collapse and tooltip attribute, a script that reads them itself being a re-implementation; S4 no other kit (daisyUI, Bootstrap, Preline, TW Elements, shadcn/ui and Radix, Petal Components, Material UI, Ant Design, Bulma, Chakra UI, Foundation, Materialize), Alpine.js reported and not refused; S5 the shell on every root layout: a fixed top navbar, an aside labelled Sidebar that both drawer attributes name, a breadcrumb, and the page, surface and border roles. Each finding names its signal, path, evidence and remedy; each exemption the policy declares names its reason and how many findings it covered. A licensed kit's profile declares its licensed markers, and where the tree carries one the licence evidence decides it: active passes, while inactive, refused or not judged (nothing judges the evidence until the design licence lands) is a finding no exemption moves, since a variant the judge cannot decide is never a pass. A repository without an interface passes vacuously; a policy that cannot be read is `bad_policy` and judges nothing. Reads the tree below the root, skipping dependencies and build output; no build and no network.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_design_kit` |
| HTTP | `GET /api/v1/design/kit` |
| CLI | `majordomus design kit` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::design |
| tags | design, ui, governance |

Input: none.

Output: `KitReport`.

## `design.kit.adopt` — Declare the kit in the repository

Writes what makes `design.kit` the repository's gate, and nothing that is there already: the profile's rule (project.ui-uses-flowbite-pro for flowbite-pro) into .ai/repo/rules/project/ from the distribution's template, the `design_kit` block naming the profile in .ai/repo/policy.yaml, and two enforcement entries running `majordomus design kit --check`, design-kit-on-push wired by the pre-push hook and design-kit wired by the repository's CI workflow when it has one. Idempotent: a second adoption reports every file unchanged. The rule's last section, 'In this repository', is the repository's own: a later adoption brings the rest of the rule up to the template and never overwrites that section, and a rule that differs from the template and has no such section is kept and named, a block naming another profile is refused, and a policy that would not read back is never written. `dry_run` reports the same and writes nothing. Wiring the hook and the workflow step is left to the repository, and `majordomus doctor` names each entry until it is wired.

| | |
|---|---|
| kind | command |
| stability | experimental |
| CLI | `majordomus design kit adopt` |
| cache | — |
| benchmark | waived (destructive) |
| provenance | builtin majordomus_cli::capability::builtin::design |
| tags | design, ui, governance |

| input | type | required | description |
|---|---|---|---|
| `profile` | string | yes | A kit profile of the distribution, `share/design/kits/<profile>.yaml`. |
| `dry_run` | boolean | no | Report what would be written, and write nothing. |

Output: `AdoptReport`.

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

