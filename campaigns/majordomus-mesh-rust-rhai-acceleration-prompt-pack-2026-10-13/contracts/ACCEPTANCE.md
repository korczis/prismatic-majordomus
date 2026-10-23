# Acceptance gates

| Gate | Required proof |
|---|---|
| G0 current truth | HEAD+timestamps, remote CI, PR queue and actual deployed SHA recorded |
| G1 governance | canonical context/rules read, declared scope, no collisions |
| G2 Rust authority | one actual Rust operation dispatcher across relevant adapters, differential tests |
| G3 Rhai | one bounded script invokes authorized read-only canonical operation, negative limits tests |
| G4 plugin | external package discovered outside source checkout with manifest compatibility and explicit grant |
| G5 zero registration | plugin capability appears across ALL authorized projections without manual consumer edits |
| G6 shell reduction | one meaningful shell domain behavior ported, parity tests, residual wrappers thin |
| G7 graph UX | canonical identities, deep links, useful drilldown, mobile/accessibility, before/after performance |
| G8 CI credibility | P0 no regressions, tests actually run, fresh vs stale evidence explicit |
| G9 release | green required trunk checks + version/tag/artifacts + published SHA + smoke proof |
| G10 speed | measured merge→deploy and graph timings, no invented efficiencies |

### Hard refusals

No skipped required checks, bypassed branch protection, falsified coverage/benchmarks, destructive cleanup of unmerged work, unauthorized plugin privilege, invented API/MCP tool names, duplicate capability registries or unsupported 'deployed' claims.

### Artifact guarantees

Each changed capability must be discoverable/typed, documented, tested, benchmarked where meaningful, included in current `.ai` knowledge/policy as appropriate, reflected in generated CLI/API/MCP/peer/UI/GH Pages output where permitted, and validated against drift. An absence of a transport means mark it unsupported or unavailable, not silently claim success.
