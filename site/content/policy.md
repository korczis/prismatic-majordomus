+++
title = "Canonical policy"
description = "The one provider-neutral file every generated instruction file derives from. Unknown keys are errors."
template = "policy.html"
+++
`.majordomus/policy.yaml` is the source of truth for how AI workers operate in a repository. It names the always-loaded budget, the default profile, the finish contract, the handover sections, the retention caps, the enforcement that `doctor` reconciles, and the instruction files `update` generates. Everything below is read from the skeleton that `majordomus init` installs.
