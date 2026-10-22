+++
title = "autonomy.approvals"
description = "The owner's standing approvals, declared once in .ai/repo/autonomy/ (kind autonomy-declaration), against this machine's Claude Code user settings: each approval held or not (its rule is an exact entry of autoMode.allow), whether $defaults stays first, how many declared environment lines and settings are held, how many autoMode.allow entries are not declared (counted, never quoted), and the verdict — full, partial, none, undeclared or unreadable — with the command that writes what is missing. Reads only; `majordomus autonomy apply` is the person's act that writes."
weight = 2
slug = "autonomy-approvals"
[extra]
id = "autonomy.approvals"
source = "apps/majordomus-cli/src/capability/builtin/autonomy.rs"
+++
