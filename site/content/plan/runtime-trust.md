+++
title = "runtime-trust — A runtime is safe to reach from another machine, and joining the mesh trusts nobody by accident"
description = "Discovery is not trust, trust is not authorization, and a caller is admitted to change anything only by a key the trust policy names. A host that can reach a runtime reads it and changes nothing it cannot authenticate; every event in the journal was checked against the trust policy that holds now, whether it arrived over a link or from disk; and the limits the documents state are the limits the code enforces."
weight = 27
template = "milestone.html"
[extra]
plan_id = "runtime-trust"
source = ".ai/repo/project/milestones/runtime-trust.yaml"
+++
