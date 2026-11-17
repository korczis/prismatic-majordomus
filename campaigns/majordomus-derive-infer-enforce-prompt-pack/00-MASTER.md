# MAJORDOMUS: DERIVE / INFER / VERIFY / ENFORCE
## Master autonomous implementation prompt for Claude Code Opus/Fable

You are operating in `korczis/prismatic-majordomus`. Your mission is to correct systemic drift between canonical sources, inference/derivation, runtime surfaces, guarantees, test evidence, and actual enforcement. Work against the ACTUAL checked-out repository, not assumptions in this prompt. Start with repo discovery and current HEAD; read AGENTS.md, .ai/**, relevant schemas, docs, ADRs, CI, tests, tooling and relevant implementation. Preserve current architecture. Treat the named paths below as clues to verify, not guaranteed current truths.

## Desired outcome
Every machine-derivable project fact has a single canonical owner, is inferred/derived without manual duplicate inventories, is exposed consistently through applicable CLI/HTTP/OpenAPI/MCP/Cockpit/site surfaces, is demonstrably covered by positive/negative behavioral tests, is attached to freshness-aware recorded evidence, and is enforced through actual repository gates and .ai governance. Majordomus must demonstrably apply its own rules to itself. Make it useful in projects that install Majordomus too; avoid hardcoded repo-specific behavior.

## Existing facilities: reuse, don't recreate
Inspect current versions of: `apps/majordomus-cli/src/product.rs`; `.ai/` context/rule contracts and their validators; capability registries; claims in `docs/CLAIMS.yaml`; site claims/guarantees; scenario/usecase tests; `test/run.sh`; OpenAPI projection; generated Cockpit capability pages; pre-commit derived-data freshness gates; model/mesh registry; GitHub release, versioning and Pages pipelines. Some public pages describe these features, but each claim requires current repo and runtime verification. Do not equate a web page's `guaranteed` label with a passing recorded test.

## Non-negotiable rules
1. Canonical fact has exactly one owner. All inventories, counts, versions, URLs, routes, statuses, supported features and relationships that can be inferred MUST be inferred. Editorial content may remain authored, explicitly distinguished from derived facts.
2. No second state store for a presentation layer. CLI, API, OpenAPI, MCP, Cockpit, product site, docs and completion are projections over existing typed registries, models and evidence.
3. No parallel orchestration, claim, evidence or CI framework. Extend existing facilities in place. Delete superseded duplicates.
4. Inference is deterministic, explainable, bounded and provenance-bearing; on ambiguity or absent sources report `unknown`/`unverified`, never fabricate.
5. A *guarantee* is not *proven* unless required current evidence supports it. Expose `declared status` separately from `recorded evidence verdict`; stale or not-run is never painted green.
6. Enforce with executable gates. Documentation-only doctrines are insufficient. A blocking violation must fail an applicable developer/CI gate with actionable diagnostics.
7. Tests must execute relevant behavior, including negative and mutation cases, in isolated/disposable repositories when practical. Source grep alone is not a behavioral test.
8. Respect trust boundaries: inferred/discovered peers, user inputs and remote content must not gain unauthorized execution; no credentials in projections, logs or fixtures.
9. Do not introduce heavy runtime costs into `cd`, `direnv`, local help or normal edit cycles; no network/build on shell entry.
10. Preserve existing public APIs where possible; when incompatible changes are necessary, use actual project versioning and migration policy, document and test them.
11. Avoid repository-wide refactor unrelated to an observed defect. Every implementation change must trace to an inventory item, invariant or failing proof.
12. Do not pretend to have deployed anything. Every claim of completion must link to executed commands, commit SHAs, CI runs and deployed checks.

## Autonomous work protocol
Run audit -> materialize findings -> create/update intent, acceptance criteria, issues/milestones via existing conventions -> opposition/critique -> prioritize -> implement smallest useful vertical slices -> test -> enforce -> document -> integrate -> release/deploy if authorized by existing repo conventions -> verify live -> repeat until goals are satisfied or a genuine external blocker is evidenced.

Use the existing Majordomus mesh/session/intent/worktree mechanisms where operational. If not operational, record an explicit blocking diagnostic and use the simplest safe fallback rather than writing a second mesh. Do not wait for human microdecisions about file naming. Do not merge failing/unreviewed changes merely to satisfy speed targets. Never disable gates or silently downgrade a blocking guarantee. Safe unattended operation may perform repository-convention-approved actions; otherwise record the authorization blocker.

## Initial repo audit (must precede changes)
Capture repository revision, dirty state, branch, active worktrees/PRs, versions, toolchain and actual runnable quality-gate commands. Read and inventory canonical sources and consumers for:
- project identity/version, release artifacts, supported platforms, deployment URLs;
- feature/capability registry, command inventory and routing;
- governance `.ai` policies, rules, doctrines, schema and generated provider files;
- intents, milestones, issues, sessions, handovers, knowledge and evidence;
- mesh/peers and trust metadata;
- site landing/product/docs/guarantees and Cockpit UI;
- CLI, HTTP/OpenAPI, MCP and shell completion.

For each derived output produce a machine-readable matrix:
`fact_id | authoritative source | inference function | consumers | required evidence | validation gate | existing test | observed divergence | severity | proposed correction`.
Collect objective reproducible examples of stale versions/counts, wrong links, wrong status labels, untested guarantees, duplicated inventory arrays and release/site mismatch. Investigate the publicly observed class of drift: homepage and some detail pages potentially presenting different versions. Treat this as a hypothesis until confirmed in the current checkout and actual deployment; find underlying build/projection source, not merely replace visible strings.

## Architecture correction
Map each canonical input to a typed provenance-aware domain object and deterministic, stable projection contract. Prefer adapting existing structures and generators. When different consumers need distinct presentation, separate editorial composition from machine-owned facts. Build once/derive everywhere rather than copy values into JSON, JS, Markdown or HTML. Verify OpenAPI is derived from real routes/schemas, docs from same semantic description, MCP from same capability registry, Cockpit from same model, and site release/version metadata from actual release ledger. Define canonical sorting semantics for arrays and total stable tie-breakers; avoid incidental filesystem/hash/concurrency order.

## Acceptance: end-to-end invariant
Pick three representative mutations (e.g. newly added capability, changed supported release platform/version, changed claim/evidence verdict), performed in isolated fixtures. Prove canonical change propagates to EVERY applicable surface without editing consumer-specific inventory. A consumer that cannot be generated must be explicitly identified as editorial or an architectural gap. Change a derived file manually and prove the relevant gate rejects it, with exit code and corrective explanation. Break an enforced `.ai` rule and prove both local and CI-equivalent refusal. Break a behavioral test for a public guarantee and prove the evidence verdict is failing or stale, never proven. Add a new guaranteed claim without valid test linkage and prove refusal. Assert repeat invocations give stable structured output.

## Integration and delivery
Implement focused work described in 01..06 prompts. Every wave updates tests and documentation in the same PR. Re-run canonical checks and examine generated drift. Reconcile PRs carefully: prioritise merging ready vertical slices rather than producing unbounded branches. Respect actual Elm-like version logic and established release gates; do not hardcode minor bumps or claim a release happened without tag/artifact/deploy confirmation. Produce before/after evidence and a concise factual report with accomplished scope, open blockers, measurable CI/PR turn times, test outcomes and links to exact proof. Only stop when acceptance criteria are satisfied or external blocker is concrete.
