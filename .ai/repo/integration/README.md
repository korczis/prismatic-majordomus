---
schema: context/v1
id: ai.repo.integration
kind: context
title: Integration
description: What the pull-request integrator records in the tracked half of the layer; today the manifests of the batches it composed.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/kinds.yaml, docs/INTEGRATION.md]
---

# Integration

The integrator (`majordomus prs`, ADR 0101) keeps its working state outside the tree: the
forge observation under `.ai/local/`, the audit trail and the lease under the common git
directory. What it writes *into* the tree is here, and only what must land with the code it
describes:

```text
batches/   one manifest per composed batch (ADR 0114), kind integration-batch
```

Nothing in this directory is written by hand. A file here is the integrator's record of an
act it took, and the gate that reads it compares it with git, not with a person's word.
