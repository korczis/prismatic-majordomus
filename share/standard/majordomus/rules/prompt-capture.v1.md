---
id: majordomus.prompt-capture
version: 1
kind: rule
title: Prompt capture is proven, or reported unavailable
description: A repository that declares prompt capture has it wired below the model and proven by running it; the archive stays ignored and untracked, and every record carries the person's prompt and nothing the model said.
statement: Prompt capture is either wired below the model and proven by execution, or reported as unavailable; the archive is never committed, and a record carries the closed field set and no model output.
status: active
class: blocking
depends_on: [majordomus.sessions-are-workers@1]
tags: [prompts, provider, evidence]

x-majordomus:
  validator: prompt_capture
  category: capture
  enforced_by: [doctor, watch]
  exit_code: 10
  claims: [prompt-capture]
  tests: [test/cases/29_prompt_capture.sh]
---

# Rationale

A worker cannot be asked to record its own prompts. It never sees the bytes the person
typed, only what the provider assembled from them, and a record written by a model is
missing exactly the prompts that mattered: the first one of a session, which arrives before
any instruction has been read, and every one where the model was busy doing what it was
asked. An instruction in a bootstrap file is a request, not a mechanism, and there is no
behavioural test that can prove a request was honoured.

So capture belongs in the provider's own hook, below the model, where it is deterministic
and where running it is the proof that it works. That leaves two honest states for a
provider — capturing, or not observable — and this rule is what keeps the second from being
quietly presented as the first.

The archive is also the most sensitive file the tool writes. Raw prompts carry credentials,
customer names and paths as often as they carry intent, so the same rule that demands the
capture be real demands that it never leave the machine through Git.

# Required behaviour

An enforcement entry of kind `provider-hook` holds only when the provider's configuration
names the shim the tool wrote, the shim is executable, and a synthetic payload driven
through it produces a record. A provider with no adapter is reported as unsupported, and a
repository that wires nothing reports that it captures nothing.

Under the archive directory, nothing is tracked by Git and the ignore boundary covers it.
Every record begins with the schema identifier and carries the closed field set the writer
emits; a field naming a model response, a completion or a transcript is a violation, because
the archive holds the person's half of the exchange and no more.

# Failure behaviour

A violation is a `FAIL` finding under the category `capture`, and the command that found it
exits 10. Under `watch` the same violation is reported as drift and the command exits 11.

# Verification

`mj_validate_prompt_capture` decides the archive invariants and the wiring verifier decides
the provider state, both dispatched from `doctor, watch`. The behavioural case
`test/cases/29_prompt_capture.sh` proves them, and CI runs that case.
