+++
title = "Browser compute"
description = "The server's half of browser compute (ADR 0170): a browser computes with the portable core, the same code this executable links, and returns a claim; the server recomputes it on the same input and says whether they agree. A verification records nothing, so any caller may ask, and agreement among browsers is never proof."
weight = 7
slug = "compute"
[extra]
id = "compute"
source = "apps/majordomus-cli/src/capability/builtin/compute.rs"
+++
