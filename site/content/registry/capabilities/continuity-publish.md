+++
title = "continuity.publish"
description = "Projects the newest handover record into a portable record — the handover body, the repository and device identity, the episode, the source state (branch, commit, and for a dirty tree the changed paths and a fingerprint, never their content), the task and its decisions, and the record it continues — refuses it when any value carries a credential, a secret environment value or a path of this machine's disk, signs it with the device's mesh key and adds it to refs/majordomus/continuity. Touches no branch, index or working tree, and no network: a sync publishes it."
weight = 14
slug = "continuity-publish"
[extra]
id = "continuity.publish"
source = "apps/majordomus-cli/src/capability/builtin/continuity.rs"
+++
