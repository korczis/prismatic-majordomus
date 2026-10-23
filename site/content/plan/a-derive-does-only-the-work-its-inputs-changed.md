+++
title = "a-derive-does-only-the-work-its-inputs-changed — A derive does only the work its inputs changed, and can say why"
description = "The derivation is a typed graph the executable owns: every producer is a node with declared inputs and outputs, every node has a key computed from the bytes it reads, a read-only plan says for each node whether it would be reused or regenerated and why, and an incremental run regenerates only the nodes whose keys moved. The full derivation stays as the oracle, and a test holds the incremental result byte-identical to it."
weight = 121
template = "milestone.html"
[extra]
plan_id = "a-derive-does-only-the-work-its-inputs-changed"
source = ".ai/repo/project/milestones/a-derive-does-only-the-work-its-inputs-changed.yaml"
+++
