+++
title = "knowledge_base.record"
description = "One knowledge record by id — candidate or curated — with every reference it names resolved: a session against the tracked session records and the ledger, a task or decision against the ledger and the task store, a commit against git, a file or test against the index and the tree, a rule, an ADR, an issue or another record against the index. Which references dangle is the question a reviewer asks before promoting, and the one the integrity validator asks of every record. A `task:none` or `decision:none` reference is refused by name."
weight = 48
slug = "knowledge-base-record"
[extra]
id = "knowledge_base.record"
source = "apps/majordomus-cli/src/capability/builtin/knowledge_base.rs"
+++
