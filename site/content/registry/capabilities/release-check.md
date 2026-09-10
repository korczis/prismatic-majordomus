+++
title = "release.check"
description = "Every release invariant this repository can decide locally: that the version clears the minimum its contract change requires, that every breaking change is named by a change record, that a breaking change carries migration guidance, that the change records agree with each other, that the release records agree with the distribution model, and that the committed contract is the contract this build states. Each failure names the command that shows it and the one that fixes it."
weight = 37
slug = "release-check"
[extra]
id = "release.check"
source = "apps/majordomus-cli/src/capability/builtin/release.rs"
+++
