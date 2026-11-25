+++
title = "gates.baseline"
description = "The runs of one check over the change, over the commit it is built on (its merge base with the trunk, or the trunk's tip) and optionally over a neutral perturbation of that commit, the same declared number of times on each side, judged per failure id: `introduced` (the change shows it and the base never did), `pre_existing` (the base shows it too: the trunk's), `fixed`, or `unknown` (the base could not be compared with, or a failure that names nothing matches another that names nothing). Flaky is a property, never a class. Under the default rule, `subset` — no failure the base does not also show — only an introduced id blocks; under `rate` an id blocks when the change shows it in more runs than the base and the control by more than the declared slack. The verdict is `clean`, `clean_with_trunk_failures` (with the trunk alarm: the base's own failures and their counts), `introduced` or `undecided` with its reason, and carries its denominator and the provenance the caller supplied. A finding that names a file says whether that file is in the change set, so a whole-tree scan can say \"outside your diff\". It judges and never runs the check, and never turns a red verification into a green one."
weight = 80
slug = "gates-baseline"
[extra]
id = "gates.baseline"
source = "apps/majordomus-cli/src/capability/builtin/gates.rs"
+++
