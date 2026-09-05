<p align="center">
  <a href="https://korczis.github.io/prismatic-majordomus"><img src="assets/logo.svg" alt="Prismatic Majordomus" width="420"></a>
</p>

# Prismatic Majordomus

**A lightweight supervisory control layer for AI-assisted work.**

[Website](https://korczis.github.io/prismatic-majordomus) ·
[CLI reference](docs/CLI.md) ·
[File schemas](docs/SCHEMAS.md) ·
[What is guaranteed, advisory, planned, or refused](docs/SITE_CLAIMS.md) ·
[Why it exists](docs/EXTRACTION_REPORT.md)

Majordomus holds one canonical policy for how AI workers operate in a repository,
generates the instruction file each tool reads from it, keeps task state outside every
conversation, reports when two workers are about to collide, and refuses to call work
finished until a contract is met. It runs entirely locally, in portable shell, and
never invokes a model.

```
$ majordomus finish --outcome completed --verify-command "make test"
OK   scope         t-20260903193012-a4f1 — 12 touched file(s), all within scope
OK   verification  t-20260903193012-a4f1 — make test — exit 0, 41s
OK   state         t-20260903193012-a4f1 — advanced (head 9b1e2d4)
FAIL blockers      t-20260903193012-a4f1 — unresolved entry in open-questions.md  [reproduce: grep -n 'unresolved' .ai/local/state/open-questions.md]
OK   note          t-20260903193012-a4f1 — 20260903T201455Z--main--9b1e2d4--c0ffee1234567890.md
finish: refused, 1 unmet
```

## The problem

Most AI-assisted work fails for boring reasons. Too much context. The wrong rules for
the tool that happens to be open. Reasoning at maximum by default. Sessions that never
end. Two agents editing the same file. No definition of done, so "done" means the
worker said so.

These are operational-control problems, and the environments this tool was distilled
from had them at scale:

- instruction files for two AI tools in the same repository sharing **0–9 %** of their
  content: not duplicates, disjoint rule sets
- an always-loaded operating contract oscillating between **0 and about 1,100 lines**
  across roughly 200 hand edits, until a hard budget with a failing check pinned it
- **seventeen** independent cases of a check documented as blocking, present on disk,
  executable, and invoked by nothing
- full worktree isolation still producing about **3,200** concurrently modified files
  and **67** duplicated patches across forty worktrees, with nine ever merged
- **10 GB** of session notes standing in for a database, recovered by a hand-written
  runbook

Better prompts do not fix any of this.

## What it does

| | |
|---|---|
| **Layer** | one portable `.ai/` directory, readable without the tool: `.ai/repo/` is tracked and canonical, `.ai/local/` is this checkout's own and ignored; `doctor` proves the manifest resolves, every section it names exists, and nothing under `local/` is tracked |
| **Policy** | one provider-neutral `policy.yaml`; unknown keys are errors |
| **Projection** | `update` generates `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, or any target you name, deterministically, and stamps each with the policy hash and the hash of its own content; a hand edit is detected and never silently overwritten |
| **State** | one active task per checkout in `current.yaml`, with `branch`, `head`, and `working_tree` computed from git, never authored |
| **Scope** | `start` takes the paths a task may touch; `check` and `finish` fail on files outside them; other worktrees' overlapping claims are reported |
| **Profiles** | one bundle per task class, each setting capability class, reasoning effort, verbosity, presentation, context toggles, verification, and checkpoint interval independently |
| **Handover** | append-only records with computed front matter and required sections, created atomically, never staged; `--resolve` finds the right one for this worktree and branch and labels how far git has moved since |
| **Finish** | a typed outcome and a contract evaluated line by line; nothing written when any line fails |
| **Doctor** | proves the installation is real: policy parses, every declared enforcement is actually invoked by the hook it names without a swallowed exit code, every projection matches its own stamp, the always-loaded file is under budget |
| **Watch** | reports drift between policy, projections, state, scope, and git, each finding with the command that reproduces it |
| **Plan** | milestones are outcome specifications and issues are execution contracts; status, execution waves and the next ready issue are derived from the dependency graph, never stored |

Every row above is backed by a case in [`test/cases/`](test/cases/). The README is not
allowed to say more than the tests do. The layer is the repository's, not the tool's:
Majordomus is the reference implementation that validates, projects and enforces it.

**Scoped context.** Context is a tree, not one file. Every `README.md` under `.ai/` is a
context document: front matter gives it an identity that survives a move, a scope (its
directory, its subtree, or listed paths), the providers it addresses, and how it composes
with its ancestors. `majordomus context resolve <path>` prints the documents that apply to
a path, least specific first; the nearest one adds to the chain and never silently
replaces it, and sibling directories never see each other's. `context validate` refuses a
tree with a missing contract, a duplicate id, a broken or cyclic override, or an override
of a document marked `final`; `context affected` names the documents a change set touches,
including the ones that track the code that changed; `context check-sync` is the one
command a hook runs. The model is in [`docs/CONTEXT.md`](docs/CONTEXT.md).

## Quick start

```bash
git clone https://github.com/korczis/prismatic-majordomus ~/majordomus
export PATH="$HOME/majordomus/bin:$PATH"

cd your-project
majordomus init            # .ai/: policy, four profiles, prompts, the vendored rule baseline, workflows
majordomus update          # CLAUDE.md, AGENTS.md, GEMINI.md generated from the policy
majordomus doctor          # names the two hook lines you still need to add
```

Add the hook lines, run `doctor` again, commit `.ai/repo/` and the generated files.
`.ai/local/` is this checkout's own state and is ignored; it never travels through git.
Then, per task:

```bash
majordomus start "fix OAuth callback" --scope lib/auth --profile debugging
# ... the AI worker reads the generated instructions and works ...
majordomus check
majordomus finish --outcome completed --verify-command "make test"
```

Requirements: bash 3.2 or newer, git, and `sha256sum` or `shasum`. Nothing else.
Nothing is installed into your project except `.ai/` and the files the policy names. A
repository set up before the `.ai/` layer, with its data under `.majordomus/`, is moved
by `majordomus migrate` (preview with `--dry-run`); every other command refuses that
layout and names it.

## Mental model

```
            Human / Organisation
                    |
                    v
               MAJORDOMUS
        policy · state · verification
                    |
      +-------------+-------------+
      v             v             v
   Worker A      Worker B      Worker C
  (Claude Code)  (Codex)       (Gemini, Cursor, local ...)
      |             |             |
      +-------------+-------------+
                    |
                    v
          Verified, accepted outcomes
```

Two principles run through everything. **Git is the authority; everything else is an
aid.** Identity fields on any record are computed from git, and a body that tries to
set them is rejected. **Numbers in prose are computed or forbidden.** `doctor` fails on
a hardcoded count in the always-loaded file.

## Philosophy

Simple things should be trivial. Complex things should be possible. Weird things should
be hackable. Majordomus is opinionated by default, programmable by design, and hackable
at the edges: complexity is optional, not ambient.

- Defaults work out of the box: `init`, `update`, `doctor`, `start`, `check`, `finish`,
  and nothing about schemas.
- The repository's policy, profiles, rules, prompts and workflows are inspectable and
  customisable, declaratively, under `.ai/repo/`.
- The internals stay open: validators, provider templates, projections and the rule
  package are there to read and to extend.
- Majordomus is not a universal agent framework; its extensibility lives inside its domain.

The rule is `.ai/repo/rules/project/optional-complexity.v1.md`.

## Core principles

- Sessions are workers, not memory.
- Load minimum sufficient context.
- Externalise decisions and durable state.
- One worker, one clear scope.
- Escalate capability and reasoning only when justified, and record it.
- Execution depth is not output verbosity.
- Define done before executing.
- Verify outcomes, not activity.
- Parallelism requires isolation.
- Handovers transfer state, not transcripts.

Each is a rule object in the vendored baseline under `.ai/repo/rules/vendor/majordomus/`,
one file each, and every enforced rule names the principle it rests on; every worker
reads the same ten sentences from the same place, whichever tool it is.

## Doctrine

A principle is a sentence a worker reads. A **doctrine** is the part of a principle a
machine can decide, and it names the code that decides it. Every rule Majordomus
enforces is a portable rule object — one Markdown file with YAML front matter — in the
package the tool ships under [`share/standard/majordomus/`](share/standard/majordomus/)
and vendors into every repository under `.ai/repo/rules/vendor/majordomus/`; the ten
principles are rule objects too, and every enforced rule names the principle it rests on
in `depends_on`. No command selects checks by hand: `check`, `finish`, `doctor` and
`watch` each ask the resolved rule set what applies to them, so a rule added to the
package is enforced from the moment it is vendored, and a repository's own rules under
`.ai/repo/rules/project/` join the same set.

A doctrine is either **blocking** (a violation stops the command with a non-zero exit)
or **advisory** (reported, and the command still succeeds). There is no third class, no
severity ladder, no baseline, and no override. The class is read at dispatch time and is
what routes the finding, so changing one word in a rule file changes whether
`majordomus check` exits 0 — and a test asserts exactly that.

The point of declaring rules is being able to check the declaration. `majordomus doctor`
walks the chain for each one, reading the source rather than the rule's description
of itself:

```
declared → validator exists → the commands it names dispatch → a blocking rule can
exit non-zero → a test proves it → CI runs that test without swallowing it
```

and in the other direction, a validator that no doctrine declares is enforcement running
under no rule, and fails. That check is itself mutation-tested: `18_doctrine_wiring.sh`
breaks each link in a throwaway copy and fails unless `doctor` goes red, because a
verifier that survives broken wiring proves nothing.

```bash
majordomus rules list          # the effective set, resolved: vendored baseline plus project rules
majordomus doctrine status     # declared, blocking, advisory, missing validators — all derived
majordomus doctrine list       # id, class, validator, the commands that enforce it
majordomus doctrine show <id>  # one rule, with its claims, its tests and the file it lives in
majordomus check --rule <id>   # run one rule
```

Full model, and what was deliberately left out: [`docs/DOCTRINE.md`](docs/DOCTRINE.md).

## Execution profiles

| profile | capability | effort | verbosity | context | verification | checkpoint |
|---|---|---|---|---|---|---|
| `routine` | fast | low | terse | task, state | verify command if files changed | 30m |
| `implementation` | standard | medium | concise | + decisions, files | verify command | 15m |
| `debugging` | strong | high → xhigh | concise | + failing output, history | verify command, regression test | 15m |
| `deep-work` | strongest | high → xhigh | detailed | + architecture | verify command, decision record | 30m |

Capability classes are not vendor model names; the projection tells the worker to pick
the closest its environment offers. Each axis is a separate field. Add a profile by
adding a file. Full schema: [`docs/SCHEMAS.md`](docs/SCHEMAS.md).

## Example workflow

```
$ majordomus start "fix OAuth callback" --scope lib/auth --profile debugging
started t-20260903193012-a4f1  profile=debugging  scope=lib/auth
INFO overlap     wt-bob — claims lib/auth/oauth — contained by your lib/auth  [reproduce: majordomus check --overlap]
next: worker reads the projected instructions; checkpoint every 15m; majordomus check

$ majordomus check
OK   state       t-20260903193012-a4f1 — advanced (head 3f2a9c1)
FAIL scope       config/secrets.example — outside claimed scope (lib/auth)  [reproduce: git status --porcelain; git diff --name-only 3f2a9c1 HEAD]
OK   checkpoint  t-20260903193012-a4f1 — 7m ago, interval 15m
OK   blockers    t-20260903193012-a4f1 — none open
check: 4 finding(s), 1 failing

$ printf '# Objective\n…\n# Current State\n…\n# Next Action\n…\n' | majordomus handover
.ai/local/state/handovers/20260903T201455Z--main--9b1e2d4--c0ffee1234567890.md

$ majordomus handover --resolve      # next session, same branch
Handover: .ai/local/state/handovers/20260903T201455Z--main--9b1e2d4--c0ffee1234567890.md
Match: same_worktree_same_branch
Git state: advanced
---
# Objective
…
```

A walk-through with real output is in [`examples/minimal/`](examples/minimal/).

## Provider adapters

```
.ai/repo/policy.yaml + profiles/          .ai/repo/rules/  (the rules themselves)
         |                                        ^
         |  majordomus update                     |  read by the worker, never copied
         v                                        |
CLAUDE.md   AGENTS.md   GEMINI.md   <any target named in the policy>  --+
```

Adapters are thin bootstraps. Each generated file tells the worker where the
repository's context lives — `README.md`, then `.ai/README.md` and its discovery
protocol — and carries no rule of its own, so a rule that exists for one provider and
not another cannot happen. `doctor` fails a generated file that carries a rule corpus of its own — a profile
table, rule bullets, a rules or lifecycle or finish-contract heading — and a
`README.md` that does not name `AGENTS.md`. The templates ship with the tool under
`share/providers/`; a repository that needs a different adapter puts its own at
`.ai/repo/providers/<provider>.tmpl`, and the rules stay where they are.

## Commands

| command | answers | writes | exit |
|---|---|---|---|
| `init` | create the `.ai/` layer here | yes; refuses to overwrite, `--extend` adds what is missing | 0 / 15 |
| `migrate` | move pre-`.ai` project data from `.majordomus/` into `.ai/` | git moves, a verified backup, the index | 0 / 12 / 15 |
| `rules` | the effective rule set: vendored baseline plus project rules, resolved | only `vendor update` | 0 / 10 / 11 / 12 / 15 |
| `doctor` | is Majordomus itself real and wired here? | no | 0 / 10 / 12 |
| `start <task>` | begin a scoped task under a profile | task record, ledger | 0 / 15 |
| `check` | is the task consistent with policy, scope, state? | no (`--checkpoint` updates one timestamp) | 0 / 10 |
| `watch` | what has drifted? | no | 0 / 11 |
| `doctrine` | what rules are enforced, by what, and are they wired? | no | 0 / 10 / 12 |
| `update` | regenerate projections from policy | projections | 0 / 10 / 15 |
| `handover` | write a continuation record; `--resolve` finds one | one new file | 0 / 10 / 12 |
| `finish` | evaluate the finish contract | task record, ledger | 0 / 10 / 15 |
| `bench` | how long does every public command take here, cold and warm, against the baseline? | local evidence under `.ai/local/benchmarks/`; the baseline only with `--write-baseline` | 0 / 10 / 12 / 15 |

Exit codes are a contract: `0` ok, `2` usage, `10` contract unmet, `11` drift found,
`12` missing artifact, `13` internal error, `15` refused. There is no "warn and
continue". Details: [`docs/CLI.md`](docs/CLI.md).

Every command reports where its time went with `MJ_TIMING=1`, and `bench` holds the
accepted state in a tracked baseline; how that works, and the rules behind it, is
[`docs/PERFORMANCE.md`](docs/PERFORMANCE.md).

## Customisation

- **Rules workers read:** rule objects under `.ai/repo/rules/project/`, one Markdown
  file each; the vendored baseline changes only through `rules vendor update`.
- **Which files are generated:** the `projections` list in the policy.
- **What "done" means:** `verification.finish_requires` in the policy, plus per-profile
  `verification.*` fields.
- **Budget for the always-loaded file:** `context.always_loaded_budget_lines`.
- **What must be wired:** the `enforcement` list; `doctor` reconciles it.
- **A new task class:** a new file in `.ai/repo/profiles/`.

Unknown keys anywhere are errors, so a typo fails loudly.

## What this is not

- not a model, and it never invokes one
- not an agent framework, orchestrator, or runtime
- not a prompt library or a memory system
- not a daemon, database, queue, or hosted service; the Rust executable's `mcp` and
  `serve` are read-only processes a client or a person starts and owns, one shared server
  per repository on the loopback interface that ends when its last client leaves
  ([`docs/MCP.md`](docs/MCP.md))
- not a slice of any other platform; there is no shared code

## Limitations

- Majordomus is invoked by a person, a git hook, or a worker following its
  instructions. It does not hook the worker's runtime.
- It measures no tokens and no cost. [`docs/ECONOMICS.md`](docs/ECONOMICS.md) says what
  it would take.
- Scope overlap is reported, never blocked, and only across worktrees of one repository
  on one machine.
- The regression-test check in `finish` is a path heuristic (`test/`, `spec/`, `_test.`)
  and says so in its message.
- The YAML subset is deliberately small: maps, lists, lists of maps, inline lists,
  quotes, comments. Anchors, multi-line scalars, and flow maps are rejected.

## Roadmap

The roadmap is not written here. Milestones are canonical records under
`.ai/repo/project/milestones/`, ordered by the dependency graph between them, and every
view of them is derived from that graph:

```
majordomus plan roadmap        # the sequence, with what is current and what is next
majordomus plan rgraph         # the same graph, as a diagram
```

Rendered at [the roadmap](https://korczis.github.io/prismatic-majordomus/roadmap/), with the
current state as a document in [`docs/PLAN_STATUS.md`](docs/PLAN_STATUS.md).

Each step is gated by the previous one being real, and that is an invariant rather than a
promise: a milestone whose dependencies are not accepted is blocked, and finishing every
issue inside it does not release it. [`docs/ROADMAP.md`](docs/ROADMAP.md) explains how the
ordering, the gate and the claim linkage are derived.

## Interfaces

The Rust executable under [`apps/majordomus-cli/`](apps/majordomus-cli/) exposes the same
`.ai/` layer to programs, read-only, through several interfaces that are all derived from
one capability registry: a capability is defined once, in a typed descriptor or in a
declarative file with its JSON Schema, and MCP, HTTP, OpenAPI, Swagger UI, the command line
and the generated reference are projections of it, so nothing is maintained twice.

Every generated file in the repository comes out of the same executable: `majordomus
generate` writes `docs/generated/`, `share/allow/`, the provider bootstraps `AGENTS.md`
and `CLAUDE.md` (from the policy and the templates), and the site's registry dataset
`site/data/registry/registry.json`; `majordomus generate --check` says which of them is
stale, and CI refuses a merge or a deploy from a stale one. The website's own generator
consumes two of those files and nothing else of the crate, so `just derive` regenerates
every derived file of the repository in dependency order and `just derive-check` names
every stale one, whichever generator owns it. The owners of every truth and the direction
of generation are in
[ADR 5](.ai/repo/adrs/0005-one-projection-plan-canonical-owners-and-the-site-as-registry-view.md)
and the graph is drawn in [`docs/GITHUB_PAGES_ARCHITECTURE.md`](docs/GITHUB_PAGES_ARCHITECTURE.md);
the site's [Executable section](https://korczis.github.io/prismatic-majordomus/registry/) —
the registry, every module and capability, the command line, the MCP surface, the HTTP API
and the benchmarks — is rendered from that dataset and from nothing typed by hand.

```bash
just build                      # cargo build of apps/majordomus-cli (or: cargo build --manifest-path apps/majordomus-cli/Cargo.toml)
just mcp                        # MCP on stdio for the client that spawned it; the first one in a repository is the shared server
just serve                      # the shared server alone: http://127.0.0.1:8741, Swagger UI at /docs, /openapi.json, /mcp
just capabilities               # every capability and its projections
just derive                     # every derived file of the repository, in dependency order
just derive-check               # every committed derived file is current; exit 10 naming the stale ones
just bench-coverage             # every operation is a benchmark target; the denominator is the registry's
just bench-run                  # time every operation: directly, over MCP (a real child), over HTTP (a real socket)
```

**Declare once, derive everything.** A capability is one `capability!` block in its
module's file; modules compose capabilities with `module!`, the root composes modules
with `compose_modules!`, and MCP, HTTP, OpenAPI, Swagger UI, the command line, the
benchmark targets, the cache behaviour and the generated reference are derived from the
registry those blocks build. Adding a capability is: define the typed input and output
(with the input's benchmark cases), write the handler, add the block to its module, run
`just derive` and `just validate`. There is no step that edits an MCP registry, an
HTTP router, an OpenAPI document, Swagger, a benchmark inventory, a documentation table or
a page of the website, because none of those is written by hand: the capability's page,
its rows on the module, MCP, API and benchmark pages and its links come out of the same run. Every externally callable operation is
benchmarked through the real transports and every claim about speed is a recorded
measurement ([`docs/CAPABILITIES.md`](docs/CAPABILITIES.md), ADR 4).

**One server per repository.** The first `majordomus mcp` binds the loopback HTTP
projection beside its stdio session and logs the URL (Swagger UI at `/docs`, the OpenAPI
document, MCP over HTTP at `/mcp`); every later `majordomus mcp` in the same repository
attaches to it instead of starting another, and the server ends when its last client
leaves. It writes one file, a lease under `.ai/local/state/mcp/`, and nothing under the
tracked tree.

**Clients start it themselves.** [`.mcp.json`](.mcp.json) (Claude Code),
[`.gemini/settings.json`](.gemini/settings.json) (Gemini CLI) and
[`.codex/config.toml`](.codex/config.toml) (Codex) name [`bin/majordomus-mcp`](bin/majordomus-mcp),
which builds the executable when it must and runs `majordomus mcp`. Open the repository in
any of them and the server is there; open it in two and they share one.

**Peers see each other.** Every attached client is a peer, named by what it said in
`initialize`; `majordomus_peers` lists them and `majordomus_announce` tells the others what
a client is working on and which paths it expects to touch, so Claude, Codex and Gemini in
one checkout can avoid colliding, out of the box.

**Every use case is executed, not described.** What a person does with the tool is one
file under `.ai/repo/use-cases/`, naming the commands, rules and claims it relies on and
carrying a scenario the tool runs against itself; the page it becomes shows that
execution, its maturity is observed from the evidence, and a public command no use case
runs is a gap `doctor` reports and `finish` refuses. `majordomus usecase` lists, runs,
tallies, traces and scaffolds them: [`docs/USE_CASES.md`](docs/USE_CASES.md).

**It reads what the repository declares it may.** What a worker reads of the repository is
declared once in `.ai/repo/scope.yaml` (in: the sources, the tests, the layer; out, which
wins: version control, the local half, dependencies, build outputs, secrets, generated
assets, archives, images, video, PDF, database dumps, fixtures over a limit, binary
content), and the executable discovers, indexes and serves nothing outside it. Any path can
be asked about: `majordomus scope <path>` answers in or out and names the rule. The
declaration, the judgement and its gates: [`docs/SCOPE.md`](docs/SCOPE.md).

What is canonical, how a repository adds a kind with its schema without a code change, and
how the pieces fail: [`docs/CAPABILITIES.md`](docs/CAPABILITIES.md); the MCP surface, the
shared server's lifecycle and the client configurations:
[`docs/MCP.md`](docs/MCP.md). The kinds and their schemas are read at run time from
[`share/kinds.yaml`](share/kinds.yaml) and [`share/schemas/`](share/schemas/), and the shell
tool's allow-lists under `share/allow/` are generated from those schemas.

## Contributing

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) and the generated [`AGENTS.md`](AGENTS.md).
The best contribution is often a deletion. Run `bash test/run.sh` (`just test` runs it with
the Rust gate and the site data check); every behaviour has a success and a failure case.
The [`justfile`](justfile) lists what a person runs here, routed to the Rust executable
wherever it can do the job and to the shell tool for the lifecycle; `just` alone lists the
recipes. This repository supervises itself: `bin/majordomus doctor` runs in its own
pre-commit hook and in CI.

The website is a projection of this repository rather than a second copy of it: every
page is generated from `README.md`, `docs/`, the policy skeleton and `docs/CLAIMS.yaml`,
and CI refuses to deploy a tree whose derived files are stale. Editing a generated file
is never the fix. See
[`docs/GITHUB_PAGES_ARCHITECTURE.md`](docs/GITHUB_PAGES_ARCHITECTURE.md).

## Origin and licence

Majordomus distils operational patterns learned while building **Prismatic**, a much
broader cognitive and epistemic platform. The relationship is one-way; no code or
vocabulary flows back. The evidence is in
[`docs/EXTRACTION_REPORT.md`](docs/EXTRACTION_REPORT.md). MIT licence.
