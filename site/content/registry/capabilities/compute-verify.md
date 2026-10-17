+++
title = "compute.verify"
description = "Recompute a claimed result of a portable-core function (today `canonical_digest`: SHA-256 of a JSON document's canonical bytes) on the input the claim carries, with the same core the browser ran as wasm32, and answer `match`, `mismatch`, or `invalid` with the reason when the input is not JSON, exceeds 1 MiB or the result is not the function's shape. It records nothing: no execution, ledger entry or journal event, and the claim never enters a record."
weight = 13
slug = "compute-verify"
[extra]
id = "compute.verify"
source = "apps/majordomus-cli/src/capability/builtin/compute.rs"
+++
