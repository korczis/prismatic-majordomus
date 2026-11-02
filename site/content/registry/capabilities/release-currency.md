+++
title = "release.currency"
description = "How much landed work the trunk holds past the last release, since when, and at which version, measured when asked (I3070). The last release is the newer of the release the layer records and the newest version tag the trunk contains: a tag is pushed before its record lands. Landed work is every commit of the trunk past it except merges and release commits. A landing is a commit of the trunk's first-parent history, and the work has waited since the oldest landing that brought anything but release commits. The stance is current when nothing has landed past the release, owed when work has, and overdue once it has waited longer than six hours. The version the public contract requires is decided by the version selection release.version makes, but only from a recorded release and only in a checkout at the trunk; otherwise the answer says which it is not. Ask with next false to leave the version out, as the brief and doctor do. It reads git and the clock, so it is never committed into derived data."
weight = 190
slug = "release-currency"
[extra]
id = "release.currency"
source = "apps/majordomus-cli/src/capability/builtin/release.rs"
+++
