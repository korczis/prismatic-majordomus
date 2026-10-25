+++
title = "quality.secret_argv"
description = "Every Rust source and every shell unit the tree would commit, read for a child process given a secret as an argument, where every user of the host reads it (ps, /proc/<pid>/cmdline): a value read from a credential source (a call or variable named as a secret, an env::var of one, a keychain or credentials-file read) or a binding filled from one, reaching .arg, .args, Command::new, a vector named argv, args or cmd, a runner's argument list, or an argument of an external shell command. Standard input, the child's environment and a 0600 file are the forms that pass. Each finding names the file, the line, the value's name (never the value), where it went and the rule; a line that says why with `secret-argv: <why>` is not one. Reads the tree and git's index only; no build and no network."
weight = 174
slug = "quality-secret-argv"
[extra]
id = "quality.secret_argv"
source = "apps/majordomus-cli/src/capability/builtin/quality.rs"
+++
