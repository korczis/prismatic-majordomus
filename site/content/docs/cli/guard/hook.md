+++
title = "majordomus guard hook"
description = "Judge the tool call a provider's pre-tool payload (on stdin) is about to make; exit 2 with the reason and the remedy when no claim covers the file it edits, or when its shell command starts a build (cargo, rustc, scripts/derive, scripts/rust-*) over ssh on a declared offload host outside `majordomus offload`; 0 otherwise"
weight = 139
[extra]
route = "/docs/cli/guard/hook/"
command = "majordomus guard hook"
source = "apps/majordomus-cli/src/cli.rs"
+++
