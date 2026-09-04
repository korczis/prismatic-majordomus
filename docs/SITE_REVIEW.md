# Site review — route audit, ownership, validation

Produced during the page-by-page review of the public site on 2026-09-04. It records what
every route is for, where its content comes from, what was changed and why, and how the
result was validated. Counts here are written by hand and will rot; the commands to
recompute them are given.

## Route audit

| route | purpose | audience | source | primary action | action taken |
|---|---|---|---|---|---|
| `/` | recognise the problem, see the control model, install, know what is guaranteed | curious developer, lead | README sections, marketing.toml, generated data | Install Majordomus | IMPROVE: pain-first hero; failure-mode flow with "who checks?"; boundary statement (refused acceptance, not runtime prevention); before/with; one primary CTA; profiles, principles, nine-responsibility cards and site pipeline moved to their pages |
| `/why/`, `/why/<slug>/` | the five moments a team recognises, each explained and bounded | curious developer | `site/content-src/why/` | read the case, then the commands | KEEP (added earlier tonight); each links responsibilities, commands, claims |
| `/getting-started/` | smallest truthful path to a supervised repository | developer | `project.json`, `policy.json` | run the commands | KEEP |
| `/supervises/`, `/supervises/<slug>/` | the nine responsibilities and the ten principles | evaluator | README rows, claims by keyword | read a responsibility | IMPROVE: principles moved here from the homepage |
| `/commands/`, `/commands/<name>/` | one page per command from the CLI reference | existing user | `docs/CLI.md` sections | read the command | KEEP; command pages list the claims they implement |
| `/profiles/`, `/profiles/<slug>/` | every field of every profile | evaluator, existing user | `profiles.json` | pick a profile | KEEP; advisory claims linked |
| `/policy/` | the canonical policy as structure and as text | existing user | `policy.json` | edit the policy | KEEP; claims defined by the policy linked |
| `/guarantees/` | the full claims matrix | skeptical engineer | `capabilities.json` | verify a row | KEEP; every row links to its page |
| `/guarantees/<status>/` | one status at a time | skeptical engineer | `capabilities.json` | pick a claim | KEEP |
| `/guarantees/<id>/` | detailed claim: meaning, mechanism, how to see it, limits, evidence, provenance, test command, where used | skeptical engineer | `docs/claims/<id>.md`, `capabilities.json` | run the test | IMPROVE: detail documents added for all claims |
| `/limitations/` | what v0.1 does not do | evaluator | README "Limitations" | read the roadmap | GENERATE (new) |
| `/roadmap/` | what comes next, gated | evaluator | README "Roadmap" | read planned claims | GENERATE (new) |
| `/architecture/` | how the site is derived | maintainer | `content-src/architecture.md`, `source.json` | read the architecture doc | KEEP |
| `/docs/`, `/docs/<doc>/` | the long-form documents as GitHub shows them | contributor, evaluator | `docs/*.md` | read | KEEP; each lists the claims it defines |
| `/concepts/` | pointer to the vocabulary document | — | stub | — | REMOVE (nav points at `/docs/concepts/`) |
| `/render-test/` | rendering reference for validation | maintainer | `content-src/render-test.md` | — | KEEP, `noindex` |
| `404.html` | not found | anyone | template | back to start | KEEP |

Recompute: `find site/public -name index.html | wc -l` after `scripts/site-build`.

## Enforcement-boundary statements

Every public sentence using prevent / block / enforce / refuse / detect / verify was listed
(`grep` over `marketing.toml`, `content-src`, `docs/claims`, templates) and classified. None
claims runtime prevention. The contract stated on the homepage and on the finish-related
claim pages is: Majordomus declares scope up front, detects a file outside it at `check` and
`finish`, and refuses to accept the work as completed; hooks that run `doctor` and
`finish --check` block a commit or push only where the repository has wired them, which
`doctor` itself verifies. Profile-related claims say in their own text that they are advisory.

## Canonical ownership of site facts

| fact | canonical home | reaches the site through |
|---|---|---|
| version, commands, exit codes | `bin/majordomus` | `project.json`, `commands.json` |
| tagline, problem evidence, what it does, what it is not, limitations, roadmap | `README.md` by heading | `readme.json` |
| profiles | `share/skeleton/profiles/*.yaml` | `profiles.json` |
| policy | `share/skeleton/policy.yaml` | `policy.json` |
| the ten principles | `share/skeleton/providers/body.md` | `lifecycle.json` |
| outcomes | `lib/finish.sh` | `lifecycle.json` |
| claims and their status | `docs/CLAIMS.yaml` | `capabilities.json`, `docs/SITE_CLAIMS.md` |
| claim detail | `docs/claims/<id>.md` | claim page bodies |
| long-form documents | `docs/*.md` via `docs/README.md` | `docs.json`, `content/docs/` |
| case studies and their hooks | `site/content-src/why/*.md` | the `why` section |
| navigation | `site/data/nav.toml` | navbar |
| marketing copy | `site/data/marketing.toml` | homepage leads |

Nothing on the site is parsed from `CLAUDE.md` or `AGENTS.md`; those are themselves
projections of `share/skeleton/providers/body.md`.

## Validation performed

```
bash test/run.sh                    13 cases, disposable repositories
scripts/generate-site-data --check  derived data matches canonical inputs
scripts/site-build                  Zola + Tailwind + vendored JS
scripts/site-check                  sync, schema, promo budget, routes, prefix, links, assets,
                                    dark CSS scoping, offline, JS hooks, Mermaid runtime,
                                    mobile lint, private paths, secrets, docs coverage, tiles,
                                    navigation, claim links, claim count
scripts/site-probe                  every route × {320, 390, 1280} px in headless Chrome:
                                    scrollWidth == clientWidth; Mermaid rendered; mobile menu,
                                    dropdown and theme toggle exercised
```

## Before / after

Before: fourteen homepage sections; nine equal-weight navigation links; the README's problem
prose repeated under a hook that already said it; claims mentioned as plain text; no pages
for limitations or roadmap; a `/concepts/` stub that only pointed elsewhere; responsiveness
verified by hand.

After: eight homepage sections in funnel order with one primary action; five navigation
intents with dropdowns; a failure-mode flow and an explicit statement of the enforcement
boundary; every claim mention a link to a detailed page; limitations and roadmap generated
from the README; the stub removed; a browser probe in CI.

## Remaining limitations

- Accessibility was reviewed by reading the markup and exercising controls in headless
  Chrome; no screen-reader or automated a11y scanner run.
- Mermaid is loaded on pages that carry diagrams; the bundle is large and is loaded only
  there.
- The responsibility→claim cross-links use a keyword map in the generator; a new claim whose
  text uses none of the keywords will not appear on a responsibility page until the map is
  extended.
