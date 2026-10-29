+++
title = "design.kit.adopt"
description = "Writes what makes `design.kit` the repository's gate, and nothing that is there already: the profile's rule (project.ui-uses-flowbite-pro for flowbite-pro) into .ai/repo/rules/project/ from the distribution's template, the `design_kit` block naming the profile in .ai/repo/policy.yaml once the distribution's policy schema declares it (until then flowbite-pro, the default profile, is adopted without it and a note says why, and another profile is refused), and two enforcement entries running `majordomus design kit --check`, design-kit-on-push wired by the pre-push hook and design-kit wired by the repository's CI workflow when it has one. Idempotent: a second adoption reports every file unchanged. The rule's last section, 'In this repository', is the repository's own: a later adoption brings the rest of the rule up to the template and never overwrites that section, and a rule that differs from the template and has no such section is kept and named, a block naming another profile is refused, and a policy that would not read back is never written. `dry_run` reports the same and writes nothing. Wiring the hook and the workflow step is left to the repository, and `majordomus doctor` names each entry until it is wired."
weight = 35
slug = "design-kit-adopt"
[extra]
id = "design.kit.adopt"
source = "apps/majordomus-cli/src/capability/builtin/design.rs"
+++
