+++
title = "postmortems.owed"
description = "Asks the forge, through its own command-line tool and with the caller's credential, for the newest runs of the trunk (branch `master`, 60 runs, unless named), and returns every run that ended in `failure` or `timed_out` since the rule came into force and that no record names in `runs[].id`, each with its page, its age and whether it is still within its grace (24 hours unless named). A cancelled, skipped or green run owes nothing. When the forge cannot be asked the answer is `observed: false` with the reason and owes nothing: an unobserved forge is said, never guessed. When `runs` is given the forge is not asked. Exit 0 when nothing is owed past its grace, 10 when something is."
weight = 152
slug = "postmortems-owed"
[extra]
id = "postmortems.owed"
source = "apps/majordomus-cli/src/capability/builtin/postmortems.rs"
+++
