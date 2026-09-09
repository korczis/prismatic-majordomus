---
id: project.diagnostics-decide-the-exit
version: 1
kind: rule
title: A return code is not the diagnostic channel
description: A command that emits an error diagnostic does not exit zero; every derivation and every check runs so that an error in the layer stops it.
statement: Where the tool reports an error diagnostic about the layer, the process that ran it exits non-zero. A derivation or a check that reads the index runs under `--strict`, so that a document excluded from the index stops the run instead of being quietly absent from everything derived afterwards.
status: active
class: blocking
depends_on: []
tags: [gates, diagnostics]
---

# Rationale

On 2026-09-09 `scripts/derive` printed

    ERROR majordomus://adr/adr-0028 is claimed by <two files>; every claimant is excluded
          code=duplicate_identity

and exited 0. Two architecture decisions had been dropped from the index, every projection
was then written without them, and the return code said success. It was noticed only
because someone grepped the log on a hunch.

That is the worst shape a gate can have. A failure that announces itself and is then
overruled by the exit code is more dangerous than no check at all, because everything
downstream — a hook, a workflow, a person reading `echo $?` — is entitled to believe it.

The index is deliberately tolerant: a file it cannot place becomes a diagnostic and the
build continues, so that one broken document does not make the repository unreadable. That
tolerance is right for *reading* and wrong for *deriving*. What is written down as the state
of the repository must not be written from a state the tool has already said is incomplete.

# Required behaviour

Every invocation that derives committed artifacts, or checks them, passes `--strict`, so
that an error diagnostic in the layer ends the run with a non-zero exit and a message naming
the count. This covers `scripts/derive` and `scripts/derive-check` today.

A command that reads the layer to *answer a question* — `context`, `search`, the served
surfaces — stays tolerant, and reports the degraded state rather than refusing. The
distinction is whether the output becomes a committed fact about the repository.

# Failure behaviour

The run exits non-zero, naming how many error diagnostics were found and the command that
lists them. Nothing is written.

# Verification

Review, and `test/cases/104_strict_derivation.sh`: a duplicate identity is introduced, the
derivation refuses, and the same tree without the duplicate derives cleanly.
