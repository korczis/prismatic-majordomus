# WAVE 06 — Non-negotiable enforcement and adversarial testing

Review existing rules/doctrines first and amend the nearest canonical rule; create a new one only if needed. Bind it to real test/CI enforcement, not decorative prose. At minimum enforce:
- Single canonical identity and no independent overlapping authority.
- Hooks bounded/non-blocking by default; event replay/idempotency and causal cycle bounds.
- Inferred/proposed never silently promoted to observed/verified.
- Canonical privacy zones; `.ai/local` never published; secret redaction at boundaries.
- Mesh trust/claims obey current model; mesh events cannot execute unapproved code.
- Typed deterministic ordering and cross-surface projection; no frontend semantic copies.
- Safe LLM budgets, explicit unavailable states and cost attribution.
- Validated state transition, evidence and reason on promotion/rejection.

Threat modeling: prompt injection from untrusted repo text and external messages; malicious MCP/tool returns; forged/stale event; signature replay; spoofed repository identity; public private-state leak; path traversal; credentials in telemetry; concurrent actor lost update; denial of service from hook storms; unbounded peer fanout; model output JSON/schema attacks; shell injection; invalid task scope; downgrade of governance.

Test classes: unit/property (permuted and randomized event sequences), concurrency/chaos (partition/reconnect/replay), end-to-end hook-to-UI, schema migration/backcompat, publication privacy scanning, latency and memory benchmarks, generated artifact drift and missing advisor/offline. Preserve negative control fixtures proving gates refuse bad behavior. Measure warm/cold paths on target environment and define CI budget based on baseline; no made-up subsecond guarantee.

Run actual repository gates: formatting, build, `majordomus check`, `knowledge check`, `skills check`, `reasoning check`, use-case impact/coverage, `scripts/derive`, relevant `scripts/ci/*`, site builds, tests. Discover exact commands and avoid running placeholder commands blindly. Distinguish preexisting unrelated failures from regressions. Do not dilute gates to secure green CI.
