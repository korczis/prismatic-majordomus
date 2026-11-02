+++
title = "majordomus guard hook"
description = "Judge the edit a provider's pre-tool payload (on stdin) is about to make; exit 2 with the reason and the remedy when no claim covers it and session.edit_guard_mode is refuse, 0 otherwise; under advisory, the default, an unclaimed edit is said on stderr and recorded, and exits 0"
weight = 139
[extra]
route = "/docs/cli/guard/hook/"
command = "majordomus guard hook"
source = "apps/majordomus-cli/src/cli.rs"
+++
