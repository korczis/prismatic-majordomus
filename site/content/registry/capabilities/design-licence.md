+++
title = "design.licence"
description = "The never-publish gate: the raw bytes of every tracked file, committed bundles included (staged, committed where that differs, and the working copy where it differs), and of every untracked file under each `ui.publish` directory, read through symbolic links and into nested repositories as a deploy archives it, compared whole, exactly and with every run of whitespace made one space, against the kit-evidence/v1 fingerprints of the declared kit's licensed files. `refused` names each file and, where the evidence says, the licensed source; `not_judged` is a public tree that declares a kit, tracks a UI file and has no evidence here, and fails like `refused`; a `ui:` block that cannot be used as written (an absolute `ui.publish` or `ui.evidence` entry) is `not_judged` too, naming the entry; `inactive` is no kit declared, a private tree without evidence, or no UI file; `active` is judged and clean, or private. The evidence is the file the input names, else the policy's `~`-relative `ui.evidence` entries; an error names the file and the line, never its content. Near-copies and fragments inlined in a larger file are not judged: the evidence holds whole-file fingerprints only."
weight = 37
slug = "design-licence"
[extra]
id = "design.licence"
source = "apps/majordomus-cli/src/capability/builtin/design.rs"
+++
