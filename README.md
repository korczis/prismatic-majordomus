# Prismatic Majordomus

**A lightweight supervisory control layer for AI-assisted work.**

> **Status: design phase.** This repository contains a specification and the evidence
> behind it. No command runs yet. Every capability below is a target for v0.1, and the
> README will describe a capability as real only once a behavioural test backs it.

---

## The problem

AI coding workers are increasingly capable. They are still routinely operated with:

- **stale, disjoint instructions** — in one workspace of about twenty repositories, the
  instruction file for one AI tool and the file for another tool in the same repository
  shared between 0 % and 9 % of their content. Which rules applied depended on which
  worker was hired.
- **unbounded always-loaded context** — one repository's operating contract oscillated
  between 0 and roughly 1,100 lines across about 200 hand edits before a hard budget
  with a failing check pinned it under 50.
- **enforcement that is declared but not wired** — at least seventeen independent cases
  were found of a check documented as blocking, present on disk, executable, and invoked
  by nothing. In one, 34 of 39 pre-commit scripts were orphaned while the policy files
  naming them read "BLOCKING".
- **isolation without coordination** — with one git worktree per worker fully in force,
  forty worktrees still produced about 3,200 concurrently modified files and 67 clusters
  of duplicated patches. Nine of eighty ever merged.
- **transcripts as the database** — session notes grew to 10 GB. Recovering working
  state after a session ended took a hand-written fifteen-to-thirty-minute runbook.
- **no measurement of AI spend on AI-assisted development** — the same team that built
  a cost profiler for its product's model calls had zero telemetry on its own coding
  workers.
- **supervisory tools abandoned within days** — four separate attempts were found in one
  workspace. Each died because nothing detected when it stopped working.

These are operational-control problems. Better prompts do not fix them.

## What Majordomus does

Majordomus sits between the person and the workers. It:

| Responsibility | In one line |
|---|---|
| **Policy** | one canonical, provider-neutral operating policy; detects when it and its projections diverge |
| **Context** | minimum sufficient context as named toggles; a hard budget on always-loaded material |
| **State** | durable task state in typed files outside every conversation, with git-derived identity |
| **Watch** | deterministic drift inspection; every finding carries the command that reproduces it |
| **Coordination** | who is working on what; scope claims normalised; overlap computed on touched files |
| **Budget** | reasoning effort, output verbosity, context strategy, and model capability as independent axes under named profiles |
| **Verification** | "done" is a contract evaluated by `finish`, not a word a worker types |
| **Handover** | durable state transferred append-only and atomically, never transcripts |
| **Maintenance** | provider instruction files regenerated from policy and fingerprinted |
| **Termination** | a worker stops when the contract is met, when blocked on a human, or when scope is exceeded |

The primary deterministic guarantee, applied to Majordomus itself first: **every
enforcement the policy declares actually resolves to something that runs.** That check
is small. The environments studied described it in five places and implemented it in
none.

## Quick start (target for v0.1)

```bash
# add Majordomus to a repository
majordomus init

# prove the installation is real: policy parses, enforcement is wired, budgets hold
majordomus doctor

# begin a scoped task under a profile
majordomus start "fix OAuth callback" --scope lib/auth/ --profile debugging

# ... an AI worker does the work, reading the projected instructions ...

# is state, scope, and policy consistent right now?
majordomus check

# hand the work to the next session, or declare it finished
majordomus handover < notes.md
majordomus finish
```

Ten minutes from clone to a finished scoped task is the success criterion for v0.1.

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

Two principles run through everything:

1. **Git is the authority; everything else is an aid.** Identity fields on any record
   are computed from git, never authored by a worker.
2. **Numbers in prose are computed or forbidden.** Majordomus writes the command that
   produces a count, never the count.

## Lifecycle

```
majordomus start <task>
      |
      v
  scope declared (paths)  --> claim normalised, overlap reported
      |
      v
  profile selected        --> effort / verbosity / context / verification fixed
      |
      v
  worker executes         (Majordomus is not involved)
      |
      v
majordomus check          --> state consistent? scope respected? blockers?
      |
      +-- incomplete --> continue, or majordomus handover
      |
      v
majordomus finish         --> finish contract evaluated; refuses if unmet
```

A session is bounded by `start` and either `finish` or `handover`. There is no "still
open since Tuesday" state that Majordomus recognises as healthy.

## Profiles

A profile bundles five independent axes. They are never collapsed into one "quality"
setting.

| Profile | Capability class | Effort | Verbosity | Context toggles | Verification |
|---|---|---|---|---|---|
| `routine` | fast | low | terse | task, current state | verify command if files changed |
| `implementation` | standard | medium | concise | + decisions, relevant files | tests required |
| `debugging` | strong | high | concise | + failing output, recent history | regression test required |
| `deep-work` | strongest | high, may escalate | detailed | + architecture notes | tests + written decision record |

Capability classes are not vendor model names. Projections map a class to whatever the
provider offers. Effort is recorded only where it differs from the default.

Full definitions: [`docs/SCHEMAS.md`](docs/SCHEMAS.md).

## Durable state

```
.majordomus/
  policy.yaml            the one canonical policy
  profiles/              routine · implementation · debugging · deep-work
  state/
    current.yaml         the one active task, or absent
    decisions.md         append-only, dated
    open-questions.md    things blocked on a human
    handovers/           append-only, one file per handover
    ledger.jsonl         append-only events, retention-capped
  generated/
    fingerprints.yaml    hash of every projection at generation time
```

Every record carries the git state it was written at. On read, Majordomus labels it
`exact`, `advanced`, `diverged`, or `different_context`. Stale state is detected and
named, never silently trusted. Transcripts are never stored.

## Commands

| Command | Answers | Writes |
|---|---|---|
| `init` | set up `.majordomus/` here | yes, refuses to overwrite |
| `doctor` | is Majordomus itself healthy and actually wired? | no |
| `start <task>` | begin a scoped task under a profile | current task, claim |
| `check` | is the task consistent with policy, scope, and state? | no |
| `watch` | what has drifted: state, policy, projections, retention? | no |
| `update` | regenerate provider projections from policy | projections, fingerprints |
| `handover` | write an append-only continuation record | one new file |
| `finish` | evaluate the finish contract; refuse if unmet | current task, ledger |

`doctor`, `check`, and `watch` are read-only. Nothing performs recursive deletion, silent
overwrite, network access, or evaluation of generated text. Exit codes are a contract;
see [`docs/CLI.md`](docs/CLI.md).

## Provider support

```
.majordomus/policy.yaml
         |
         |  majordomus update
         v
CLAUDE.md   AGENTS.md   GEMINI.md   generic Markdown
```

Projections are generated, fingerprinted, and headed with the command that regenerates
them. Adapters translate; they never add rules. A rule that exists for one provider and
not another is a policy bug.

v0.1 projects a deliberately narrow subset: the always-loaded budget, the profile table,
state file locations, the finish contract, and the rule that identity fields are never
authored. What is not projected is listed, not implied.

## Examples

`examples/minimal/` will contain a scratch repository walked from `init` to `finish`,
with the exact output of every command. Until it exists, [`docs/CLI.md`](docs/CLI.md)
shows the target output.

## Philosophy

- **Declared is not enforced.** A check that is documented and not wired is worse than
  no check: it produces a compliance report and then decays.
- **Guaranteed or observed, nothing between.** A check either blocks deterministically or
  reports with a reproduce command. Every "advisory tier" studied decayed into
  decoration.
- **Cheap to keep, loud when broken.** The four abandoned tools died silently. Majordomus
  checks itself before it checks anything else.
- **No new nouns.** No agents, personas, roles, tiers, or registries. A supervisory tool
  that adds nouns becomes the thing it supervises.
- **A budget without a failing check is a wish.**

## What this is not

- not a model, and it never invokes one
- not an agent framework, orchestrator, or runtime
- not a prompt library
- not a transcript summariser or memory system
- not a server, daemon, database, queue, MCP surface, or hosted service
- not a slice of any other platform; there is no shared code

## Limitations (v0.1)

- Majordomus is invoked by a person, a git hook, or a worker following its projected
  instructions. It does not hook the worker's runtime.
- It does not measure tokens or cost. Any `estimated_` field is labelled and excluded
  from enforcement.
- It does not route work to models.
- It does not coordinate across machines.
- It does not represent dependencies between tasks.
- Portable shell means bash 3.2 and BSD userland are the floor. YAML handling uses a
  small dependency with a documented fallback.

## Roadmap

| Version | Adds |
|---|---|
| **0.1** | policy, profiles, state, the eight commands, doctor with wiring reconciliation, projections for four providers, tests |
| 0.2 | opt-in runtime adapters: read-size clamp, output condensation, subagent budget — limits derived from the profile, never global constants |
| 0.3 | execution telemetry, once a provider exposes it honestly |
| 0.4 | cost per accepted outcome, only on measured data |
| 0.5 | routing recommendations derived from 0.4 |
| 1.0 | shared policy across repositories and workers |

Each step is gated by the previous one being real.

## Origin

Majordomus distils general operational patterns learned while building **Prismatic**,
a much broader cognitive and epistemic platform. The relationship is one-way: patterns
flowed out; no code, dependency, domain logic, or vocabulary flows back. The evidence
and the derivation are in [`docs/EXTRACTION_REPORT.md`](docs/EXTRACTION_REPORT.md).

## Documents

| | |
|---|---|
| [`docs/DESIGN.md`](docs/DESIGN.md) | the v0.1 specification |
| [`docs/CLI.md`](docs/CLI.md) | every command: behaviour, exit codes, output |
| [`docs/SCHEMAS.md`](docs/SCHEMAS.md) | every file: schema and a concrete example |
| [`docs/EXTRACTION_REPORT.md`](docs/EXTRACTION_REPORT.md) | the evidence, the pattern ledger, what was rejected |
| [`AGENTS.md`](AGENTS.md) | the contract for changing this repository |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) · [`SECURITY.md`](SECURITY.md) | |

No licence file is present yet; until one is added the repository is all rights
reserved by default. A licence is a blocker for the first release.
