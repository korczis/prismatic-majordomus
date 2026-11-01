+++
title = "majordomus proceed take"
description = "Take the decision once: claim the selection exclusively on the primary checkout's server, ensure the issue's canonical worktree, record the take there and start the task bound to the issue in it. The same session taking again, from either checkout, after a restart or after finish, gets that work back (re-claiming an expired claim, releasing a finished one) and creates nothing. Exit 10 when nothing can be selected, 11 when the decision is provisional or no server answers for the primary checkout, 15 when the take is refused; start's own exit is passed through"
weight = 239
[extra]
route = "/docs/cli/proceed/take/"
command = "majordomus proceed take"
source = "apps/majordomus-cli/src/cli.rs"
+++
