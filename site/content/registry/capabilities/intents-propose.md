+++
title = "intents.propose"
description = "Write a new intent record under `.ai/repo/project/intents/` from a statement of what must become true, with an optional title (the statement's first line when absent) and id (the title's slug when absent). The record names no milestone, criterion or invariant: its stage derives as `declared`, `intent validate` reports the two absences as warnings rather than failures, and no work can serve it until it is planned (ADR 0144). Nothing about its standing is stored. An existing record is refused, never overwritten, and the file appears whole or not at all. The answer names the identity and the file; `intents.record` answers the derived stage once the index holds it."
weight = 93
slug = "intents-propose"
[extra]
id = "intents.propose"
source = "apps/majordomus-cli/src/capability/builtin/intents.rs"
+++
