+++
title = "gates.state"
description = "Every gate the CI model declares with the standing of its last recorded run in this checkout, whichever task recorded it: `pass` or `fail` over the files as they stand now, `stale` when those files have changed since the run, `queued` when no run has ever reported — listed again under `never_reported`, because a missing verdict is the case an absent reader renders green — and `unknown` when a run exists and cannot be checked against this tree. Each run carries the commit it was taken at and whether that is the commit checked out now. Read offline from this checkout's own record; what it did not consult, the forge's CI runs, is named under `unreached` rather than left out."
weight = 77
slug = "gates-state"
[extra]
id = "gates.state"
source = "apps/majordomus-cli/src/capability/builtin/gates.rs"
+++
