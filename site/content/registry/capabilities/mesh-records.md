+++
title = "mesh.records"
description = "Every typed coordination record the journal holds, folded: its author (`<stream>/<session>`), kind, task, scope, status, sources, derived evidence state (asserted unless a source is a file at a commit or a tool's observed result), current revision and every earlier one, the record it supersedes and the ones superseding it, its author's standing and why it is stale (an expired lease, a closed or silent author) — stale records are flagged, never removed. Also the conflicts between live plans or decisions of different authors for one task or over meeting scopes, and the writes the fold ignored (a revision not by the author, out of order, or after its author's session closed). `record`, `task`, `kind`, `author`, `path` and `live` narrow it; the digest is the whole fold's. Where this process runs no mesh it answers `active: false` with the reason."
weight = 138
slug = "mesh-records"
[extra]
id = "mesh.records"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
