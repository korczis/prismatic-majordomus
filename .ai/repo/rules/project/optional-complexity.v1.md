---
id: project.optional-complexity
version: 1
kind: rule
title: Complexity is optional, not ambient
description: Simple things are trivial, complex things possible, weird things hackable; a change that makes the default path harder, adds a foundational noun without a concrete need, or leaks internals into ordinary use is refused in review.
statement: Keep the core closed and the edges open; a change may add sophistication behind a stable mechanism, never mandatory complexity in front of the ordinary path.
status: active
class: blocking
depends_on: [project.no-new-nouns@1]
tags: [design, usability, extensibility]
---

# Rationale

Majordomus is opinionated by default, programmable by design, and hackable at the edges.
Simple things should be trivial. Complex things should be possible. Weird things should be
hackable. That order matters: the tool earns the right to be deep by being shallow first,
and every supervisory layer in the source material that reversed the order was abandoned
within days, because nobody could use it without first understanding it.

The mechanism is a closed core and open edges. The core is the small set of concepts
`docs/CONCEPTS.md` names — policy, profile, rule, projection, state, doctrine, validator,
adapter — and it stays small, explicit and stable. Extension adds instances, mappings,
validators, templates, metadata and integrations around those concepts; it does not add
concepts. Mechanisms stay stable; meaning and configuration may be data-driven. Data-driven
does not mean infinitely meta: a mechanism that exists to define mechanisms, a schema for
schemas, a DSL because something could be generalised, is the failure this rule refuses.

Majordomus is not a generic framework for arbitrary agents, workflows, resources or
orchestration. Its extensibility lives inside its domain, and a change that makes it a
framework for defining frameworks is out of scope however elegant.

# Required behaviour

Three layers of use, each complete without the next:

- A **default user** runs `init`, `update`, `doctor`, `start`, `check` and `finish` and
  is never asked to understand a schema, front matter, a projection template, a validator
  or the state files. Ordinary error messages name what to do, not an internal structure.
- A **power user** customises the repository declaratively: the policy, the profiles, the
  project rules, the prompts, the workflows and the knowledge sources, in the files under
  `.ai/repo/` and nowhere in the implementation.
- An **advanced user** inspects and extends the mechanisms: validators, provider
  templates, projections, allow-lists, the rule package, the site generator. Advanced
  capability must not make the two paths above harder.

Before an architectural change is merged, its author and its reviewer answer these, and a
"yes" to a question whose honest answer should be "no" is a reason to refuse:

1. Does this make the default path more complex?
2. Is the complexity it adds mandatory, or optional?
3. Is a new abstraction actually necessary?
4. Can it be expressed by composing the concepts that already exist?
5. Does it solve a concrete Majordomus use case, named in the change?
6. Is an extension point justified by a real need rather than a speculative one?
7. Does internal complexity leak into ordinary CLI output or error messages?
8. Is provider neutrality preserved?
9. Is it consistent with the roadmap as it stands?
10. Is it turning Majordomus into a generic framework instead of strengthening its domain?

Anti-patterns, each refused on sight:

- a routine task that needs schema internals or front-matter semantics to complete;
- raw metadata or schema errors in ordinary command output;
- generic infrastructure with no Majordomus requirement behind it;
- an extension point created before one real requirement demands it;
- a foundational noun introduced casually (see `project.no-new-nouns`);
- customisation that depends on editing implementation files;
- doctrine duplicated into provider files instead of read from the layer;
- hackability confused with a lack of defaults;
- data-driven design confused with unconstrained metamodel growth;
- an abstraction kept because it is elegant;
- an advanced feature that complicates the standard workflow.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not
merged. Where a case covers part of it — the bootstraps carrying no rule corpus, the
projections generated from the policy, unknown configuration keys refused — that case
is named by the rule that owns it.

# Verification

Review, against the questions above. The related machine-enforced rules are
`majordomus.bootstrap-integrity` (a projection stays a thin bootstrap),
`majordomus.projection-integrity` (generated files are generated) and
`majordomus.policy-integrity` (unknown keys are errors); `project.no-new-nouns` is the
rule this one rests on.
