# Phase 13: Security, Authorization, Trust Boundaries and Safe Autonomy

## Threat model
The UI, MCP clients, agent workers, untrusted GitHub content, provider outputs, mesh peers, external URLs, logs and project files cross distinct trust boundaries. A working autonomous loop without meaningful policy checks is a remote code execution system with a pleasant dashboard.

## Capability authorization
- Establish actual security model and extend it, not parallel ACLs.
- Query vs effect capabilities must be distinct and permissioned.
- Explicit authority for issue create/edit, branch push, merge, tag/release, deploy, rollback, cleanup and external API changes.
- Evaluate policy before scheduler assignment AND immediately before irreversible effect.
- Authorization scopes include repository, target environment, branch, path scope, effect kind, time/budget and actor.
- Record approve/deny decisions with policy version, input digest, actor and expiry.
- Support safe preset tiers: observe, propose, sandbox execute, integrate, release, deploy. Names optional; semantics required.
- Production deploy and irreversible cleanup disabled unless an explicit existing project policy authorizes them.
- Human emergency stop cancels future work and safely drains in-flight attempts. Cannot unsend already completed external effects.

## Prompt injection containment
- Treat issue body, PR comments, logs, documentation loaded from other repos, model suggestions, web and MCP tool outputs as untrusted.
- Never treat quoted/source text as higher authority than `.ai` project policy and runtime authorization.
- Agent proposals cannot grant their own permissions or modify effective policy to bypass controls.
- Changes to governance/enforcement require separate verification and protected review policy.
- Disallow tool-argument smuggling from UI URLs/markdown or untrusted project artifacts.
- Do not assume any LLM-generated plan satisfies criteria before machine validation.

## Mesh security
- Reuse authenticated peer identities and existing wire semantics.
- Expired/stale/revoked credential paths fail closed; show diagnostic.
- Claims cannot authorize effects; claim only prevents overlap per its actual contract.
- Reconcile split-brain/partitions without assuming global lock semantics.
- No cross-repo peer action without explicit scope.
- Encrypt channels according to existing transport; do not invent cryptography.

## Secrets and logs
- Never expose provider API keys, deployment tokens, personal prompts, raw `.ai/local`, hidden paths or environment secrets in browser/API/MCP/docs.
- Use typed redaction, tested both for structured fields and free-form output.
- Separate public docs from private runtime event streams.
- Avoid persistent storage of unnecessary LLM transcripts; store bounded context and artifact hashes.
- Audit logs are append-only, sanitized and access-controlled.

## Negative tests
- unauthorized deploy denied even with UI button forged
- stale approval token denied
- mutated approval after plan revision invalidated
- injected issue attempts to disable tests
- untrusted peer sends false completion receipt
- stolen/replayed request ID does not duplicate effect or bypass actor binding
- credentials absent, revoked, expired and malformed
- hostile worker output includes token-shaped secret
- cross-repo entity address rejected
- local-only `.ai` state never appears on public Pages/OpenAPI samples
- cleanup cannot delete unmerged user work
- production action stops safely on policy ambiguity
