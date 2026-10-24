+++
title = "mesh.events"
description = "The journal's events a page at a time, in Lamport order: each with its stream, sequence, stamp, repository, signing key, kind and body. The answer's cursor is the next page's `cursor`, and a walk resumed from each one returns every event exactly once, a partitioned runtime's late event included; `after` still walks by Lamport stamp."
weight = 114
slug = "mesh-events"
[extra]
id = "mesh.events"
source = "apps/majordomus-cli/src/capability/builtin/mesh.rs"
+++
