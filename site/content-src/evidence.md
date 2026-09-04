+++
title = "Evidence graph"
description = "Every claim this project makes, wired to the document that defines it, the file that implements it, and the test that proves it."
template = "evidence.html"
[extra]
graph = "claims-graph"
+++
Each claim is one node. Around it sit the kinds of evidence that can back it: the
document that specifies it, the file that implements it, and the behavioural test that
proves it. Evidence nodes are shared, so a file that carries several claims is one node with
several edges, and a claim standing on nothing is visible as a dot with nothing attached.

A claim marked **guaranteed** must reach a test. That is not a convention — the generator
refuses to build this page if one does not, and the check that follows the build refuses if
the edge was lost on the way in.
