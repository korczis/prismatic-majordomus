+++
title = "Second adoption in a foreign repository"
description = "forensic finding, open: the 24 things a second adopting repository met on its first day, where each stands on the trunk, and the issue of the milestone `a-second-adopter-is-served` that owns it"
weight = 33
[extra]
source = "docs/ADOPTION_SECOND_RUN.md"
+++

{% raw %}

What a repository that shares nothing with this one met on the day it adopted Majordomus, finding by
finding, with how each was observed. The first such record is `ADOPTION_FIRST_RUN.md`; it concerned the
vendored rules of one release. This one concerns a whole first day: `init`, the projections, `doctor`, the
plan, evidence, the pull-request lane and the UI.

## 0. What this was measured against

Adopter: `/Users/korczis/dev/progresus-ark-orchest` (a pnpm/TypeScript monorepo, vitest, GitHub
flow with `develop` → `main`, deployed with Docker Compose to two GCP virtual machines, no git
tags, 4,358 commits). Tool: majordomus 0.21.0, then 0.22.0 (installed), UI served by a 0.23.0 debug
build from the worktree `fix/an-adopting-repository-is-not-read-as-unknown`. Date: 2026-10-13.
Every item was observed, not inferred, unless it says so. "Reproduce" is run in the adopter.

Each finding was then read again in this repository at master `e506c041a4` (0.23.0). The plan holds one
issue per finding under the milestone `a-second-adopter-is-served`; G11 and G21 follow from G12 and G18
and keep an issue of their own because their messages mislead on their own.

## 1. The findings and where they stand

<div class="overflow-x-auto" tabindex="0">

| Finding | Issue | Standing on master `e506c041a4` |
|---|---|---|
| G01 | I2585 — An untracked layer is read the same by both programs, and says that it is untracked | changed: the executable reads an untracked layer since ADR 0145 (I2491); the shell tool does not, and neither says so |
| G02 | I2586 — doctor counts every failure it prints | still present |
| G03 | I2587 — The reasoning check judges a gate, not every workflow that names a credential | still present |
| G04 | I2588 — A region projection's host document is not judged as a bootstrap | still present |
| G05 | I2589 — init projects into a region where a hand-written target already exists, and update says append | still present |
| G06 | I2590 — The bootstrap states a worktree layout only where the policy enforces it | still present |
| G07 | I2591 — A projection target the policy declares is in scope without being listed twice | still present |
| G08 | I2592 — Every directory a command writes under the layer carries its contract | still present |
| G09 | I2593 — A deployment declares what runs where without inventing a cargo package | changed: #1032 removed the provider block; the contract still requires a cargo build and a machine pool |
| G10 | I2594 — The declared version is read from where the repository declares it | still present |
| G11 | I2596 — An intent criterion's test reference resolves through the declared runners | still present |
| G12 | I2595 — A repository declares its test runners, and a declared runner's tests are runnable | still present |
| G13 | I2597 — A plan derived from history is sealed at the times it happened | still present |
| G14 | I2598 — plan answers in time linear in its issues and names a scope conflict once | still present |
| G15 | I2599 — plan status reports the project header | still present |
| G16 | I2600 — A commit reference has one grammar for every writer and every checker | still present |
| G17 | I2601 — majordomus evidence reaches record, stamp and show | still present |
| G18 | I2602 — A document with a front matter of its own is indexed, and a block scalar is read | still present |
| G19 | I2604 — The pull-request lane integrates on the branch the repository declares | still present |
| G20 | I2605 — Every record with a Markdown body has a rendered page in the UI, with its diagrams | still present |
| G21 | I2603 — A document the index refused is named as refused, not as unknown | still present |
| G22 | I2606 — A gate model without classes is a model | still present |
| G23 | I2607 — product validate measures an adopter against what the adopter owns | still present |
| G24 | I2608 — A plan is derived from what a repository already has, with provenance | still present |

</div>


## 2. What they have in common

Most findings are one of three shapes, and the fix for a shape is the same each time:

- **A constant of this repository read as every repository's.** The crate manifest as the place a version is
  declared (G10), two test directories as the only tests (G11, G12), one date as the day seals began (G13),
  the forge's default branch as the integration branch (G19), this repository's worktree layout in every
  bootstrap (G06). The fix is a declaration with today's value as its default, never a test of which
  repository this is.
- **Two programs that answer one question differently.** Discovery of an untracked layer (G01), a commit
  reference (G16), the `evidence` verb (G17), the project header (G15). The fix is one rule held by a case
  that asks both.
- **A check written for this repository's own files, run on an adopter's.** The reasoning check over every
  workflow (G03), the bootstrap check over a host document (G04), product coverage of the tool's own
  modules (G23). The fix is to judge what the adopter declared.

The rest are omissions an adopter meets at once: no contract for directories the tool itself creates (G08),
no page for a record's body (G20), nothing that derives a plan from the history a repository already has
(G24).

## 3. The findings as observed

## G01 An untracked layer reads as an empty repository, with no hint
After `majordomus init` with `.ai/` excluded through `.git/info/exclude`, every UI page and
`/api/v1/objects?kind=adr` answered as if nothing existed ("kind 'adr' is not among the kinds this
repository has: document"); `adr list` printed `(none)` while `adr check` said "53 identities added
here". Nothing said "the layer exists on disk and is not in the index". Cause: discovery is the git
index. Staging the layer (`git add`) made everything appear.

## G02 `doctor` prints 10 FAIL lines and summarises "8 failure(s)"
Reproduce: `majordomus doctor | grep -c '^FAIL'` vs the last line. Seen right after init: FAIL lines
bootstrap×3, wiring×2, projection×3, reasoning×2; summary `doctor: 8 failure(s)`. Later 3 FAIL lines
with `doctor: 2 failure(s)`. The `reasoning` FAIL lines look uncounted.

## G03 The reasoning CI check fails on a credential named in a comment and in non-gates
`FAIL reasoning <workflow> — ci: CI names the model credential
ANTHROPIC_API_KEY; no gate may depend on a live model`, where the workflow is `issue-mining.yml` in the adopter's `.github/workflows/`. The only occurrence in that file is a YAML
comment (line 100). `e2e.yml` sets it as env for an opt-in, non-required, manually skippable job.
A scheduled automation that runs a model on purpose is not a gate. No policy key exempts a file or
declares which workflows are gates, so an adopter can never get `doctor` green.

## G04 `bootstrap` fails a host CLAUDE.md for "a rule corpus of its own" even in `mode: region`
After `projections[].mode: region` and `majordomus update`, doctor says `FAIL bootstrap CLAUDE.md —
carries a rule corpus of its own (a profile table, rule bullets or a rules section)`. docs/ADOPTION.md
says of region mode that "an edit outside the markers is the repository's own business and is never
reported as drift".

## G05 `init` leaves projections in whole-file mode over existing hand-written files
The repository had a 96-line CLAUDE.md and a 142-line AGENTS.md. `init` wrote the policy with
whole-file projections for both; `update --dry-run` printed `create CLAUDE.md`. Region mode had to be
found in ADOPTION.md and set by hand. Also `update --dry-run` prints `create` for a target that
exists and will only get a region appended.

## G06 The generated bootstrap states the tool's worktree layout as this repository's
The provider templates carry "Linked git worktrees of this repository live at
`<repository>-wt/<branch>` …", which contradicted the adopter's own worktree rule. No policy key
turns the paragraph off or points it elsewhere; the adopter had to copy and edit three templates.

## G07 `update` creates GEMINI.md and then warns that it is out of scope
`WARN discovered by class 'readme' but out of the repository scope (undeclared); not read
code=out_of_scope source_path="GEMINI.md"`. `init` wrote `.ai/repo/scope.yaml` listing AGENTS.md and
CLAUDE.md but not the third projection target the same policy declares.

## G08 Directories the tool itself uses fail `doctor` for a missing context contract
`FAIL context .ai/repo/ci — missing-contract`, same for `.ai/repo/features/domains`,
`.ai/repo/providers`, and `.ai/repo/evidence` (created by `evidence record` itself). The skeleton
ships no README for them, and `evidence record` creates its directory without one.

## G09 `deployment/v1` admits one provider, `fly`
`provider.name` is an enum of `fly`; `build.package`/`binary`/`profile` and `machines.autostart`
are that provider's vocabulary. A product deployed elsewhere cannot declare a deployment truthfully,
so the Deployments page stays empty. (A branch `chore/fly-is-not-a-target`, PR #1032, may address it.)

## G10 Release and version surfaces assume the tool's own crate and tags
Delivery shows `Declared unknown` (release.version reads "the version the crate manifest declares")
and "the commits since the last release imply a major release" in a repository with no tags and a
`package.json`/CHANGELOG release model. Nothing lets an adopter say where its version is declared.

## G11 Intent criteria with `evidence: test` accept only the tool's own test layout
`FAIL unresolved_evidence_ref … names no test: give a path under test/cases/ or
apps/majordomus-cli/tests/, or suite:<case> / crate:<binary>`. An adopter's tests live elsewhere.

## G12 Evidence: a claim whose test is not a shell case or a crate test is `unrunnable` for ever
19 claims naming vitest files read `unrunnable — names a path no runner drives`. The adopter had to
write `test/run.sh` and 19 shell wrappers in the tool's TSV format to record a run. There is no
declared way to say "this command runs these test paths" (a declared runner, or a generic
command runner).

## G13 Plan stamps on or after one global date can never be imported
`SEALED_FROM = "2026-10-01T00:00:00Z"` is a constant (lib/project.awk:45, plan.rs). An adopter
deriving its plan from history got 457 `stamp_without_event` failures for work merged in October and
had to run `plan start|verify|done` 3×145 times, stamping today's time on work finished days ago.
There is no import/backfill verb that records "this happened then, sealed now".

## G14 `plan` is slow and floods warnings at a few hundred issues
With 541 issues: `majordomus plan list` takes about 20 s (4 s user, 11 s system), each
`plan start|verify|done` the same; `plan validate` prints 11,860 `WARN scope_conflict` lines, one per
pair of open issues that share a scope path. No aggregation, no bulk lifecycle command.

## G15 `plan status` does not report the project header
`/api/v1/plan/status` returns `"project": {"name": "", "repository": "", "default_branch": ""}` although
`.ai/repo/project/project.yaml` holds `name: Ark Orchest`, `repository: able-cz/progresus-ark-orchest`,
`default_branch: main` (two comment lines above them).

## G16 A commit reference `adr propose` accepts, `knowledge check` refuses
`adr propose --from commit:62a0a417` and a knowledge record with `commit:<8 hex>` were written;
`knowledge check` then failed 75 of 76 records: "names `commit:86ace934`, which resolves to nothing".
Full 40-hex SHAs passed. Either both normalise an abbreviation or the writer refuses it.

## G17 `majordomus evidence record|stamp|show` is documented and the shell tool refuses it
docs/EVIDENCE.md says `majordomus evidence record --suite …`; the installed `majordomus evidence
record --help` answers `majordomus: evidence: unknown option record`. Only
`libexec/majordomus-cli evidence …` works, which is not on PATH.

## G18 A repository's own Markdown with its own front matter is refused as a document, loudly
Adding source classes of kind `document` for `docs/**` made every Rust command log one
`ERROR key(s) not in schema 'majordomus.document/v1': date, ticket …` or
`ERROR unsupported YAML construct on line 2: >` per file (99 of 102 specs). A document class should
be able to carry a foreign front matter (ignored or passed through), and folded/literal YAML scalars
(`>`, `|`) are ordinary YAML.

## G19 The pull-request lane can never be ready in a repository that integrates on `develop`
`majordomus prs status`: all 31 open pull requests `other_base … base_is:develop`, plus
"allows no merge commit (it allows: nothing): … no pull request can be ready" and "the branch
protection or rulesets could not be read". The lane assumes pull requests target the default branch;
no policy key names the integration branch.

## G20 Record bodies are not rendered as Markdown with diagrams in the UI
(Stated by the owner as a requirement: every page fully rendered Markdown, mermaid.js diagrams, one
renderer, enforced.) ADR, knowledge, rule, feature and document bodies are Markdown; the UI has no
ADR or knowledge page in its navigation at all (only `/api/v1/objects?kind=adr`), so 53 ADRs and 76
knowledge records are invisible there. PR #1050 `feature/markdown-is-rendered-once` may address the
renderer.

## G21 Feature `docs:` references reject documents the same repository tracks
`unknown document reference: "<runbook>"`, where the runbook is `vps-google-cloud.md` in the adopter's `docs/product/06-guides/runbooks/` — because G18
refused the file as a document. Follows from G18; listed because the message blames the reference.

## G22 `gates.yaml` without `classes` is "does not parse"
`".ai/repo/ci/gates.yaml does not parse: missing field `classes`"` and the gate count reads 0. A model
with gates and no classes is a meaningful state (every gate always, or none selected).

## G23 `product validate` warns about the tool's own modules and kinds in an adopter
99 warnings of the form "the kind 'profile' is named by no stable feature; the product page says
nothing about it" and `modules 0/56` — the floors measure the tool's own capability registry against
the adopter's product features.

## G24 Nothing derives a plan, claims or gates from what an existing repository already has
(Requirement rather than defect.) The adopter's milestones were derivable from CHANGELOG release
sections, its issues from merged pull requests, its gates from workflow jobs, its claims from guard
tests. All of it was hand-scripted (about 600 lines of Python). A generic, declared derivation
(`majordomus derive plan --from forge|changelog`, with provenance on every record) would have filled
Roadmap, Issues, Delivery and Evidence on day one.
{% endraw %}
