+++
title = "majordomus prs repair"
description = "Bring master into one named pull request whose only conflict with it is over derived (`merge=derived`) files. Eligible only when the classification says it is behind master and its merge conflicts on no authored path; an authored conflict is refused, naming the files. A dry run by default, decided offline on the last recorded observation, like `status`. With `--apply` it takes the integration lease, observes the forge again, records the act on the trail first, and merges master in a scratch worktree, derives, commits and pushes a fast-forward leased on the observed head; it never merges into master. Exit 10 on a refusal or an absent observation, 12 when it could not act"
weight = 91
[extra]
route = "/docs/cli/prs/repair/"
command = "majordomus prs repair"
source = "apps/majordomus-cli/src/cli.rs"
+++
