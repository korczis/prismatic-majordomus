+++
title = "quality.rustdoc"
description = "The crate's rustdoc tree — the rustdoc surface's artifact, as the web topology resolves it — judged against the crate's own inventory of exported items: every item that owns a page has it at the route rustdoc gives it, no item page is left without an item, the tree declares it was built from HEAD, the library's index is present and names the crate, its assets are present, every relative link resolves, and no file names the machine it was built on or carries a credential. Answers the verdict (clean, findings, or no_tree when there is nothing to judge), the counts it joined, every exported module with its page, and one typed finding per defect with the file and what to do."
weight = 89
slug = "quality-rustdoc"
[extra]
id = "quality.rustdoc"
source = "apps/majordomus-cli/src/capability/builtin/quality.rs"
+++
