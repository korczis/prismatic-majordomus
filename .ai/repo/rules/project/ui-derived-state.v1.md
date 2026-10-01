---
id: project.ui-derived-state
version: 1
kind: rule
title: An interface shows only what is derived, and unknown is never drawn as healthy
description: Every fact, count, status and inventory the Cockpit and the site show is derived from a capability answer or a generated document with its provenance; nothing is mocked or hardcoded, no inventory is kept twice, a verdict carries its evidence, an absent or undecided value renders as unknown, a derived value says when it was observed, and the frontend computes presentation only. The ui-integrity gate refuses the violations a line scan can see.
statement: A user interface MUST NOT show mocked data, hardcoded domain state or a second hand-kept copy of an inventory; every status it draws MUST carry the evidence it was decided from; a value that is absent, failed or undecided MUST render as unknown and never as healthy, ok or zero; a derived value MUST say when it was observed where freshness matters; and the frontend MUST compute presentation, never reality.
status: active
class: blocking
depends_on: [project.product-surface-derived@1, project.entry-reports-only-evidence@1, project.web-surface-declared-once@1, project.interfaces-are-projections@1]
tags: [ui, cockpit, site, evidence, derivation, enforcement]

x-majordomus:
  tests: [test/cases/547_the_ui_says_only_what_is_derived.sh, test/cases/389_cockpit_projection.sh]
---

# Rationale

On 2026-09-29 two read-only audits (the Cockpit and the public site) classified every fact
and list the interfaces show. Nothing was mocked, but a thin layer was not what it looked
like. A health check was ok because its status was the literal `HealthStatus::Ok`. Effect
chips counted `0` because the count was the literal `0`. An upstream with no tracking data
read `+0 −0`. A model with no declared status read "available" because that was the enum's
`#[default]`. A stat strip dropped the tallies that were bad. On the site, a declared
guarantee was coloured as proven. Each rule that bears on this (`product-surface-derived`,
`entry-reports-only-evidence`, `web-surface-declared-once`, the ADR 0012 projection gate) holds
one part of it. None of them says the whole invariant, and none reached Cockpit Rust literals.
ADR 0089 records the decision.

# Required behaviour

1. **No mocked production UI data.** A page renders what a capability answered or what a
   generator derived. Fixture or illustration data never ships as a production value.
2. **No hardcoded domain state.** A status, count, maturity or verdict is never a literal
   in a template, a page or a handler. A literal that is right by construction says why in a
   `// ui-integrity: <reason>` comment.
3. **No duplicated inventories.** A list of routes, areas, states, effects or tallies is read
   from the one place that declares it. It is not written out again beside that place.
4. **Evidence required.** A verdict that asserts health, currency or proof names what it
   was decided from (`decided_by`, `evidence`, the preflight `Check`). A declared status is
   styled apart from an observed one.
5. **Unknown is not healthy.** An absent, failed or undecided value renders as unknown. It
   never renders as ok, healthy, available, current or zero. No enum defaults to a healthy
   variant.
6. **Freshness is explicit.** A value read once and shown later says when it was observed.
   Where a staleness bound is declared, the value says when it went stale.
7. **The frontend computes presentation, not reality.** A Cockpit page asks a capability
   through `Context::execute`. It reads neither the index nor, outside the navigation's
   declared allow-list, the registry, the product model or the Why catalogue. A verdict is
   decided in the capability, not in the view.

# Failure behaviour

The `ui-integrity` gate (`scripts/ci/cockpit-projection-check --ui-integrity`, job
`structure`, every plan) exits 10 on a finding that `.ai/repo/ui-integrity-baseline.txt`
does not hold. The findings it can see are:

- a direct `ctx.registry`, `ctx.product` or `ctx.why` read in a Cockpit file outside the
  allow-list;
- a health check given `status: HealthStatus::Ok` as a literal;
- a `#[default]` on a healthy enum variant;
- an absent count rendered as zero in `pages.rs`.

The `cockpit-projection` gate exits 10 on a direct index read. The baseline only shrinks: a
line is paid by fixing its finding. `--strict` ignores the baseline, and that is the end
state.

# Verification

`test/cases/547_the_ui_says_only_what_is_derived.sh` plants each violation in a fixture tree.
It asserts that each one is refused by check, file and line, that a clean tree and an
exempted site pass, and that the ratchet admits known debt, refuses new findings, reports
paid lines and is ignored under `--strict`. `test/cases/389_cockpit_projection.sh` holds the
index half. Sub-rules 1, 3, 4 and 6 are enforced here only as far as a line scan reaches.
The rest, and the site's templates, are the follow-ups ADR 0089 lists.
