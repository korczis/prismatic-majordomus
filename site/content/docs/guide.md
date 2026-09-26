+++
title = "Guide"
description = "what a team gets from the tool and why, then every part of it in turn: how it works, the commands to see it, the test cases that prove it and how finished it is; the first ten minutes; running the proof yourself and keeping the run as evidence; and what it does not do yet"
weight = 11
[extra]
source = "docs/GUIDE.md"
+++

{% raw %}

This is the practical companion to two other documents. [`CONCEPTS.md`](@/docs/concepts.md) is the
vocabulary; [`HOW_IT_WORKS.md`](@/docs/how-it-works.md) is the architecture. This one goes feature by
feature: what problem it removes, how it works, the commands to see it, and the test cases that
prove it. Every test named here is a real file in this repository, and
[Proving it yourself](#proving-it-yourself) runs them.

Maturity is stated per feature. "On master" means merged and tested. "Branch only" means the work
exists in an open pull request and is not a feature yet.

## Why a team wants it

AI coding agents are good workers and poor custodians. The cost a team pays is not in the code
they write; it is in everything around it. Each row below is a cost Majordomus removes, the
mechanism that removes it, and the test that shows the mechanism works.

<div class="overflow-x-auto" tabindex="0">

| The cost today | What Majordomus does | What the team gets | Proved by |
|---|---|---|---|
| every new session re-learns the project; decisions get re-argued | compiles context from repository state; handovers record the commit they describe and are shown as history once stale | a worker from any provider continues without the previous chat, and knows when a note is out of date | `test/cases/25_continuity_lifecycle.sh`, `test/cases/127_briefing_freshness.sh`, `test/cases/62_session_divergence.sh` |
| agents change files outside what they were asked to touch | every task declares a scope; `check`, `finish` and the pre-push hook refuse changes outside it | reviewable changes with a boundary a machine enforces | `test/cases/04_start_check.sh`, `test/cases/06_finish.sh` |
| parallel agents collide in the same directories | worktrees at a path derived from the branch; a repository-wide board where workers announce scope; overlap reported when it is created | collisions visible before they become merge conflicts | `test/cases/96_worktree_topology.sh`, `test/cases/250_the_board_is_repository_wide.sh`, `test/cases/114_two_worktrees_one_repository.sh` |
| rules in instruction files that nothing checks | rules declare how they are enforced; a doctrine's validator is dispatched by the commands; `doctor` verifies the wiring; the rule proof says which rules anything actually runs | a governance file you can trust, because a broken link fails a build | `test/cases/17_doctrine_enforcement.sh`, `test/cases/18_doctrine_wiring.sh`, `test/cases/125_rule_proof.sh` |
| "tests pass" and "done" as unverifiable claims | a finish contract that refuses until scope, verification, gates, blockers and publication are satisfied; evidence recorded with the commit it proves | completion that means something, and a record of why | `test/cases/06_finish.sh`, `test/cases/32_refusal_lifecycle.sh`, `test/cases/124_evidence.sh` |
| a flat backlog with hand-edited statuses that go stale | status derived from dependencies, timestamps and evidence; execution waves; scope conflicts inside a wave | a plan that cannot contradict itself, and a safe order to parallelise | `test/cases/41_project_status.sh`, `test/cases/42_dag_waves.sh`, `test/cases/133_plan_transition.sh` |
| different tools and docs describe the project differently | one instruction policy projected into each provider's file; one capability registry behind CLI, MCP, HTTP, OpenAPI, the Cockpit and the reference; drift regenerated and compared in CI | every person, agent and interface reads the same state | `test/cases/03_update.sh`, `test/cases/76_capabilities_projections.sh`, `test/cases/56_derived_current_gate.sh` |
| a deploy "succeeded" and nobody checked the public site | publication verified against the commit the public site serves | release status you can believe | `test/cases/180_publication_is_observed.sh`, `test/cases/275_publication_is_current.sh` |

</div>


What it deliberately is not: a model, a coding agent, an orchestrator that launches workers, a
memory database, or a lock manager. It works beside whatever agents a team already uses and
calls no model itself.

## The first ten minutes

```console
$ curl -fsSL https://majordomus.dev/install.sh | sh     # installs majordomus and majordomus-cli
$ cd your-repository
$ majordomus init                                       # creates .ai/ and the provider bootstraps
$ majordomus doctor                                     # read-only: is everything declared actually wired?
$ majordomus start "fix login redirect" --scope apps/web/auth
$ majordomus context                                    # what a worker needs, compiled, within a budget
$ majordomus check                                      # scope, state, blockers, selected gates
$ majordomus handover < note.md                         # or: finish --outcome completed --verify-command "make test"
```

For an AI client, the repository's MCP server exposes the same state. `bin/majordomus-mcp` is
what `.mcp.json`, `.codex/config.toml` and `.gemini/settings.json` start, and
`majordomus-cli serve status` prints the local URL of the Cockpit, the API and Swagger UI.

## How it works, part by part

### The layer and `doctor`

**How it works.** `init` creates `.ai/`: `manifest.yaml` says which section lives where,
`repo/` is tracked, and `local/` is git-ignored. A repository counts as supervised when the
manifest exists. `doctor` is read-only. It dispatches every doctrine bound to it: layout,
schemas, policy, the vendored rule package against its hashes, the doctrine wiring chain,
provider files against their generation stamps, and hook wiring. Every finding carries a
reproduce command.

**Try it.** `majordomus init`, `majordomus doctor`, `majordomus doctor --json`.

**Proved by.** `test/cases/01_init.sh`, `test/cases/02_doctor_basic.sh`,
`test/cases/68_ai_discovery.sh`, `test/cases/32_schema_integrity.sh`,
`test/cases/83_doctor_budget.sh`.

**Maturity.** On master.

### One policy, a file for every agent

**How it works.** `.ai/repo/policy.yaml` is the provider-neutral policy. `majordomus update`
renders `CLAUDE.md`, `AGENTS.md` and the other provider files from it and from templates in
`.ai/repo/providers/`. Each file carries a stamp with the policy hash and its own content hash, so
a hand edit is detected rather than overwritten silently.

**Try it.** `majordomus update --dry-run`, `majordomus update --diff CLAUDE.md`, `majordomus update`.

**Proved by.** `test/cases/03_update.sh`, `test/cases/13_region_projection.sh`.

**Maturity.** On master.

### Task and scope

**How it works.** `start` records one active task per checkout, with a normalised scope and
optional obligations (`--requires tests,docs,…`). `check` fails every file changed since the
task began that falls outside the scope. `finish` refuses the outcome `completed` for the same
reason, and `finish --check` runs in the pre-push hook. `start` and `check --overlap` also report
other worktrees whose active tasks claim overlapping paths. That report is advisory and exits 0.

**Try it.**

```console
$ majordomus start "rate limiting" --scope apps/api/limits --requires tests
$ majordomus check
$ majordomus check --overlap
$ majordomus finish --check
```

**Proved by.** `test/cases/04_start_check.sh` (scope refusal; overlap reported with exit 0),
`test/cases/27_foreign_task.sh`, `test/cases/108_scope_after_trunk_merge.sh`,
`test/cases/93_scope_policy.sh`, `test/cases/103_obligations.sh`.

**Maturity.** On master, and enforced at commit and push time.

### Profiles

**How it works.** A profile under `.ai/repo/profiles/` fixes how much ceremony a kind of work
gets: context sections, budgets, whether a verify command is required, the checkpoint interval.
`start --profile` selects one; the policy names the default.

**Proved by.** `test/cases/16_profiles.sh`.

**Maturity.** On master.

### Sessions and episodes

**How it works.** Provider hooks call `majordomus capture session --event start|compact|end`.

- **Start** opens an episode, makes sure this checkout's shared server runs, and prints a briefing.
- **Compact** derives a checkpoint.
- **End** derives a checkpoint and a handover, then closes only that provider's own episode.

A closed episode becomes an immutable session record that references what was produced and copies none of it. Closing happens exactly once, even under concurrency, and an episode nobody ended is detected.

**Try it.** `majordomus session list`, `majordomus session show <id>`.

**Proved by.** `test/cases/54_session_lifecycle_hooks.sh`, `test/cases/60_session_lifecycle.sh`,
`test/cases/61_session_envelope.sh`, `test/cases/220_exactly_once_close.sh`,
`test/cases/222_an_episode_nobody_ended.sh`, `test/cases/170_provider_lifecycle_declared.sh`.

**Maturity.** On master for Claude Code, the one provider with declared lifecycle hooks. Other
clients use the same records and MCP surface without automatic capture.

### Context

**How it works.** Three mechanisms, kept apart:

1. **Context documents.** A context document is any Markdown file under `.ai/` declaring `schema: context/v1` and a scope. `context resolve <path>` composes the documents that apply to a path, least specific first. Each adds to its ancestors, and superseded documents drop out. `context explain` says why each one applies. `context affected` names the documents a change touches.
2. **Compiled session context.** `context` assembles git state, peers, the task, the profile, the applicable documents, open questions, decisions, the last checkpoint and handover, relevant files and history, fitted to a line budget:
   - sections are dropped in a fixed order;
   - checkpoint and handover shrink to a pointer before they disappear;
   - git, task, profile and questions are never dropped;
   - every drop is listed under EXCLUDED;
   - still over budget exits 10, never truncating silently.
3. **Ranked working set.** `majordomus-cli devcontext compile` walks the object graph from a task, ranks by authority tier and relevance, and spends a token budget. It names everything it skipped. Its only inference, keyword intent, is capped below every declared relation.

**Try it.**

```console
$ majordomus context --budget-lines 120
$ majordomus context resolve apps/web/auth
$ majordomus context explain apps/web/auth
$ majordomus-cli devcontext compile --help
```

**Proved by.** `test/cases/23_context.sh`, `test/cases/55_session_context.sh`,
`test/cases/69_context_documents.sh`, `test/cases/70_context_impact.sh`,
`test/cases/71_context_sync.sh`, `test/cases/106_context_peers.sh`,
`test/cases/130_context_compiler.sh`.

**Maturity.** On master. The shell `context` command does not use the Rust ranking yet.

### Checkpoints and handovers

**How it works.**

- **Checkpoint.** A capped progress note while work continues. It resets the staleness clock that `check` measures.
- **Handover.** An append-only record for whoever continues. The body is authored and must have `# Objective`, `# Current State` and `# Next Action`. The front matter is derived: repository, worktree, branch, commit, working-tree state and changed files. A body that claims its own identity is refused.

`handover --resolve` finds the newest record for the same worktree and branch, then the same branch, never repository-wide. It labels the commit relation as exact, advanced, diverged or a different context, and prints the freshness verdict from the policy's thresholds (fresh, aging or stale); a stale record is marked as history, to be read as context rather than as an instruction. The session briefing applies the same verdict and does not quote a stale handover's next action. A blocker recorded before a handover is still a blocker after it.

**Try it.**

```console
$ majordomus checkpoint --derive
$ majordomus handover --derive
$ majordomus handover --resolve
```

**Proved by.** `test/cases/20_checkpoint.sh`, `test/cases/05_handover.sh`,
`test/cases/25_continuity_lifecycle.sh`, `test/cases/62_session_divergence.sh`,
`test/cases/127_briefing_freshness.sh`, `test/cases/131_freshness_has_an_age.sh`,
`test/cases/50_blocker_across_handover.sh`.

**Maturity.** On master.

### Decisions, open questions and ADRs

**How it works.**

- **Decisions.** Recorded with what was decided, why and what was rejected. They are superseded by a later entry, never edited.
- **Open questions.** Stored as state. One that names the active task refuses `finish --outcome completed`.
- **ADRs.** Live in `.ai/repo/adrs/` and are `proposed` until a person accepts them. The tool refuses a record that accepts itself, and ADR numbers are allocated without collisions.

**Try it.** `majordomus decision add "<what>" --why "<rationale>"`,
`majordomus question add "<question>"`, `majordomus adr list`, `majordomus adr next`.

**Proved by.** `test/cases/21_decision_question.sh`, `test/cases/99_adr.sh`,
`test/cases/271_adr_allocation.sh`.

**Maturity.** On master.

### Planning: milestones, issues, status and waves

**How it works.** Milestones and issues are YAML in `.ai/repo/project/`. An issue declares dependencies, scope, acceptance criteria, validation, required evidence and transition timestamps, and has **no status field**. Status is derived:

1. **Done:** not cancelled, completed, and every required evidence token covered.
2. Otherwise the first that applies of **verify**, **active**, **blocked** (a dependency not done) and **ready**.
3. A status ahead of its blockers is reported as premature execution.

**Waves** are the Kahn layering of the dependency graph. Cycles get no wave and are reported. Ready issues in the same wave that share paths produce a scope-conflict warning: dependencies say what could run in parallel, scope says what safely can. `plan.transition` moves an issue only when the derived plan allows it.

**Try it.**

```console
$ majordomus plan status
$ majordomus plan waves
$ majordomus plan next
$ majordomus plan validate
```

**Proved by.** `test/cases/40_project_model.sh`, `test/cases/41_project_status.sh`,
`test/cases/42_dag_waves.sh`, `test/cases/43_plan_command.sh`,
`test/cases/133_plan_transition.sh`, `test/cases/99_plan_capabilities.sh` (the Rust reader is
byte-identical to the shell engine).

**Maturity.** On master. Intent, a typed record the plan serves, is branch only.

### Worktrees

**How it works.** Parallel work lives in git worktrees at a path derived from the branch,
`<repository>-wt/<branch>`, with nothing registered. The pre-commit hook refuses a feature-branch
commit from anywhere else. `worktree migrate` moves misplaced worktrees with fingerprints taken
before and after.

**Try it.** `majordomus-cli worktree create feature/x`, `majordomus-cli worktree status`,
`majordomus-cli worktree migrate --plan`.

**Proved by.** `test/cases/96_worktree_topology.sh`,
`test/cases/114_two_worktrees_one_repository.sh`, and the crate tests in
`apps/majordomus-cli/tests/worktree.rs`.

**Maturity.** On master, and enforced at commit time.

### The shared server, the board and overlap

**How it works.**

- **The server.** Each checkout runs one shared server, chosen through a lease file. It survives a crash of its owner. `serve ensure` converges on one, and `serve status` reports whether it is ready, outdated or stale.
- **Peers.** Every MCP client that attaches is a peer.
- **Announcing.** A worker announces an intent and a scope with `majordomus_announce`.
- **The board.** `majordomus_peers` gathers the boards of every worktree of the repository, stamps each peer with its checkout, and says whether the answer is complete.
- **Overlap.** Two scopes meet when they are equal or one contains the other on a path boundary.

Overlap is shown on announce, in the peer list and in `context`. It is awareness, not a lock: the board lives in memory and is not persisted.

**Try it.** `majordomus-cli serve status`, `curl -s http://127.0.0.1:8742/api/v1/peers`,
`majordomus context` (the PEERS section).

**Proved by.** `test/cases/90_mcp_shared_server.sh`,
`test/cases/113_shared_server_survives_a_crash.sh`,
`test/cases/250_the_board_is_repository_wide.sh`, `test/cases/106_context_peers.sh`, and
`apps/majordomus-cli/tests/peer_claims.rs`.

**Maturity.** On master, repository-wide on one machine.

### The mesh

**How it works.** Majordomus instances can discover each other beyond one machine:

- an Ed25519 node identity per user;
- signed, bounded envelopes over UDP multicast, UDP broadcast or an HTTP rendezvous endpoint;
- a registry with a presence TTL;
- deny-unknown trust, which changes only labels.

It is observation: nothing is executed remotely.

**Try it.** `majordomus-cli mesh identity`, `majordomus-cli mesh status`,
`majordomus-cli mesh doctor`.

**Proved by.** `test/cases/130_mesh.sh`, `apps/majordomus-cli/tests/mesh.rs`.

**Maturity.** On master, off by default (`.ai/repo/mesh/majordomus.yaml`). There is no Tailscale
or mDNS provider. The peer board and the claims replicate across runtimes, on one machine or
several, over authenticated links (ADR 0067).

### Rules, doctrines and the rule proof

**How it works.**

- **Rules.** A rule is a versioned Markdown file with a statement, a class (blocking or advisory), dependencies, and optionally an `x-majordomus` block. Its enforcement mode is derived from that block:
  - a validator a command runs makes it *dispatched*;
  - tests a gate runs make it *gated*;
  - only a stated reason makes it *reviewed*;
  - nothing makes it *declarative*.
- **Doctrines.** The dispatched rules. `doctor` checks each doctrine's chain: validator, dispatching command, failure propagation, test and claim. The check is mutation-tested.
- **Rule proof.** The Rust rules engine reduces every rule to one proof state, from proven through gated and not run to dangling and unproven. A named proof that does not exist fails the `rule-proof` gate.

**Try it.**

```console
$ majordomus doctrine list
$ majordomus rules list
$ majordomus-cli rules report
$ majordomus-cli rules show majordomus.scope-integrity
```

**Proved by.** `test/cases/17_doctrine_enforcement.sh`, `test/cases/18_doctrine_wiring.sh`,
`test/cases/14_wiring_dispatcher.sh`, `test/cases/67_rule_dag.sh`,
`test/cases/133_rule_graph.sh`, `test/cases/125_rule_proof.sh`,
`apps/majordomus-cli/tests/rules.rs`.

**Maturity.** On master. Most rules stand at `not run` rather than `proven`, because CI does not
yet record its runs into the evidence ledger.

### Claims and evidence

**How it works.**

- **Claims.** `docs/CLAIMS.yaml` holds the public claims, each with its source, implementation, test and status, published as `/guarantees/`.
- **Evidence.** A recorded execution in `.ai/repo/evidence/ledger.json`: test, runner, outcome, commit, working-tree state, digest and origin.
- **Proof states.** Derived from the ledger and the tree, not from time:
  - *proven*: nothing changed since the run;
  - *inputs unchanged*: something changed, but not this claim's own inputs;
  - *stale*: its inputs changed;
  - *failing*, *not run*, *unrunnable*, *no test*.

**Try it.**

```console
$ majordomus-cli evidence show
$ majordomus-cli evidence claim scope-enforcement
$ MJ_TEST_REPORT=report.tsv bash test/run.sh 04_start_check
$ majordomus-cli evidence record --suite report.tsv
```

**Proved by.** `test/cases/124_evidence.sh`, `test/cases/77_rust_evidence.sh`.

**Maturity.** On master. Recording CI runs automatically is branch only.

### Gates and the finish contract

**How it works.** Gates are repository-owned checks declared in `.ai/repo/ci/gates.yaml`, with
path classes deciding which ones a change selects; CI is a thin adapter over that plan. `check`
lists the gates a task's change selects, and a gate that never reported is `queued`, never green.

`finish --outcome completed` walks the policy's `finish_requires`: scope respected, verification ran, state updated, no open blockers, note present, obligations met, gates passed, publication current. Any unmet line refuses with exit 10 and names what is missing. A refused finish is a normal lifecycle event, not an error to work around.

**Try it.** `majordomus finish --check`,
`majordomus finish --outcome completed --verify-command "make test"`.

**Proved by.** `test/cases/06_finish.sh`, `test/cases/32_refusal_lifecycle.sh`,
`test/cases/94_ci_plan.sh`, `test/cases/131_completion_gates.sh`,
`test/cases/138_governance_gates.sh`.

**Maturity.** On master, and enforced in the pre-push hook.

### One registry behind every interface

**How it works.** Every operation of the Rust executable is declared once, with typed input and output. The following are derived from that declaration:

- HTTP routes under `/api/v1`;
- the OpenAPI 3.1 document;
- Swagger UI;
- MCP tools and resources;
- the Cockpit's runner;
- the generated reference.

The command line is written by hand and cross-checked against the registry in both directions. A capability that writes the repository must declare that effect. The MCP instructions and OpenAPI name every such capability, and a test holds both to the registry. Indexed repository objects, such as rules, ADRs and claims, become MCP resources at `majordomus://<kind>/<identity>` without registration.

**Try it.**

```console
$ majordomus-cli capabilities list
$ majordomus-cli capabilities describe rules.show
$ majordomus-cli capabilities validate
$ curl -s http://127.0.0.1:8742/openapi.json | jq '.paths | keys'
```

**Proved by.** `test/cases/76_capabilities_projections.sh`, `test/cases/46_cross_surface.sh`,
`test/cases/92_openapi_reference.sh`, `test/cases/72_rust_mcp.sh`,
`apps/majordomus-cli/tests/projections.rs`, `apps/majordomus-cli/tests/shared_units.rs`.

**Maturity.** On master.

### Discovery, generation and drift

**How it works.**

- **Discovery.** `share/kinds.yaml` defines document kinds and `.ai/repo/knowledge/sources.yaml` maps git pathspecs to them, so a new rule file is indexed with no list to edit.
- **Generation.** `majordomus generate` writes the executable's projections, and `scripts/derive` runs the whole derivation graph.
- **Drift.** `derive-check`, `generate --check` and `generate-site-data --check` regenerate and compare byte for byte. The pre-commit hook refuses stale derived files.

**Try it.** `just derive`, `just derive-check`, `majordomus-cli generate --check`.

**Proved by.** `test/cases/64_knowledge_discovery.sh`,
`test/cases/51_derived_artifacts_committed.sh`, `test/cases/56_derived_current_gate.sh`,
`test/cases/104_strict_derivation.sh`, `apps/majordomus-cli/tests/generate_check.rs`,
`apps/majordomus-cli/tests/index_behavior.rs`.

**Maturity.** On master, and enforced at commit time and in CI.

### Repository environment and the Cockpit

**How it works.**

- **Repository environment.** A typed snapshot of a checkout: project, version control, toolchains, layer, workflows, providers, local services and diagnostics, with provenance for every value. The shell banner (through direnv), `majordomus-cli env`, `GET /api/v1/environment` and the MCP resource are renderers of it.
- **The Cockpit.** A server-rendered local web UI at `/cockpit`. It offers a runner for every capability, a browser for every indexed kind, and views for release, design, mesh, worktrees, continuity, executions and graphs. Every page asks the same executor as the API.

**Try it.** `majordomus-cli env`, `majordomus-cli env --json`, then open the Cockpit URL that
`majordomus-cli serve status` prints.

**Proved by.** `test/cases/100_environment.sh`, `apps/majordomus-cli/tests/environment.rs`,
`apps/majordomus-cli/tests/cockpit.rs`.

**Maturity.** On master. Plan and board views in the Cockpit are branch only.

### Versions and publication

**How it works.**

- **Versions.** The version is written by one command, `majordomus-cli release bump`. `release analyze` measures the minimum bump by comparing the public capability surface with the last release.
- **Releases.** A tag builds, publishes, writes a release record and smoke-tests the public installer.
- **Site deploys.** A merge to master publishes the site from committed projections. The deploy is verified when `majordomus.dev/build.json` serves the merge commit, and the `pages-live` gate checks that no publication is overdue.

**Try it.** `majordomus-cli release analyze`, `curl -s https://majordomus.dev/build.json`.

**Proved by.** `test/cases/112_version_matches_surface.sh`,
`test/cases/87_release_pipeline.sh`, `test/cases/180_publication_is_observed.sh`,
`test/cases/275_publication_is_current.sh`.

**Maturity.** On master.

### The command line itself

**How it works.**

- **Two programs.** The shell tool `majordomus` owns the task lifecycle. The Rust executable `majordomus-cli` owns the registry and its surfaces; the shell tool names it when a command belongs there.
- **Help.** Every command answers `--help` with its usage and nothing else, before and after `init`.
- **Coverage.** Every public command has behavioural and negative coverage.
- **Completion.** Answered from one command graph composed from both programs and the `just` recipes.

**Try it.** `majordomus --help`, `majordomus-cli --help`, `majordomus-cli completion zsh`.

**Proved by.** `test/cases/15_command_surface.sh`, `test/cases/31_command_coverage.sh`,
`test/cases/101_command_graph.sh`, `test/cases/102_completion_shell.sh`,
`test/cases/98_cli_reference.sh`, `apps/majordomus-cli/tests/cli_examples.rs`.

**Maturity.** On master.

### End to end

`test/cases/19_end_to_end.sh` walks one repository through `init`, a scoped task, checks,
a handover and a finish, as a new user would.

## Proving it yourself

Every shell case runs against a fresh temporary git repository and the real `bin/majordomus`.

```console
$ bash test/run.sh 04_start_check 25_continuity_lifecycle 42_dag_waves 125_rule_proof
$ MJ_TEST_JOBS=4 bash test/run.sh                      # the whole suite
$ cd apps/majordomus-cli && cargo test --test projections --test rules --test peer_claims
```

To keep a run as evidence tied to the commit it proves:

```console
$ MJ_TEST_REPORT=report.tsv bash test/run.sh 04_start_check 06_finish
$ majordomus-cli evidence record --suite report.tsv
$ majordomus-cli evidence show
```

## What it does not do yet

- CI runs every gate but does not record its runs into the evidence ledger, so most rules and
  claims read `not run` until someone records a run.
- The mesh has no Tailscale or mDNS provider.
- Overlap is reported, never enforced. The only refusal is a task's own scope.
- Automatic session capture exists for Claude Code only.
- Intent as a typed record, plan and board views in the Cockpit, and a richer entry preflight are
  open pull requests.
- The command line and the Cockpit's area list are written by hand, checked against the
  registry rather than generated from it. [`HOW_IT_WORKS.md`](@/docs/how-it-works.md) lists every such
  place.
{% endraw %}
