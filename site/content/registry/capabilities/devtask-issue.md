+++
title = "devtask.issue"
description = "Identity, title, intent, status, milestone, acceptance criteria, dependencies, blockers, the commits its own evidence names and the commits git found, its branches, the sessions that worked on it, its readiness and everything wrong with its records — each field carrying whether a person authored it (`explicit`), a machine worked it out by a rule that cannot be wrong (`derived`), a machine worked it out by a rule that can (`inferred`, with the rule stated), or it is not available at all (`unknown`, with the reason). A key the record does not carry is `unknown`, never an empty string: `objective: \"\"` and a record with no objective are the same string and different facts. An id the model does not declare is answered, not refused. `git: false` answers from the records alone, which is what a comparison across machines wants."
weight = 23
slug = "devtask-issue"
[extra]
id = "devtask.issue"
source = "apps/majordomus-cli/src/capability/builtin/devtask.rs"
+++
