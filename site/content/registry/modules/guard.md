+++
title = "Edit guard"
description = "Whether a write to a file of this checkout is covered by a claim: the active task's scope, or a live mesh claim of the writing session. The judgement a provider's pre-tool hook asks before every edit (majordomus guard hook), switched by session.edits_require_claim and made to refuse by session.edit_guard_mode."
weight = 27
slug = "guard"
[extra]
id = "guard"
source = "apps/majordomus-cli/src/capability/builtin/guard.rs"
+++
