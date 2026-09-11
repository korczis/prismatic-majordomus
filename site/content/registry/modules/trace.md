+++
title = "Traceability"
description = "Which branches and commits realised an issue, and which issue and milestone a commit served — derived from git and from the canonical project model on every call, stored nowhere. A branch names an issue when one of its path components is an issue id; a commit belongs to the issue whose branches hold it; a commit no such branch holds is reported as unattributed rather than left out, because work with no execution contract is what a traceability report exists to make visible. Pull requests are a GitHub fact and this executable makes no network call: `scripts/traceability` reads them and joins them to this answer over the branch name."
weight = 29
slug = "trace"
[extra]
id = "trace"
source = "apps/majordomus-cli/src/capability/builtin/trace.rs"
+++
