# Extraction Report — Prismatic Majordomus v0.1 design phase

This is the record of how [`DESIGN.md`](DESIGN.md) was derived. It exists so that every
design decision can be traced to an observed failure or an observed success, and so that
a reader can see what was considered and rejected, not only what was kept.

The source material was a large private engineering environment: instruction files across
roughly twenty repositories, one very large monorepo with a mature policy and hook
corpus, several years of session notes and retrospectives, and four earlier attempts at a
supervisory tool. Nothing from that material is reproduced here. Paths, names, quotations,
and internal vocabulary have been removed. Magnitudes are kept because they are the
evidence.

Method: study → abstract → challenge → synthesise. Not copy → rename → publish.

---

## 1. Product Definition

**Prismatic Majordomus** is a lightweight supervisory control layer for AI-assisted work.
It holds one canonical policy, projects it into whatever instruction format each AI
worker reads, keeps durable task state outside every conversation, coordinates scope
between concurrent workers, and refuses to call work finished until a finish contract is
met.

It is not a model, not an agent framework, and not a runtime. It supervises workers; it
does not run them.

One sentence: *AI workers should not manage themselves.*

## 2. The single root cause

Every extraction thread converged on one finding, and it is stated first because the
rest of the design is downstream of it.

**Enforcement is declared in prose and never reconciled against what actually runs.**

Independent instances found in the source material, each verified by reading the wiring
rather than the documentation:

| # | Declared | Actual |
|---|---|---|
| 1 | 39 pre-commit scripts, README describing a fallback dispatch loop | dispatcher hard-codes 5; 34 orphaned; no loop exists |
| 2 | two worktree guard hooks, policy frontmatter reads `BLOCKING` | never dispatched; their self-test is `echo OK; exit 0` |
| 3 | a filename-convention hook "enforced since" a given date | not wired for months; then wired with a flag bug that matched zero files on every platform |
| 4 | catalogue of 546 AI agents cited as headline capability | 1 of 183 sampled entries actually registered with the runtime |
| 5 | 7 runtime hooks under a tool's config directory | no settings file in the repository had a hooks key |
| 6 | a well-engineered commit guard with self-test and rationale | registration lost when unrelated tooling rewrote the settings file; hook, tests, and docs all still present |
| 7 | a tool-call budget governor (read clamp, subagent cap, output condensation) | installed, never registered, state directory empty |
| 8 | a session guard with digest-based drift detection | ran for one day; 18 status files; nothing since |
| 9 | two audit ledgers mandated by the enforcement contract | never created |
| 10 | a protected-branch config file read by the override validator | does not exist |
| 11 | a compression module with byte-exact validation table, "Production Ready" | module does not exist |
| 12 | a prior extraction attempt claiming 106 artefacts | shipped 2 |
| 13 | a claims registry reporting "100 % verified, zero violations" | the baseline file one directory away records 27,263 |
| 14 | a doctrine pillar with module, policy, and hook | registered in no pillar list; never executed |
| 15 | a process-timeout option on a runtime call | the function has no such option; the rescue branch is dead code |
| 16 | five scripts headlined as the recommended onboarding workflow | none exist |
| 17 | a `--changed` scoping flag on a CI job | computes the list, prints it, runs unscoped anyway |

Seventeen is not exhaustive. It is the count at which the pattern stopped being
interesting to extend.

The consequence for the design: `doctor`'s primary deterministic guarantee is that every
enforcement the policy declares resolves to an artifact that exists, is executable, and
is reachable from the named dispatcher. That check is roughly sixty lines. The source
environment described it in five places and implemented it in none.

## 3. Pattern Ledger

Columns: **Proven** means observed to work in production, not merely designed.
**Generalizable** means usable by an ordinary team with git and a shell, no other
runtime. **Complexity** is an estimate for a portable-shell implementation.

| Pattern | Problem it addresses | Evidence | Proven | Generalizable | Complexity | Include |
|---|---|---|---|---|---|---|
| Enforcement wiring reconciliation | declared-but-not-wired checks | 17 instances above | no — never built | yes | ~60 lines | **yes, core** |
| Git-derived identity on state records | workers hallucinate branch/head | handover writer forbids authored identity; resolver validates | yes | yes | low | **yes** |
| Read-time divergence label (`exact/advanced/diverged/different_context`) | stale state trusted silently | handover resolver via merge-base | yes | yes | low | **yes** |
| Scoped recovery, never repo-wide fallback | unrelated note leaks into context | resolver tiers; absence exits 0 | yes | yes | low | **yes** |
| Append-only, atomic, never-staged handover | overwritten or auto-committed state | hard-link publish with retry | yes | yes | low | **yes** |
| Required handover sections enforced at write | empty continuation records | awk section check refuses write | yes | yes | low | **yes** |
| Always-loaded budget with failing check + pointer integrity | contract grew 0→1,100 lines over 200 edits | 200-line cap held at 49; the uncapped sibling regrew 3.6× | yes | yes | low | **yes** |
| "Unscoped rule is always-loaded context" | hidden context growth | linter line flagged it | yes | yes | trivial | **yes** |
| Counts computed, never written | five contradictory app counts, three agent counts, five pillar counts across authority files | a validator exists and still found 15 discrepancies | partly | yes | low | **yes** |
| Separate axes: model / effort / verbosity / context / verification | "Sonnet with Reasoning" treated as a model | current rubric separates model and effort; verbosity absent everywhere | yes (2 axes) | yes | trivial | **yes, extended** |
| Effort as delta from default | profile sprawl | "set only where it differs" | yes | yes | trivial | **yes** |
| Named execution profile bundling the axes | no such bundle existed | a budget governor had an implicit unnamed profile with drift alarms | no | yes | low | **yes** |
| Typed outcomes (closed vocabulary) | `no_match` vs `failed` indistinguishable in prose | 157 of 298 notes carried a free-text status; four inconsistent enums | no | yes | trivial | **yes** |
| Count-ratchet (one integer baseline; increase fails; missing baseline warns) | legacy dirt blocks everyone or is allowlisted away | four live ratchets; absolute gates hit 1,400 findings and were allowlisted to 8 | yes | yes | ~40 lines/metric | **yes** |
| Hard-zero for catastrophic classes, never ratchet | a ratchet permits the worst instance | decision matrix in source | yes | yes | trivial | **yes** |
| Exit-code contract; no "warn and continue" | fail-open hooks | contract states it; scanner catches `\|\| true` | yes | yes | trivial | **yes** |
| Trailer proves authorship, never that a gate ran | commit-message markers accepted as evidence | postmortem in source | yes | yes | policy | **yes** |
| Authorization inputs derived or corroborated, never ambient | override validator defeated by env vars | postmortem in source | yes | yes | low | **yes** |
| Self-disabling bootstrap hatch | shipping a gate into a repo that predates it | one record, worked as designed | yes | yes | ~10 lines | **yes** |
| Claim normalisation + containment both directions | trailing-slash and subtree overlaps undetected | live registry shows both classes | no — was broken | yes | low | **yes** |
| Overlap on touched files, not declarations | 3,200 collisions invisible to declaration checks | forensic collision matrix | no | yes | low | **yes** |
| Claim folded into `start`, not optional | 10 of 18 worktrees had empty claims | compliance audit | no | yes | trivial | **yes** |
| Registry rebuildable from git | 82 % stale registry entries at one audit | repair command existed | partly | yes | low | **yes** |
| Retention cap on append-only stores | 10 GB of unread metrics snapshots | none existed | no | yes | trivial | **yes** |
| Tool self-applies its own checks | 500-line anti-sprawl linter decayed in 19 days | regression measured | no | yes | policy | **yes** |
| Published "intentionally absent" list | readers assume omissions are oversights | one protocol README did this | yes | yes | prose | **yes** |
| Reproduce command on every finding | audits contained false claims caught only by a second reader | multiple | no | yes | policy | **yes** |
| Semantic output condensation (head + diagnostic lines + tail, self-stamped) | oversized tool output | governor implementation | yes, briefly | provider-specific | low | **deferred v0.2** |
| Read-size clamp at tool boundary | worker inhales 5,000-line file | governor implementation | yes, briefly | provider-specific | trivial | **deferred v0.2** |
| Subagent count budget | fan-out cost | governor set it to 1 globally; rolled back within days | yes, then rejected | yes if task-derived | low | **deferred v0.2** |
| Exclusion telemetry on context packing | under-filled context is undebuggable | packer manifest | yes | yes | low | **deferred** |
| Compliance markers as scoped greppable exemptions | invisible suppressions | convention in source; one marker documented as not working | partly | yes | low | **deferred** |
| Task dependency edges | — | one demo file, subtasks pending a year later | no | unclear | medium | **rejected v0.1** |
| Description-based routing with trigger phrases | choosing procedures | works in source | yes | tool-specific | — | **projection concern only** |

## 4. Patterns Rejected

Rejected outright, with the failure that justifies rejection:

| Rejected | Why |
|---|---|
| Named agents, personas, roles, tiers, commanders, registries | 546 entries; 305 were naming variants with no functional difference; 139 were stubs whose capabilities equalled their responsibilities; 109 were pitch-deck slides. A supervisory tool that adds nouns becomes the thing it supervises. |
| An advisory tier that calls itself enforcement | Every advisory tier decayed into decoration. Two-tier "advisory/blocking" was documented and did not exist. |
| Closed enum for worker identity | An 8-value enum used only to build filenames, unvalidated on read, already violated in its own live registry. |
| Hand-maintained dispatcher with meaningful-looking numeric prefixes | 5 of 39 invoked; ordering was decorative; `chmod -x` silently removed a gate. |
| Fail-open guard pattern `if [ -x hook ]; then hook; fi` | Missing executable bit deletes the gate with no error. The correct shape blocks with a restore instruction. |
| Owning the merge | The source implementation force-moved the target branch after rebasing, discarding unpushed commits, with no ancestry assertion. |
| Bundled quality-gate runner inside the coordination tool | Duplicated CI logic; the source audit recommended collapsing it. Shell out to a configured verify command. |
| Global constant budgets (subagents = 1, always haiku) | Rolled back within days. Budgets must be task-derived or the human disables the governor and nothing is measured. |
| Silent mutation of a worker's tool input | Invisible to the worker; drift should be surfaced before it is forced. |
| Bytes ÷ 4 labelled as a token count | Indistinguishable from a measurement in the manifest. Estimates are labelled and excluded from enforcement. |
| Documented metrics with no producer | A compression subsystem with byte-exact retention percentages for code that was never written. The tool refuses to display a metric with no producer. |
| Multiple overlapping state stores | Four coexisting namespaces required a disambiguation table. Ship one store. |
| Hand-rolled restricted-JSON serialiser in shell | Correct only because the writer rejected escaping; cost a sort-key bug, a regex portability trap, and a durability hole. Use a real serialiser. |
| Vestigial schema fields | Four always-null routing fields ported from an earlier system. Every field is written and read or it is removed. |
| Minimum fan-out / minimum agent count | 44 procedures were marked failed unless they spawned 3–6 agents, taxing even trivial ones. |
| Rename accepted as fix | The migration caught itself mechanically renaming a doctrine and reverted. If a check can be satisfied by search-and-replace it is not a check. |
| Mythological vocabulary | 2,850 files still carried banned phrasing a year after the removal order. Ordinary engineering language only. |
| Unbounded session-note directories | 10 GB, 1,496 files, a manual fifteen-to-thirty-minute recovery runbook. |
| Any daemon, server, database, queue, MCP surface, background monitor, vector store | No evidence any of these solved a problem the file-based mechanisms did not. The stdio MCP reader added later in `apps/majordomus-cli/` is none of these: no process outlives its client, no state outlives a process; see `.ai/repo/adrs/0001-rust-cli-and-stdio-mcp.md`. |

## 5. Proposed File Tree

```
.
├── README.md
├── LICENSE
├── SECURITY.md
├── CONTRIBUTING.md
├── AGENTS.md                    # canonical AI-readable contract for this repo
├── CLAUDE.md                    # pointer to AGENTS.md until `update` generates it
│
├── bin/
│   └── majordomus               # single portable-shell entry point
│
├── lib/                         # sourced shell modules, one per subcommand
│   ├── common.sh
│   ├── doctor.sh
│   ├── start.sh
│   ├── check.sh
│   ├── watch.sh
│   ├── update.sh
│   ├── handover.sh
│   └── finish.sh
│
├── share/
│   ├── skeleton/                # what `init` copies into a project's .majordomus/
│   │   ├── policy.yaml
│   │   ├── profiles/            # routine · implementation · debugging · deep-work
│   │   ├── templates/           # handover, completion, decisions, open-questions
│   │   └── providers/           # body.md + one wrapper template per provider
│   └── allow/                   # key allowlists: unknown keys are errors
│
├── .majordomus/                 # this repository supervising itself
├── .githooks/                   # pre-commit runs doctor, pre-push runs finish --check
│
├── test/
│   ├── run.sh
│   └── cases/                   # one script per behaviour, disposable repo each
│
├── examples/
│   └── minimal/                 # a scratch repo walked through start → finish
│
└── docs/
    ├── README.md                # index
    ├── DESIGN.md                # specification
    ├── CLI.md                   # commands, exit codes, target output
    ├── SCHEMAS.md               # every file, with a concrete example
    └── EXTRACTION_REPORT.md     # this document
```

Removed from the initial proposal: `principles/` (folds into `policy.yaml` and README),
`workflows/` (the lifecycle is the CLI; a workflows directory would be a second
description of it), a `scripts/majordomus-*` fan of executables (one entry point,
sourced modules). Nothing was kept because the prompt proposed it.

## 6. Canonical Concepts

Ordinary engineering vocabulary. No court.

| Term | Meaning |
|---|---|
| **policy** | the one canonical YAML file; provider-neutral |
| **profile** | a named bundle fixing effort, verbosity, context toggles, verification, and output contract for a task class |
| **projection** | a provider-specific instruction file generated from policy; fingerprinted |
| **task** | the one active unit of work; `state/current.yaml` |
| **scope** | the normalised path set a task may touch |
| **claim** | scope recorded for overlap detection; part of `start` |
| **checkpoint** | an update to `current.yaml`; staleness is measured from the last one |
| **handover** | an append-only continuation record with required sections |
| **outcome** | one of `completed / partial / blocked / no_match / failed` |
| **finish contract** | the checklist `finish` evaluates before accepting `completed` |
| **ledger** | `state/ledger.jsonl`, append-only, retention-capped, written only by Majordomus |
| **ratchet** | a one-integer baseline that may fall but not rise |
| **wired** | an enforcement whose artifact exists, is executable, and is reachable from its dispatcher |
| **drift** | any deterministic disagreement between policy, projection, state, and git |

## 7. MVP Boundary

**v0.1 can:** hold canonical policy; define four profiles; start a scoped task with a
normalised claim; keep durable state with git-derived identity; label state staleness
at read time; write and resolve handovers; evaluate a finish contract; detect the drift
classes in the design; generate fingerprinted projections for four providers; prove its
own enforcement is wired; run entirely offline in portable shell.

**v0.1 cannot:** invoke a model; measure tokens or cost; route dynamically; hook a
worker's runtime; run anything in the background; coordinate across machines; represent
task dependencies.

The boundary is stated in the README with equal prominence for both halves.

## 8. Extraction / Confidentiality Audit

Performed on this repository's contents before each push.

- No internal paths, hostnames, URLs, customer or case identifiers, or secrets appear.
  Checked by grep for the source environment's directory names, host patterns, and
  vocabulary; zero hits.
- No quotation from private material appears. Magnitudes are reported; sentences are
  not.
- The source environment's doctrine vocabulary (pillar names, acronyms, theatrical
  labels) does not appear.
- The only brand reference is "Prismatic" in the product name and in the one-way
  origin statement.
- The working ledger used during extraction lives in a session scratch directory
  outside this repository and is not committed.
- Dependency direction: none. This repository imports nothing and links to nothing in
  the source environment.

## 9. Risks

| Risk | Mitigation in design |
|---|---|
| Majordomus itself decays like the four tools before it | `doctor` runs against its own installation first; every check has a reproduce command; the README states what is guaranteed versus observed |
| Portable shell limits (macOS ships bash 3.2; BSD awk and sed differ) | no associative arrays, no `mapfile`, no GNU-only flags; tested on both; YAML handled by a small dependency with a documented fallback |
| YAML parsing in shell is fragile | v0.1 restricts the policy schema to a flat, parseable subset and validates it; a real parser is used where available |
| Projections get hand-edited | fingerprints make it visible in one command; the generated header names the regeneration command |
| Users skip `start` and work unsupervised | `check` and `finish` refuse without `current.yaml`; the projected instructions tell the worker to run `start` |
| Over-blocking creates bypass culture | blocking is limited to cheap, deterministic, self-evidently correct checks; everything about work-in-progress is reported, not blocked |
| Scope creep during implementation | the "intentionally absent" list is a public commitment; the next phase's review asks, for each responsibility, documented / represented / validatable / enforced / measurable, and does not confuse the first with the last |
| Two concurrent AI writers in one checkout, which the design says should be two worktrees | Observed while building v0.1 itself: the CLI and the derived website were written by two sessions in one working copy. It held because the scopes were disjoint by directory and both sides declared them to each other before touching anything, which is the claim step the design makes mandatory. The tool cannot yet enforce this on the checkout it lives in; `start` refuses a second task, so the second writer simply did not run it. Recorded as a limitation, not hidden. The cleaner example from the same evening: the site generator was correct against a base URL it never actually resolved against, so every asset 404ed once deployed under the repository path, and neither writer could have caught it alone; it took one screenshot from one side and one fix from the other, and the fix shipped with a check that fails on any unprefixed path. That is the repository's own rule applied to itself: no claim without an executable behind it |
| The brand invites reading it as a slice of the source platform | the origin statement is one-way and explicit; there is no shared code |

## 10. Recommended Implementation Plan

Five phases, each gated by the reality of the previous one, none started before the
previous is reviewed.

1. **Design** — this document and `DESIGN.md`. Complete.
2. **Implement v0.1** — complete; see `CLI.md`, `SCHEMAS.md`, `test/cases/`. Original brief: — the file tree above; behavioural tests in disposable
   repositories for: init happy path, missing policy, invalid YAML, unknown key, missing
   scope, unfinished task, valid completion, handover write and resolve, refusal to
   overwrite, each drift class, and the healthy no-finding case. Every README claim is
   compared against implementation before the phase closes.
3. **Make supervision real** — for each responsibility, answer documented /
   represented / validatable / enforced / measurable honestly; close the highest-value
   gaps in this order: state integrity, policy integrity, drift detection, lifecycle
   integrity, handover integrity, projection drift, budget visibility, coordination
   visibility. Produce a supervision model document that separates guaranteed from
   observed.
4. **Hostile review** — claim/implementation matrix; attempt to delete 30 % of the
   repository; ten-minute new-user walk-through; clean-extraction scan including git
   history; security and portability audit; terminology audit. Returns READY or NOT
   READY with blockers.
5. **Release** — only after READY. Template repository, minimal CI, `v0.1.0`, release
   notes without inflation, remote validation including a fresh template-generated
   repository.

Phase 3 is the next action.

---

## 11. Second pass — session records and a knowledge compiler

The first pass studied instruction files, hooks, state stores and coordination. It did
not study two subsystems that turn out to matter for the same problem: a file-based
session-note discipline, and a stateless Markdown compiler that produced a browsable
generated note vault from a repository's own documents.

They were re-examined when [`M003`](../.ai/repo/project/milestones/M003.yaml) was
planned. The method is unchanged — study, abstract, challenge, synthesise — and so is the
confidentiality rule: mechanisms and magnitudes are reported, code and vocabulary are not.

The two are of very different quality, and the difference is itself the finding. The
session-note discipline is filename-shaped: it enforces a naming grammar and has no model
of what a record contains. The compiler is properly built, and nearly every guard in it
carries a comment naming the failure it prevents.

### What the evidence says

| Observation | Magnitude | Consequence for this design |
|---|---|---|
| A session-note directory with a stated retention of "most recent 50 files" | roughly 10 GB across 585 entries at this pass; section 4 records 1,496 files at the first pass, and both are kept rather than reconciled — the count moved, the volume did not | The bloat was not a retention failure. A second producer wrote non-Markdown snapshots into a directory whose every tool filtered on one extension, so no threshold applied and no audit saw them. Retention has to be a property of a store, not of a file pattern. |
| Its records | free text end to end; a machine-checkable field count of zero, including an identifier field labelled auto-generated that nothing generated | A record that asserts a commit hash nobody computed is worse than no record. Every identity field of a Majordomus session is computed from git and refused in an authored body. |
| Its "compression" step | replaced the original in place with a `grep`-derived digest, keeping the only full copy untracked | Majordomus archives and never rewrites. Nothing summarises a record, because nothing here calls a model and a regex digest is a lossy model with worse failure modes. |
| That step's file selector | a hardcoded year prefix that stopped matching when the year changed; it exits reporting success having done nothing | A maintenance command that can do nothing and report success is indistinguishable from one that works. |
| Its ordering and selection | files selected by filesystem mtime, then grouped by the date parsed out of the filename | Two clocks in one loop, and mtime does not survive a clone. Majordomus orders by the recorded timestamp and breaks ties with the ledger. |
| Its enforcement hook | documented as blocking; silently matched nothing on every platform because NUL-delimited input was piped into a line-oriented filter | The same failure class the first pass found seventeen times, found again in the subsystem meant to prevent it. |
| Its retention numbers | four destination-and-threshold pairs for one dataset, across a policy, a README and two scripts, none reconciled, none with a stated derivation | Retention is one policy block, read by one command. |
| Its compliance percentage | counted exempt files in the denominator | A number that systematically understates the thing it exists to report. |
| The compiler's discovery | driven by `git ls-files`; a last recorded run classified 25,539 tracked Markdown files into 3,655 curated, 2,662 session logs and 19,222 excluded | Repository truth, no exclusion globs to maintain, no build output, no untracked files. Adopted directly. |
| The compiler's edges | 7,731 edges, every one explicit, none inferred | The graph knows only what somebody typed. That is the property worth having first. |
| Its broken links and orphans | roughly a third of its nodes | Reported, never gated, with the reasoning recorded: gating on a count that is large by design trains people to ignore the failure. |
| Its cold run | about 2.1 seconds over 3,655 sources, with the manifest short-circuiting rendering but not analysis | A manifest that only skips writes is worth much less than one that skips work. Measure before believing. |

### Adopted, by re-derivation

Each of these is an idea, re-implemented from scratch in portable shell and awk against
Majordomus's own records. No code, no schema and no vocabulary was carried across.

| Idea | What it becomes here |
|---|---|
| Identity from a stable source fact; hashes only for change detection | A node id is a canonical id or a repository path. `source_hash` says whether it moved, never what it is. |
| Provenance required on every edge | An edge carries the file it was observed in, and an edge without one is a validation failure rather than a silent drop. |
| A confidence vocabulary whose "inferred" value is currently unused | The graph's trustworthiness becomes a checkable claim: the count of inferred edges is zero, and a command says so. |
| Classify from structure and declared metadata, never from prose | A node's kind comes from its source class or an explicit field. A document containing the word "roadmap" does not become a roadmap. |
| `unknown` as a first-class answer | An unrecognised source is a node of kind `unknown`, reported, not guessed at and not dropped. |
| Never default a status to a plausible value | Already the rule for issue status, which is derived and has no stored field. Extended to node kind. |
| Gate only on defects this run caused | A broken reference between two things Majordomus owns is a failure. A link to a deliberately external resource is not. |
| Absent is not corrupt | A missing manifest is a first run. A manifest that does not parse fails loudly, because treating it as absent would silently disable the guarantee it exists to provide. |
| Tag a link by the syntax it was written in, not by the shape of its target | An extension-less relative link is a link because of how it was written. |
| Strip fenced code before scanning for links | A path inside a code sample is an example, not a reference. |
| Order-independent, hash-backed collision disambiguation | A readable fragment is a hint; the hash is the guarantee. |
| Record both values when two sources of one fact disagree | Report the drift rather than picking a winner and hiding it. |
| A per-file failure is an error, not an aborted run | One malformed input must not cost the build. |
| Sorted, de-duplicated generated output | The artefact is diffable, so a rebuild's effect is reviewable. |
| Project a high-volume, low-durability corpus as one index | Checkpoints are referenced from their session, not made one node each. |
| Filename-carried UTC timestamps for chronological records | Already the shape of a checkpoint and a handover here, and it already carries the sub-day resolution and the uniqueness component the studied grammar lacked. |
| Skip the heavy runtime when the job does not need it | Already true by construction: there is no runtime to skip. |

### Refused

| Refused | Why |
|---|---|
| Any database — embedded, relational, columnar or graph | The corpus is a few thousand small files. A database is a dependency, a migration story and a second source of truth bought before anything measured a need for one. Measure first. |
| Embeddings, vector search, similarity, clustering, automatic taxonomy | None can name the line that justifies the relation, which is the test the extraction boundary now uses. They are also unfalsifiable by the person best placed to notice they are wrong. |
| Any generated summary of a session, by a model or by a regex | Majordomus calls no model, and the studied regex digest is the case study in why the cheap substitute is worse. A worker writes its own summary or the record has none. |
| A note-vault renderer, and any dependency on a particular note-taking application | The graph is renderer-independent. A renderer can be added later against the same generated data; nothing in the core may assume one. |
| Copying record bodies into a session note | The single most consequential defect of the studied session store. A session is an envelope of references. |
| A mutable session file that other commands append references to as they run | It puts a write on the hot path of every command and recreates the second store. The envelope is derived from the ledger at close. |
| Destructive compression or in-place rewriting of any record | Archiving moves; it never rewrites and never overwrites an archive. |
| Filesystem mtime as an ordering or selection input | It does not survive a clone and it is not the time the record asserts. |
| A retention rule that names a file pattern rather than a store | The 10 GB directory is what that costs. |
| Bypass channels for an enforced rule | Two existed there, in a discipline whose own policy said it could not be bypassed. |
| A second "search" whose contract quietly replaces the first | `search` stays a literal scan over durable records with no index. `knowledge search` is a different corpus with a different contract, and both say so. |
