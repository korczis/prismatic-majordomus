+++
title = "Pull-request integration"
description = "Every open pull request classified against the current master — ready, needs refresh, waiting for checks, review or a dependency, draft, needs repair, conflicting, blocked, unsafe (auto-merge armed), redundant (its work is on master already), superseded (by a declared successor that landed, named in superseded_by), possibly redundant, other base or unknown — each with the master and head it was decided against, its reasons, its evidence, its risk and its overlaps, ranked deterministically; and the audit trail of the executor that merges the next provably safe one, one at a time. The relation to master is decided by git with this repository's own merge drivers, because the forge cannot run the derived-file driver. Read from the last recorded forge observation; the one exception is the dry-run proof, which observes the forge itself because the observation is part of what it proves moves nothing."
weight = 24
slug = "integration"
[extra]
id = "integration"
source = "apps/majordomus-cli/src/capability/builtin/integration.rs"
+++
