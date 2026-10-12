+++
title = "majordomus derive"
description = "Whether a derive may leave stage B (the site data) alone: the decision `scripts/derive --incremental` acts on, made against this checkout's record of its last successful stage B, and the record itself. The default derive is the full one and never asks (ADR 0143)"
sort_by = "weight"
template = "docs-cli-group.html"
page_template = "docs-cli-command.html"
weight = 99
[extra]
route = "/docs/cli/derive/"
command = "majordomus derive"
source = "apps/majordomus-cli/src/cli.rs"
+++
