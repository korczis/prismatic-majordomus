+++
title = "majordomus prs batch-check"
description = "The gate of ADR 0114 D5: a composed branch that is not a batch is refused. Walks the first-parent line from the merge base of `--base` and `--head`; a merge there whose second parent is the current head of another open pull request of this repository is a member merge. Fewer than two and no manifest added or changed under `.ai/repo/integration/batches/`: not a batch, exit 0. Two or more, or a manifest added or changed whatever the branch merges: it must add or change exactly one manifest, naming exactly those members, at least two, in that order with those heads and merge commits, and every other commit on the line that is not a merge of the base may change only the manifest, the version files `release bump` writes and `merge=derived` paths. The open pull requests are read from the forge, and only when git alone cannot decide. Exit 0 not a batch or a conforming one, 10 refused with every finding named, 12 when it cannot run — git or the forge could not be read — never clean because it could not look"
weight = 92
[extra]
route = "/docs/cli/prs/batch-check/"
command = "majordomus prs batch-check"
source = "apps/majordomus-cli/src/cli.rs"
+++
