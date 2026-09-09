+++
title = "plan.validate"
description = "Every finding the derivation produced, in the order it produced them: a dependency on something that is not an issue, a cycle, an issue executing ahead of its dependencies or of its milestone's gate, an issue with no acceptance criteria, evidence missing under a completion date, a milestone whose graph contradicts itself, two issues of one wave sharing a path. A failure means the model is invalid; a warning means it is legal and worth reading."
weight = 47
slug = "plan-validate"
[extra]
id = "plan.validate"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
