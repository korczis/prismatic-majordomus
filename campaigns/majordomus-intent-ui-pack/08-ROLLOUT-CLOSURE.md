# 08 — RELEASE, DEPLOYMENT, VERIFICATION AND PRODUCT COMPLETION

Finish the full integration using current repo release conventions. First reconcile source state and baseline, verify schema compatibility, generated files, checks, docs and all `.ai/` gates. Merge/release incrementally under branch protection. Avoid unbounded PR accumulation or an all-at-once risky mega-PR. Do not automatically bump every individual issue/PR unless existing version doctrine explicitly requires that behavior; derive version transitions from canonical policy.

Deliver a usable launch command (`majordomus ui` where the current CLI semantics permit), discover or start the single appropriate server, show its actual URL, make startup resilient to existing server and port conflicts, include local-first safe network defaults, and make the failure mode actionable. Do not add startup network calls or builds on every invocation. Verify cold and warm times.

Create a reproducible end-to-end demonstration with a real typed demo intent in a disposable repo or safe existing test fixture. Record screenshots/browser flows of intent list/detail, action preflight, execution progress, issue/PR links, evidence and mobile navigation. Where real external CI/deploy is authorized, verify deployed code/version/health and meaningful user journeys using production URL determined from live configuration. Otherwise stop at honest `awaiting external authorization` and do not counterfeit release success.

Acceptance matrix:

| Invariant | Proof |
|---|---|
| Launch | Actual `majordomus ui` process and URL, restart behavior |
| Canonical derivation | New capability/object appears without consumer registration |
| Intent | Create/edit/validate/preflight, explain criteria and links |
| Plan | Gap + critique + milestones/issues dependency DAG |
| Execution | Actual authorized command via shared substrate |
| Live | Two clients, reconnect, replay/resnapshot, server restart |
| Mesh | Node detail, workload, logs, safe actions, peer loss |
| GitHub | Accurate linked PR/check/review and safe policy-gated actions |
| Delivery | Version and release policy plus post-deploy verification |
| Evidence | Criterion satisfaction derived, stale evidence invalidated |
| UX | Mobile narrow width, drawers, keyboard, screen readers |
| Quality | Unit/integration/E2E/security/perf/`.ai` gates pass |
| Docs | CLI/API/MCP/OpenAPI/Cockpit/site docs generated and consistent |

Conclude with a compact factual report: `what changed`, architectural reuse, deleted duplicated code, branch/PR/commit IDs, completed functionality, precise unimplemented features, tests and gates executed (with outcome), coverage and SLO numbers **only if measured**, deployment URL/version and independently verified runtime evidence (only if actually deployed), residual risks, and next smallest high-impact step. Reopen or mark blocked any failed acceptance criterion. Never claim `done` because the agent reached the end of a prompt.
