+++
title = "autonomy.friction"
description = "Counted from Claude Code's own transcripts of this repository's checkouts (the primary and every <repository>-wt/ worktree) over the last since_days days: tool calls, and every refusal by kind (classifier_denial, hook_block, safety_check, user_rejection), by the classifier's stated reason and by tool, with the rate per thousand calls. Counts only: no command, path or payload leaves a transcript. A prompt the person approved leaves no trace and is not counted."
weight = 3
slug = "autonomy-friction"
[extra]
id = "autonomy.friction"
source = "apps/majordomus-cli/src/capability/builtin/autonomy.rs"
+++
