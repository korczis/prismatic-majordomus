# Site claims

Every substantive claim the public site makes, the canonical file it comes from, what
implements it, and the evidence that it is true. A claim about the *product* belongs in
`docs/CLAIMS.yaml` and is rendered at `/guarantees/`; this file covers claims about the
*site itself* — the ones a reader is entitled to disbelieve until shown the check.

Status has the same meaning as in `docs/CLAIMS.yaml`: **guaranteed** is implemented,
deterministic and backed by a check that runs in CI; **advisory** is true but not
mechanically enforced; **planned** is not implemented.

## Derivation

| Claim | Source | Implementation | Evidence | Status |
|---|---|---|---|---|
| Every product fact on the site is read from a canonical repository file | `docs/GITHUB_PAGES_ARCHITECTURE.md` | `scripts/generate-site-data` | `test/cases/16_site_derivation.sh` changes the version, a README row, a profile axis, a vocabulary row and the outcome vocabulary in a throwaway copy, and fails if the derived output does not change | guaranteed |
| The version shown is the version in the code | `bin/majordomus` | `scripts/generate-site-data` | `test/cases/14_site_data.sh` compares `project.json` with `MJ_VERSION` | guaranteed |
| The task lifecycle is read from the code that enforces it | `lib/finish.sh` | `scripts/generate-site-data` | `test/cases/16_site_derivation.sh` adds an outcome to the `case` statement and asserts it reaches `lifecycle.json` | guaranteed |
| Adding a profile file adds a profile page | `.majordomus/profiles/` | `scripts/generate-site-data` | `test/cases/15_site_builds.sh` asserts a route per shipped profile | guaranteed |
| Adding a vocabulary row adds a routable term page | `docs/CONCEPTS.md` | `scripts/generate-site-data` | `test/cases/16_site_derivation.sh` adds a row and asserts the page exists | guaranteed |
| The site cannot be built from stale generated data | — | `scripts/site-check` | the `derivation` step regenerates and diffs before anything else is checked | guaranteed |

## Capability honesty

| Claim | Source | Implementation | Evidence | Status |
|---|---|---|---|---|
| No capability sentence appears without an implementation and a test | `docs/CLAIMS.yaml` | `scripts/generate-site-data` | the `claim` check fails on a missing path, and on a `guaranteed` claim with no test; `test/cases/16_site_derivation.sh` proves it fails | guaranteed |
| Guaranteed, advisory and planned are never mixed | `docs/CLAIMS.yaml` | `site/templates/guarantees.html` | each claim renders its own status badge from `status` | guaranteed |
| Every responsibility matches a row of the README table | `README.md`, `docs/RESPONSIBILITIES.yaml` | `scripts/generate-site-data` | the `responsibility` check fails when the counts or the keys disagree | guaranteed |

## Commands and profiles

| Claim | Source | Implementation | Evidence | Status |
|---|---|---|---|---|
| Every subcommand is exercised by CI | `bin/majordomus` | `test/cases/17_command_surface.sh` | the `command` check fails when no case invokes a command; CI runs `test/run.sh` on Linux and macOS | guaranteed |
| Every subcommand is documented in the CLI reference | `docs/CLI.md` | `scripts/generate-site-data` | the `command` check fails when a command has no `## majordomus <name>` section | guaranteed |
| `doctor` and `watch` are read-only | `lib/doctor.sh`, `lib/watch.sh` | — | `test/cases/17_command_surface.sh` hashes `.majordomus/state` before and after running both | guaranteed |
| Every profile is exercised by CI, and the verification it declares is enforced | `.majordomus/profiles/` | `test/cases/18_profiles.sh` | starts a task under each profile and asserts `finish` refuses when the declared regression test or decision record is absent | guaranteed |

## The artifact

| Claim | Source | Implementation | Evidence | Status |
|---|---|---|---|---|
| Nothing is fetched from the network at runtime | — | `scripts/site-build` vendors every asset locally | `scripts/site-check` fails on any `src=` to a host other than this site | guaranteed |
| Every route carries title, description, canonical URL and social metadata | — | `site/templates/base.html` | `scripts/site-check` asserts all five on every route | guaranteed |
| Every route has a skip link, one `h1`, and a main landmark | — | `site/templates/base.html` | `scripts/site-check` asserts all three on every route | guaranteed |
| Every internal link resolves | — | `scripts/generate-site-data` rewrites relative Markdown links | `scripts/site-check` resolves every `href` against the built output | guaranteed |
| Every table and diagram scrolls inside its own container | — | Tailwind `overflow-x-auto` | `scripts/site-check` fails on a table or diagram outside one | guaranteed |
| The site works with JavaScript disabled | — | static generation | every filter is progressive; `scripts/site-check` fails an Alpine-driven page that does not say so. Rendering is not itself asserted by an automated browser check | advisory |
| The site is usable at 320px | — | mobile-first Tailwind utilities | reviewed by hand; the structural half (no page-level horizontal overflow source) is checked, the visual half is not | advisory |
| Colour contrast meets WCAG AA throughout | — | Tailwind slate palette | not measured by a tool in CI | planned |
| Rendered output is checked by an automated accessibility audit | — | — | nothing runs axe or pa11y today | planned |

## What is deliberately not claimed

No performance numbers, no adoption metrics, no cost savings, and no testimonials. The
repository has no benchmark data, so the site states none.
