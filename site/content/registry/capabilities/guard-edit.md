+++
title = "guard.edit"
description = "Whether the given session may write the given path now: allowed when the policy's session.edits_require_claim is off, the path is outside the checkout or its local state, the repository declares no enabled mesh, the active task's scope covers it, or a live mesh claim of the session covers it; refused as unclaimed otherwise, with the remedy. What cannot be read — the policy, the mesh — is let through and named, never refused."
weight = 78
slug = "guard-edit"
[extra]
id = "guard.edit"
source = "apps/majordomus-cli/src/capability/builtin/guard.rs"
+++
