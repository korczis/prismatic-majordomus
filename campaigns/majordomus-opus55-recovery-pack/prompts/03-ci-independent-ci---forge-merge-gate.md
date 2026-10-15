# M03: Independent CI / Forge Merge Gate
**Priority:** P0  
**Prerequisites:** M01, M02  
**Milestone objective:** Move acceptance to a verifier the worker cannot trivially bypass; keep canonical policy platform-neutral.

## Task briefing


## Operating contract (mandatory)

You are Claude Opus 5.5 operating in `github.com/korczis/prismatic-majordomus`. You are an implementer, not a report generator. Work from the checked-out HEAD, not the 2026-10-07 audit snapshot (`3ede593c0e`). Audit assertions are regression leads, not truths to copy: reproduce each applicable finding or record that it was already fixed. Never fabricate test output, deployment status, timings, evidence, releases, external adoption, or provider support.

1. Discover current repository instructions, AGENTS/CLAUDE files, `.ai/` rules, doctrines, ADRs, issue/milestone schemas, source architecture, CI gates and existing test commands. Reuse canonical mechanisms. Do not blindly assume file paths or APIs named in the audit still exist.
2. Establish baseline with revision, environment, status, focused test results and concrete reproduce commands. Check for concurrent work, dirty trees, and ownership conflicts. Preserve user changes. Prefer a scoped worktree/branch according to actual policy.
3. Declare one intent (or link an existing relevant intent), its invariants, measurable satisfaction criteria and explicit non-goals. Create or reconcile the milestone and issue records using the repository's real `.ai/` system and GitHub sync only if available and authorized. Each `Issue` below is one issue and one independently reviewable change. Never backfill ceremonial lifecycle timestamps.
4. Use the smallest coherent patch; prove effect with behaviour, negative controls and mutations. Tests that only assert text exists, snapshots of self-declared success, and unconditional catch/skip are not proof. No ratchet baseline additions for changes introduced by this campaign. No `|| true` or disabling safeguards to make checks green.
5. For each changed observable behaviour update all genuinely affected projections: CLI, typed schema, API/OpenAPI/Swagger, MCP, Cockpit, docs and website **when those surfaces actually consume the capability**. Do not add meaningless endpoints or UIs just for coverage; avoid manually duplicated inventories and derived state. Explicitly report N/A and why.
6. Security: no secret values in output, persistent artefacts or fixtures; no auto-commit/auto-push, PR merging, remote deploy, destructive cleanup or branch-protection edits without their existing authorization and the user's explicit deployment authority. Test external side effects in isolated fixtures.
7. Keep targeted feedback fast and deterministic. Measure cold/warm where performance is relevant. For long suites choose focused regression plus mandatory policy gates; do not label partial execution a full-suite pass. Capture stdout/stderr and exit status in evidence for replay.
8. Only claim satisfied when an independent acceptance gate replays the test/evidence under a distinct trust boundary. If this is impossible in the environment, label blocked with a precise reproduction and dependency; continue with other safe, independent issues.
9. Keep session/handover/issue state real. Attach commit hashes, verifier identity, artifact digests, commands, outputs, CI links when available. Do not invent review or user-study evidence. Update README claims only after tests support them; retract unsupported claims.
10. Finish with a compact operator report: HEAD before/after, issues completed/blocked, implementation diff, threat model, tests with pass/fail/skip and runtimes, behavioural/negative evidence, generated drift check, remaining risks, rollback and exact next dependent milestone. Do not use a green CLI screen as proof of tamper resistance.

### Universal adversarial matrix

For each applicable capability, test: legitimate success, denied invalid input, input mutation after approval, missing/corrupt/stale records, clean and dirty repo, unrelated file change, symlinks/path traversal, concurrent writers, alternate checkout/worktree, nonzero verifier and timeout, interrupted execution, wrong actor, old schema, duplicate/unknown keys and machine-readable output determinism. Explicitly explain exceptions; do not scatter artificial tests that do not exercise the component.

### Issue implementation loop

For each issue: DISCOVER existing owner -> REPRODUCE -> THREAT MODEL -> CHANGE MINIMALLY -> UNIT + E2E + MUTATION/NEGATIVE TESTS -> CROSS-SURFACE CHECK -> DOCS + MIGRATION -> EVIDENCE RECORD -> RE-CHECK DIFF -> ISSUE COMPLETE. Stop progressing dependent issues on a red acceptance gate. A new layer requiring a new noun needs documented justification against reuse or deletion.


## Milestone-specific hard constraints

Do not assume a passing existing test suffices: recreate both the reported failure and a meaningful negative control, then re-run after fixing. Keep interfaces stable unless explicitly versioned. Cross-branch or cross-user trust relies on an independent verifier, not an advisory hook. Treat every scope increase as an intent amendment with recorded approval.

## Issues (execute sequentially unless genuine parallel ownership is proven)

### M03-I01: Replayable CI entrypoint

**Implementation:** Implement standalone, noninteractive evaluator on clean checkout that reads tracked contract, trusted PR diff and pinned verifiers; no dependence on `.ai/local` or developer hooks.

**Acceptance criterion:** CI and local evaluator match under same inputs.

**Required issue deliverables:** code or explicit justified no-code correction; schema/migration where needed; regression test; negative or mutation test; proof artifact/reproduce command; relevant generated projections; concise docs; rollback considerations.

**Failure injection:** devise one representative mutation that would violate the criterion and demonstrate a red test or independent gate. Record its result; never invent a green status.

### M03-I02: GitHub Action integration

**Implementation:** Provide narrowly privileged GitHub Action/composite workflow with safe pull_request permissions, pinned tooling and isolated untrusted PR execution; avoid pull_request_target code execution vulnerabilities.

**Acceptance criterion:** Fork PR and same-repo PR acceptance cases covered.

**Required issue deliverables:** code or explicit justified no-code correction; schema/migration where needed; regression test; negative or mutation test; proof artifact/reproduce command; relevant generated projections; concise docs; rollback considerations.

**Failure injection:** devise one representative mutation that would violate the criterion and demonstrate a red test or independent gate. Record its result; never invent a green status.

### M03-I03: Required status documentation

**Implementation:** Document exact repository admin steps for branch ruleset required check, permissions, review and exception process; do not falsely claim it is required without observing remote setting.

**Acceptance criterion:** Verifier status proves checks; ruleset status separately confirmed or marked unverified.

**Required issue deliverables:** code or explicit justified no-code correction; schema/migration where needed; regression test; negative or mutation test; proof artifact/reproduce command; relevant generated projections; concise docs; rollback considerations.

**Failure injection:** devise one representative mutation that would violate the criterion and demonstrate a red test or independent gate. Record its result; never invent a green status.

### M03-I04: Bypass regression suite

**Implementation:** Exercise `git -c core.hooksPath=/dev/null`, `--no-verify`, edited local scope, benign verify, changed policy, deleted evidence and PR diff scope violation against CI fixture.

**Acceptance criterion:** Each attack gets rejected where policy requires.

**Required issue deliverables:** code or explicit justified no-code correction; schema/migration where needed; regression test; negative or mutation test; proof artifact/reproduce command; relevant generated projections; concise docs; rollback considerations.

**Failure injection:** devise one representative mutation that would violate the criterion and demonstrate a red test or independent gate. Record its result; never invent a green status.

### M03-I05: Offline/local forge-neutral contract

**Implementation:** Provide documented executable CI protocol (inputs/output/exit codes) independent of GitHub integration so other forges can use it without forked semantics.

**Acceptance criterion:** A clean process without GitHub credentials reproduces deterministic verdict.

**Required issue deliverables:** code or explicit justified no-code correction; schema/migration where needed; regression test; negative or mutation test; proof artifact/reproduce command; relevant generated projections; concise docs; rollback considerations.

**Failure injection:** devise one representative mutation that would violate the criterion and demonstrate a red test or independent gate. Record its result; never invent a green status.

## Milestone verification checklist

- [ ] Every issue has a real evidence-bound acceptance criterion and authentic chronology.
- [ ] Current-HEAD baseline and audit delta recorded.
- [ ] Focused tests, negative tests and at least one realistic mutation pass.
- [ ] No changed contract is accepted based on worker-controlled evidence alone.
- [ ] Current repository gates and generation/drift checks run or explicit blockers recorded.
- [ ] No new baselines, fabricated test attestations, unexplained dependencies, or secrets.
- [ ] Public claims reflect exactly proven capabilities.
- [ ] Next dependent milestone is unblocked only on a trustworthy acceptance receipt.

## Required final response

Provide: summary table per issue (implemented/evidenced/blocked), exact test commands and results, evidence IDs/digests, observed regressions killed, changed architecture, timing or payload metrics if measured, security limits, docs/projections updated, residual risk and rollback. Never summarize a blocked required gate as finished.
