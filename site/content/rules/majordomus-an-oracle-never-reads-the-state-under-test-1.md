+++
title = "An oracle takes its inputs from the declared controls or a closed form, never from the state under test"
description = "An oracle takes its inputs from the declared controls or a closed form, never from the state under test"
weight = 5
[extra]
kind = "rule"
slug = "majordomus-an-oracle-never-reads-the-state-under-test-1"
identity = "majordomus.an-oracle-never-reads-the-state-under-test@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/an-oracle-never-reads-the-state-under-test.v1.md"
+++
{% raw %}

## Rationale

A check compares a measurement with an expectation. When the expectation is computed from
state the system under test produced, a defect in the system moves both of them, and the
comparison still agrees. The check then cannot fail, and a green run of it is activity
rather than an outcome (`majordomus.verify-outcomes@1`). Nothing in its output shows this:
it passes on correct code and on wrong code alike.

Observed in algoglyph, a public repository: a simulator claimed that the Mach cone behind a
moving source had the angle the source's speed implied. The claim read the source's speed
from the simulated source itself. A wrong speed in the simulation moved the measured cone
and the expected angle together, so the claim could never fail. Only a planted change to
the simulation, which the claim should have caught and did not, exposed it.

This extends `majordomus.an-equivalence-proof-keeps-a-failing-mutation`, which the
Majordomus repository's pull request #3053 adds to this package: that rule keeps a planted
mutation beside every claim of equivalence. This one says where any check's expectation
may come from, so that such a mutation can fail it.

## Required behaviour

- **Expectations come from declared inputs.** The expected value of a check is computed from
  the inputs the caller declared to the system (controls, parameters, seeds, constants of the
  model) or from an independent closed form of the same quantity. The source's speed in the
  observed case is the speed the scenario set, not the speed the simulated source reports.
- **The state under test is only measured.** Values the system under test produced (its
  state, its outputs, its derived fields, its own report of its inputs) appear on the
  measurement side of the comparison only. An expectation that reads any of them is
  rewritten to read the declared input or the closed form instead.
- **A planted change moves only the measurement.** For each such check, a planted change to
  the system under test (a wrong constant, a dropped term, an input read from the wrong
  place) makes the measurement move and leaves the expectation unchanged, so the check
  fails. Where the repository keeps such a mutation as a permanent control, it follows
  `majordomus.an-equivalence-proof-keeps-a-failing-mutation`.
- **The independent path is named.** A check whose expectation comes from a closed form or a
  reference implementation says which one, so a reader can see that it shares no code or
  state with the system under test.

## Failure behaviour

Nothing decides this rule directly: it has no validator, so a violation is reported by
nobody. It is normative text. A worker applies it when writing a check, and a reviewer reads
each expectation back to its inputs and treats one that reaches the state under test as a
check that cannot fail.

## Verification

Trace the expected value of the check to its sources. Every one of them is a declared input
or an independent closed form; none is read from the system under test. Apply a planted
change to the system: the measurement moves, the expectation does not, and the check fails.
A repository may add a project rule with a validator for the part a machine can decide, for
example that a test's expectation helpers take no argument of the simulated state's type.
{% endraw %}
