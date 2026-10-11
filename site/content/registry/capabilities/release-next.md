+++
title = "release.next"
description = "What the integration executor raises the trunk's version to when it lands a head (ADR 0134): the version the trunk declares, how far the head moves the public contract measured against the trunk's committed registry, what that movement requires under the version's mode, the repository's floor from `release.floor` in the policy, the exemption class that lowered the floor if every changed path falls in one, the larger of the requirement and the floor, and the trunk's version raised by it — with one sentence per step. Everything is read from git at the two refs, so the answer does not depend on which executable runs or on the branch's own version, which is why a retried landing cannot raise twice. It decides and writes nothing; an input that cannot be read is refused rather than guessed."
weight = 157
slug = "release-next"
[extra]
id = "release.next"
source = "apps/majordomus-cli/src/capability/builtin/release.rs"
+++
