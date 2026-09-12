+++
title = "deploy.verify"
description = "Live verification: every surface the change reaches — the published site, the published release metadata, each active deployment — is asked for the identity it states (the commit, version or tag at its own address) and compared with what this checkout expects. A surface stating an older identity is stale, one that does not answer is unreachable, and neither is a pass: a deploy command that exited 0 with the old revision still live is exactly what this refuses. The request carries no header and the evidence carries no body beyond the fields compared. The one capability of this executable that reaches the network, and it reaches only addresses the repository itself declares."
weight = 12
slug = "deploy-verify"
[extra]
id = "deploy.verify"
source = "apps/majordomus-cli/src/capability/builtin/deploy.rs"
+++
