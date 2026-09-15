# Phase 8 — Security, LLM-Assisted Semantics, Provider Policy, and Context Assembly

Use the master contract. Deterministic RKS must already work before this phase.

Add optional LLM-assisted semantic enrichment without compromising provenance, security, reproducibility or offline behavior.

## Golden rule

LLMs enrich RKS. They do not become the source of truth for facts that deterministic extraction can provide.

The deterministic bootstrap/check path must remain functional without network or provider credentials.

## Provider integration

Inspect and reuse Majordomus’ real provider/model abstraction. Do not create RKS-specific API-key handling or provider registry.

RKS semantic operations should use canonical provider/model selection, policy, streaming and audit/session infrastructure where available.

## Remote processing policy

Before any repository content is sent to a remote model, evaluate:

- evidence visibility/classification
- source-specific remote-processing permission
- provider capability/policy
- secret/redaction policy
- organization/repository configuration if present

Represent policy centrally and test it.

Example semantics:

```text
source.allow_remote_processing = false
provider.remote = true
=> blocked
```

Local providers may follow a different policy path if the existing platform distinguishes them.

## Secret-safe ingestion

Reuse or implement safe secret detection/redaction before remote model payload construction.

Never log or persist raw credentials in:

- evidence
- prompts
- traces
- knowledge projections
- Cockpit
- MCP responses

Add regression fixtures containing obvious fake secrets and assert they are not leaked.

## Semantic capabilities

Implement optional operations such as:

### Semantic classification

Classify discovered components/concepts/workflows when deterministic metadata is insufficient.

Results must be `derived`, carry provider/model/extractor provenance and reference input evidence.

### Summaries

Generate concise human/agent summaries of knowledge nodes.

Summaries are projections, not replacement evidence.

### Weak relation proposals

Propose semantic relations such as conceptual dependency or workflow association. Mark them derived with confidence/basis.

### Semantic conflict candidates

LLM may flag possible contradictions in free-text docs. These should become reviewable candidates, not silently authoritative conflicts unless corroborated.

## Confidence discipline

Do not accept arbitrary “0.93 confidence” from the model as canonical.

Define confidence basis in application logic using factors such as:

- deterministic corroboration
- multiple independent sources
- structured declaration
- semantic-only inference

If a numeric score exists, document how it is derived.

## Prompt/version provenance

Record enough metadata to reproduce or audit semantic derivations where consistent with existing Majordomus session/prompt history:

- semantic operation/extractor version
- provider/model identifier
- prompt/template version or hash
- evidence IDs/fingerprints
- output schema version

Do not create a second prompt history system if Majordomus already has one.

## Context assembly

Implement an RKS context-selection capability for coding agents.

Input may include:

- task/issue
- changed files
- current symbol/component
- branch/worktree

Use graph traversal + relevance ranking to produce a bounded context package containing:

- most relevant knowledge nodes
- supporting evidence references
- related ADRs/decisions
- related workflows/components

Avoid simply dumping the full knowledge base into context.

Return explanation metadata indicating why items were selected.

## MCP/agent integration

Expose context assembly through existing MCP/agent mechanisms where appropriate.

An agent should be able to ask for knowledge related to a source file/component/task and receive a compact, provenance-backed slice.

## Caching

Cache semantic outputs keyed by evidence fingerprints + operation/provider/prompt version so unchanged inputs do not repeatedly incur provider cost.

Invalidation must be deterministic.

## Tests

Required:

- remote processing blocked by policy
- fake secret never sent/logged
- semantic result tagged derived with provenance
- cache hit/miss based on evidence fingerprint
- changed prompt/provider version invalidates semantic cache where appropriate
- context assembly selects relevant nodes and excludes unrelated fixture nodes
- deterministic path works with zero providers configured

Use mocked/local test providers. Normal test suite must not depend on paid network APIs.

## Documentation

Document:

- deterministic vs semantic responsibilities
- remote processing policy
- data flow to providers
- provenance/audit metadata
- context assembly
- cost/cache behavior
- limitations of semantic inference

## Acceptance criteria

- RKS stays useful offline.
- Remote model use is policy-gated and secret-safe.
- Semantic claims are never mislabeled as observed facts.
- Provider/model/prompt provenance is available.
- Context assembly materially reduces irrelevant context and explains selections.
