+++
title = "majordomus release merge-version"
description = "The git merge driver for the version line (`merge=version`): merge a version file with the base and both sides rewritten to the greater declared version, so the version never conflicts and the advance decides it; git invokes it as `release merge-version %O %A %B %P`"
weight = 111
[extra]
route = "/docs/cli/release/merge-version/"
command = "majordomus release merge-version"
source = "apps/majordomus-cli/src/cli.rs"
+++
