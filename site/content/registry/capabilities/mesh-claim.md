+++
title = "mesh.claim"
description = "Claim repository paths for a session. An exclusive claim that meets a live exclusive claim of another session — on this runtime or any runtime this one has heard — is refused as `claim_conflict` with the claims it meets; an advisory claim is recorded and its overlaps reported. A claim lives while its session is open and its runtime beats: a crashed holder's claim expires everywhere on its own; and an exclusive claim of a session that beats is suspended — refusing nobody — while its holder makes no progress within its ttl. Writes this runtime's journal only."
weight = 113
slug = "mesh-claim"
[extra]
id = "mesh.claim"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
