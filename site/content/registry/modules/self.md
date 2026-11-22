+++
title = "Self-update"
description = "An installation keeps itself at the published stable release through the one installer: the running version against the version the published metadata names, forward only and stable only, and an apply that re-runs install.sh with the installation's own prefix rather than a second implementation of download, verification and swap (ADR 0161)."
weight = 54
slug = "self"
[extra]
id = "self"
source = "apps/majordomus-cli/src/capability/builtin/self_update.rs"
+++
