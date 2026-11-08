+++
title = "mesh.review.request"
description = "Ask for a review of a branch, commit or pull request, optionally of one named runtime — which must be linked and carry the `reviews` feature, or the request is refused as `feature_unsupported` — and optionally for one session (`for_session`, `claude-code-<uuid>`), whose route the answer carries: `{via: provider-socket, to: uds:<socket>, provider}` from that session's board row on this machine, or `{unavailable: <why>}`. The caller wakes the session with its own provider's messaging tool, carrying the review key; Majordomus connects to no socket and reads no messaging token. Replicated to every linked runtime. Writes this runtime's journal only."
weight = 139
slug = "mesh-review-request"
[extra]
id = "mesh.review.request"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
