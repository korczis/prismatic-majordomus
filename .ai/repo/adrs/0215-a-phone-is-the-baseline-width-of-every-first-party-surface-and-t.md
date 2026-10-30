---
schema: adr/v1
id: adr-0215
kind: adr
title: "A phone is the baseline width of every first-party surface, and the primary flow is accepted there before a surface grows"
status: proposed
date: 2026-10-18
tags:
  - ui-surfaces
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0022-the-ui-standard-is-an-audit-over-a-discovered-page-set.md
  - file:.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md
  - file:.ai/repo/adrs/0036-the-design-system-is-a-typed-declaration-and-every-first-party-surface-consumes-its-projections.md
  - rule:project.ui-conformance
  - rule:project.design-tokens-declared-once
  - file:share/design/tokens.yaml
  - file:.ai/repo/project/milestones/the-ui-works-on-a-phone.yaml
  - file:.ai/repo/project/issues/I3214.yaml
  - file:.ai/repo/project/issues/I3215.yaml
  - file:.ai/repo/project/issues/I3216.yaml
  - file:.ai/repo/project/issues/I3217.yaml
  - file:.ai/repo/project/intents/a-person-runs-majordomus-from-a-phone.yaml
  - file:.ai/repo/project/intents/the-mesh-is-operated-from-any-device.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0022-the-ui-standard-is-an-audit-over-a-discovered-page-set.md
    - file:.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md
    - file:.ai/repo/adrs/0036-the-design-system-is-a-typed-declaration-and-every-first-party-surface-consumes-its-projections.md
    - file:share/design/tokens.yaml
    - file:.ai/repo/project/milestones/the-ui-works-on-a-phone.yaml
    - file:.ai/repo/project/issues/I3214.yaml
    - file:.ai/repo/project/issues/I3215.yaml
    - file:.ai/repo/project/issues/I3216.yaml
    - file:.ai/repo/project/issues/I3217.yaml
    - file:.ai/repo/project/intents/a-person-runs-majordomus-from-a-phone.yaml
    - file:.ai/repo/project/intents/the-mesh-is-operated-from-any-device.yaml
---

# 215. A phone is the baseline width of every first-party surface, and the primary flow is accepted there before a surface grows

## Context

The owner reported on 2026-10-08 that the app is not mobile first, that drawers fail on a phone and that this harms adoption. ADR 0022 sweeps every declared width for overflow and accessibility, and ADR 0135 says the UI's shell must work at every declared width, but neither holds behaviour (a menu that closes, a control a thumb can hit, a flow completed) at the floor width.

## Decision

The floor width the design declares is the baseline: every first-party surface (site, /ui, /cockpit) is designed and accepted there first and enhanced upward; one component, route set and design declaration serve every width; the narrow-screen menu is one kit component; the ui audit refuses an undersized touch target; the primary IDD flow has a browser case at the floor width; a new screen is not accepted until it passes there.

## Alternatives rejected

- A separate mobile UI or design system
- Desktop layouts that hide functions below a width
- Responsive CSS breakpoints as the only proof

## Consequences

I3214..I3217 become the milestone the-ui-works-on-a-phone; rule ui-conformance gains the target-size and phone-flow checks; I1016 is either absorbed or unblocked by I3214.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner's acceptance is at `chatgpt-majordomus:c197#0`; accepting this record is still the owner's act.

- `chatgpt-majordomus:c197#0`
- `chatgpt-majordomus:c197#1`
