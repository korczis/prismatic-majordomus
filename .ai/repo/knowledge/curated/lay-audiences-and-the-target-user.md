---
schema: knowledge/v1
id: lay-audiences-and-the-target-user
kind: knowledge
class: convention
title: "The product is for engineers today; a non-technical listener is told in their own trade, with the target architecture marked as such"
description: "Applies to explanations for people outside software (a workshop owner, a restaurant)."
status: verified
epistemics: decided
date: 2026-10-18
tags:
  - product-adoption
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:README.md
    - file:.ai/repo/adrs/0077-published-content-may-be-written-in-the-readers-language.md
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0077-published-content-may-be-written-in-the-readers-language.md
  - type: relates_to
    target: file:.ai/repo/knowledge/curated/positioning-leads-with-the-outcome.md
---

# The product is for engineers today; a non-technical listener is told in their own trade, with the target architecture marked as such

As of October 2026 the public product targets engineers and small engineering teams who already use coding agents; it is not usable by non-technical users. When the owner explains it to someone outside software, the explanation uses that person's domain: a foreman who keeps the jobs, hands work over between shifts, applies the shop's procedures and checks the result before it goes to the customer. Mesh, agents and providers are not mentioned. Anything that describes where the product is heading rather than what it does is said to be the target, not the present.

## Sources

- `chatgpt-majordomus:c289#0`
- `chatgpt-majordomus:c289#2`
