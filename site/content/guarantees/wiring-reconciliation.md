+++
title = "Every enforcement the policy declares is reconciled against what actually runs"
description = "The policy has an enforcement list: things that are supposed to run Majordomus (a pre-commit hook running doctor, a pre-push hook running finish --check). For each entry, doctor proves four things: the executable exists, it is executable, the hook or CI file named in wired_by exists and is executable, and that file actually invokes the command — without discarding its exit code. A declaration that fails any of these is reported as not wired. This is the single most important check in the tool."
weight = 17
[extra]
claim_id = "wiring-reconciliation"
status = "guaranteed"
source = "docs/claims/wiring-reconciliation.md"
+++
{% raw %}

## What it means

The policy has an `enforcement` list: things that are supposed to run Majordomus (a pre-commit hook running `doctor`, a pre-push hook running `finish --check`). For each entry, `doctor` proves four things: the executable exists, it is executable, the hook or CI file named in `wired_by` exists and is executable, and that file actually invokes the command — without discarding its exit code. A declaration that fails any of these is reported as not wired. This is the single most important check in the tool.

## How it works

`lib/doctor.sh` resolves the enforcement's `path` (on `PATH`, repository-relative, absolute, or as the path written on the hook line itself), resolves the hook directory through `core.hooksPath` or `.git/hooks/`, reads the hook file, greps for `majordomus <first-arg>`, and then greps the same line for `|| true` or `|| exit 0`. `wired_by: ci:<file>` checks a CI file the same way; `wired_by: manual` is reported as unverified, never as wired. `doctor` runs this against the repository it is installed in — including this project's own hooks in CI.

## How to see it

```bash
printf '#!/bin/sh\nmajordomus doctor || true\n' > .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
majordomus doctor
# FAIL wiring      doctor-on-commit — .git/hooks/pre-commit invokes it but swallows the exit code (|| true)
```

## What it does not cover

It proves reachability, not execution history: a wired hook can still be bypassed by someone who edits `.git/hooks` or clones elsewhere. Those are documented limits of any local hook, stated in the security notes, not silent failures.

## Why it exists

Seventeen independent cases were found in the source environment of a check documented as blocking, present on disk, executable, and invoked by nothing — including a well-engineered commit guard whose registration was silently dropped when unrelated tooling rewrote a settings file, while its tests and documentation stayed in place. Declared-but-not-wired is the default end state of enforcement, not an edge case; a check that fails on it is the product.
{% endraw %}
