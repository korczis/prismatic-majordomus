+++
title = "autonomy.explain"
description = "What a static permission rule may do with one shell command under a profile: allow (it runs unasked), defer (the client's own classifier or prompt decides) or require_approval (a person approves it under every profile), with its risk class, the reason and the table entry that matched. A compound command — chaining, a pipe, a redirection, a substitution, a leading assignment, find -exec — never earns an allow; history rewrites, destructive removal, privilege and external effects require approval even inside isolation."
weight = 3
slug = "autonomy-explain"
[extra]
id = "autonomy.explain"
source = "apps/majordomus-cli/src/capability/builtin/autonomy.rs"
+++
