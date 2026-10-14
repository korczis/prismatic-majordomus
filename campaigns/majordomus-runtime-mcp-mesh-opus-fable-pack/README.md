# Majordomus Runtime / MCP / Mesh Convergence

A dependency-ordered implementation prompt pack for Claude Opus / Fable. Based on public repository and documentation reviewed 2026-10-09; the executing agent must re-audit actual HEAD.

## Execute

Start with `00-MASTER-ORCHESTRATOR.md`. Feed it the entire directory as context or run individual numbered missions in order. Each mission should become a milestone, and its acceptance steps should become issues in the native Majordomus planning system, synchronized to GitHub only via existing mechanisms.

## Hard constraints

Canonical source of truth, typed Rust domain, derived projections, no parallel UI/MCP state, zero false-green claims, trust-by-policy, no implicit remote execution, recovery proofs, in-repo dogfooding, quality gates, and release evidence. No production-changing action without appropriate repository policy and actual authorization.

## Sources to inspect at execution time

- https://github.com/korczis/prismatic-majordomus
- https://majordomus.dev/docs/mesh/
- https://majordomus.dev/docs/mcp/
- https://majordomus.dev/docs/surfaces/

The published Mesh guide describes Ed25519 identities, signed journal, multicast/broadcast/rendezvous, deny-unknown trust, off-by-default declaration, eventual claim conflict under partition, HTTP without transport confidentiality. Preserve these facts unless new verified implementation supersedes them.
