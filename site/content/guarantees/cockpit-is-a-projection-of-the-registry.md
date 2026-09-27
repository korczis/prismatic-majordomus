+++
title = "A capability added to the registry appears in the Cockpit's listing, navigation and search and on a page of its own with a form generated from its schema, without one line of the Cockpit being edited"
description = "The Cockpit at /cockpit holds no list of its own. What it shows is what the registry"
weight = 171
[extra]
claim_id = "cockpit-is-a-projection-of-the-registry"
status = "guaranteed"
source = "docs/claims/cockpit-is-a-projection-of-the-registry.md"
+++
{% raw %}

## What it means

The Cockpit at `/cockpit` holds no list of its own. What it shows is what the registry
answers, so a capability that did not exist when the Cockpit was written appears in it
anyway: in the capability listing, in the navigation for its kind, in the search, on a
page of its own, and with a runner form generated from the capability's input schema.
Nobody adds a route, a template entry or a menu item for it, and nothing in the Cockpit's
source names it.

The same holds in the other direction. A capability the registry no longer carries is gone
from every one of those surfaces, because none of them kept a copy.

## How it works

The shared server builds the capability registry from the canonical `capability!`
declarations and from the declarative objects of the layer, each of which the registry
turns into a capability of its own. The Cockpit, under `apps/majordomus-cli/src/cockpit/`,
renders that registry: the listing and the search read the registry's own listing, the
navigation is derived from the kinds the index holds, an entity page is addressed by a
slug derived from the object's identity, and the runner form is generated control by
control from the capability's input schema, with its examples taken from the capability's
benchmark cases.

`apps/majordomus-cli/tests/cockpit.rs` proves it over a real socket against a disposable
repository. `a_capability_the_repository_adds_reaches_every_cockpit_surface` adds one rule
to the fixture, commits it, starts the server, and then asks for the new capability in the
listing, on its own page, on its object page with the navigation entry for its kind, at its
entity address, through the typed entity API, in the search, and in the JSON the command
palette reads. `the_runner_form_is_generated_from_the_input_schema` proves that a
capability's form carries the controls its schema declares, with requiredness and bounds,
and none written for that capability.

## How to see it

```bash
majordomus serve                                   # logs the URL of the shared server
# then open <url>/cockpit/capabilities, pick any capability, and use its runner form
cd apps/majordomus-cli && cargo test --test cockpit
```

Add a rule under `.ai/repo/rules/project/`, commit it, and reload the Cockpit: the rule's
capability is in the listing, the search and its kind's index without anything else changed.

## What it does not cover

It proves that a capability reaches the Cockpit, not that every page of the Cockpit is
well laid out or that a person finds what they are looking for. Visual conformance is held
by the design rules and their own cases.

It does not prove that a capability's handler is correct. The runner form sends what the
schema accepts; what the capability does with it is proven by that capability's own tests.

The test adds a declarative object, which is the way a repository adds a capability
without a build. A capability declared in Rust reaches the same surfaces through the same
registry, and is proven by the page sweep in the same file rather than by this test.

## Why it exists

A second list is where a projection starts to drift. An interface that keeps its own
catalogue of what the system can do is correct on the day it is written and wrong on the
first day a capability is added without it, and nobody sees the gap until a person looks
for a thing the machine surfaces already serve. `project.interfaces-are-projections` rules
that out for every interface, and this claim is the Cockpit's evidence that it holds: the
proof is a capability the Cockpit's authors never saw, shown on every surface anyway.
{% endraw %}
