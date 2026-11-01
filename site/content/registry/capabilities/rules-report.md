+++
title = "rules.report"
description = "The whole rule corpus joined to the tree and the ledger: per rule, its class, its enforcement mode, the validator and the cases it names, whether each is in the tree, the execution behind each, the CI gates that run them, and the sentence explaining how the state was derived. The tallies count the whole corpus even when the answer is filtered, and the findings name every rule whose declared class the proof does not support. Kept per checkout state, never longer: the ledger is a file that changes outside this process, and an edit of it, of a validator or of any tracked file, a new untracked file, a commit or a new ref is a new answer."
weight = 194
slug = "rules-report"
[extra]
id = "rules.report"
source = "apps/majordomus-cli/src/capability/builtin/rules.rs"
+++
