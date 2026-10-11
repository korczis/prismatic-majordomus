# Canonical surfaces and user flows

## User journeys
1. Upload ZIP in mobile/desktop UI → progress → validation errors or preview → revision detected → proposed graph diff → authorized run.
2. Open pack → revision history → documents → segment → criterion → issue → owner/peer → PR → CI evidence → deployed release.
3. Update pack → show unchanged/superseded/new requirements and impacted execution → explicit accepted revision.
4. Resume after machine restart → status and last verified checkpoint, no duplicate PR.
5. Launch `majordomus prs drain` or equivalent existing command → see queue, policy gate, conflict, merge and verification.

## Required read model
Stable IDs, canonical default ordering, pagination, filtering, membership, source location, evidence links, peer/lease state, timestamps with provenance, blocked reasons and remediation, revision pin. Map to CLI JSON, REST/OpenAPI, MCP and UI from ONE typed model. UI never owns completion or status semantics. Keep interactive filters as presentation overlays.

## UX quality gates
Mobile-first drawers, accessible keyboard navigation, meaningful loading/error/empty states, deep links, stale-event reconciliation, large pack virtualization and readable long names. Use current repo UI stack; Flowbite/Tailwind/Alpine/Cytoscape only if present/appropriate. Navigation is derived from supported capabilities, not manually cloned inventories.
