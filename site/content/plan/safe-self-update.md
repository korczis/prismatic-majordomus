+++
title = "safe-self-update — Every mesh node follows the published release by itself, versions meet safely, and a release's provenance can be checked"
description = "An enrolled mesh node follows the stable release by itself through the one installer, forward only, activating in order and rolling back a failed activation (ADR 0161); a newer client replaces an older server in an ordered hand-off and an older client is told it is older; a change to any wire or on-disk schema needs a version bump; a released archive carries a provenance attestation the installer checks."
weight = 39
template = "milestone.html"
[extra]
plan_id = "safe-self-update"
source = ".ai/repo/project/milestones/safe-self-update.yaml"
+++
