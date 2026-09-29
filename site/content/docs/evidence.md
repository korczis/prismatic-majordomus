+++
title = "Evidence"
description = "a claim's proof as a recorded execution rather than a test path that resolves: the test identity derived from the matrix, the seven proof states and how each is derived, why `proven` and `inputs unchanged` are never one state, the ledger's shape and its retention decision, recording a run, the four capabilities with their projections, both directions of the join, the gate and its ratchet, and the limits"
weight = 58
[extra]
source = "docs/EVIDENCE.md"
+++

{% raw %}

How this repository knows that a claim is proven rather than merely referenced: a stable
identity for every test, derived from the path a claim already names; a recorded execution
carrying the provenance the run itself did not have; a ledger of the latest execution per
test; and one derivation that joins the two and says, per claim, exactly what can honestly
be said about it. Behaviour as implemented; where this document and the executable
disagree, the document is wrong and changes in the same commit. The decision is
ADR 40;
the rules it serves are `project.no-claim-without-test` and
`project.never-reported-is-not-green` under
[`.ai/repo/rules/project/`](https://github.com/korczis/prismatic-majordomus/tree/master/.ai/repo/rules/project).

<pre class="mermaid">
flowchart TD
  claims["docs/CLAIMS.yaml&lt;br&gt;claim → source, implementation,&lt;br&gt;test (a path)"]
  testid["TestId::of(path)&lt;br&gt;suite:84_distribution_model&lt;br&gt;crate:why"]
  ledger[".ai/repo/evidence/ledger.json&lt;br&gt;latest execution per test:&lt;br&gt;outcome, seconds, commit, tree,&lt;br&gt;digest, time, origin, command"]
  latest["ledger.latest(id)"]
  report["evidence::report()"]
  states["proven · inputs_unchanged · stale · failing&lt;br&gt;not_run · unrunnable · no_test"]
  out["CLI · HTTP · MCP · scripts/evidence-check"]
  claims --&gt; testid --&gt; latest
  ledger --&gt; latest
  latest --&gt; report --&gt; states --&gt; out
</pre>


## The problem

`docs/CLAIMS.yaml` binds a claim to a test with a **path**:

```yaml
- id: distribution-canonical-model
  claim: Every platform, artifact name and installation URL is derived from one model …
  source: docs/DISTRIBUTION.md
  implementation: apps/majordomus-cli/src/distribution/mod.rs
  test: test/cases/84_distribution_model.sh
  status: guaranteed
```

A path is a declaration that a proof exists somewhere. It is not the proof.
`scripts/generate-site-data --check` verifies that every `source`, `implementation` and
`test` path resolves — a check on the *reference*, not on the *claim*. Until this
subsystem, nobody could answer from the repository whether that case had ever run, against
which revision, with what result, or whether the result still applied, and the sentence was
rendered on the public site as a guarantee on the strength of a file existing.

This is the same defect [ADR 30](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/adrs/0030-a-task-owes-obligations-and-evidence-goes-stale.md)
named one level up: *a test result that cannot go stale is a claim about the past presented
as a claim about the present.*

## The model

Three objects and one derivation, in
[`apps/majordomus-cli/src/evidence/`](https://github.com/korczis/prismatic-majordomus/tree/master/apps/majordomus-cli/src/evidence).

<div class="overflow-x-auto" tabindex="0">

| | what it is |
|---|---|
| `TestId` | the stable identity of a test: the runner that owns it, and the name that runner knows it by |
| `Execution` | one recorded run of one test, with the provenance the run itself did not carry |
| `Ledger` | the latest execution of every test, as a tracked file under `.ai/repo/evidence/` |
| `report()` | the join: every claim of the matrix against every execution recorded, deriving one `ProofState` per claim |

</div>


The join is derived on every read and is never written down. There is no second place where
a claim's proof state is stored, and therefore no place for it to go stale.

### Test identity, derived rather than entered

A test's identity is computed from the path a claim already names. Nothing is entered by
hand, no field is added to the matrix, and no claim had to be migrated:

<div class="overflow-x-auto" tabindex="0">

| path a claim names | identity | the test's own source | the command that runs it |
|---|---|---|---|
| `test/cases/84_distribution_model.sh` | `suite:84_distribution_model` | the same path | `bash test/run.sh 84_distribution_model` |
| `apps/majordomus-cli/tests/why.rs` | `crate:why` | the same path | `cargo test --test why` |
| anything else | none | — | — |

</div>


Both spellings already existed: `test/run.sh` names a case by its file stem, and `cargo
test --test <stem>` runs an integration binary by its. `TestId::of` only names the join.

"Anything else" is an answer, not a failure. A claim may name a template, a fixture, a
library or a document — `share/install/install.sh.in`, `test/lib.sh`, a nested path under
`test/cases/` that the runner does not enumerate — and the report says `unrunnable` rather
than counting it as covered. The matrix's own placeholder for nothing, `-`, quoted or not,
reads as no test at all.

A third runner would be a variant of `Runner` and a branch in `TestId::of`. It is
deliberately a code change rather than a configuration one: a runner nothing executes is a
runner whose results nothing can record.

### An execution

Every field is provenance. A result with no commit is an anonymous green, which is the one
thing a badge must never be derived from.

<div class="overflow-x-auto" tabindex="0">

| field | what it holds |
|---|---|
| `test`, `runner`, `source` | which test, whose, and where its own source lives |
| `outcome` | `pass`, `fail`, `skip`, `timeout` or `error` |
| `seconds` | how long it took, whole seconds |
| `commit` | the full object name the run was made against |
| `working_tree` | `clean`, `dirty` or `unknown` — whether that commit describes what ran |
| `digest` | `sha256:<hex>` of the test's own source as it was when the run was recorded |
| `at` | RFC 3339, UTC, to the second |
| `origin` | `local`, `ci` or `release` |
| `command` | the exact command that runs this one test again |
| `run` | the CI run it was recorded in — provider, identifier, attempt, workflow, job and address — for a `ci` recording made inside one; absent otherwise (see *Recorded in CI*) |

</div>


Only `pass` proves anything. The runner's word is read case-insensitively (`ok`, `pass`,
`passed`; `fail`, `failed`; `skip`, `skipped`; `timeout`) and **anything unrecognised is
`error`** — a result nobody can classify is not a pass. There is deliberately no `not_run`
outcome: that is the absence of an execution, not an outcome one had, and encoding it would
let a recorder write "this did not run" and have it counted among the things that did.

`origin` is provider-neutral. A CI adapter records `ci`; nothing in the model knows or
cares which CI it was.

## The seven states

Ranked from strongest to weakest, so a summary that sorts by the state reads as a ranking.
Each state carries its own one-sentence meaning in the model, so every surface says the
same thing rather than inventing a gloss.

<div class="overflow-x-auto" tabindex="0">

| state | in short |
|---|---|
| `proven` | a passing run the presented revision contains, nothing but the ledger changed since, on a clean tree at both ends |
| `inputs_unchanged` | a passing run, and **nothing the claim itself names** has changed since — or it has not, but one of the two trees was not its commit |
| `stale` | a passing run that no longer proves the presented revision: something the claim names changed, the revision does not contain it, git could not compare, or a failure the checkout holds withholds it |
| `failing` | the latest execution of this claim's test failed, timed out or could not be run |
| `not_run` | the claim names a test a runner owns, and no execution that ran it has been recorded — never run, or it declined to run |
| `unrunnable` | the claim names a path no runner in this repository drives, so no execution of it can ever be recorded |
| `no_test` | the claim names no test at all — correct for a `planned` or a `rejected` claim, a defect for any other |

</div>


Every state is decided by one function, `evidence::freshness`, from the recorded run and the
revision it is judged at, and the first row that matches wins. E is the commit the run was
recorded on; "the presented revision" is what is judged (below); "the inputs" are what the
claim names — exactly three paths: its `source`, its `implementation` and the test's own
source (a rule names its test's source and its own definition). "Changed" is every path
that differs between E and the presented revision, and changed' is that set without the
ledger's own row.

<div class="overflow-x-auto" tabindex="0">

| # | when | state | the report adds |
|---|---|---|---|
| 1 | the claim names no test | `no_test` | |
| 2 | it names a path no runner drives | `unrunnable` | |
| 3 | nothing is recorded for the test | `not_run` | |
| 4 | the run was a skip | `not_run` | detail: the test declined to run |
| 5 | the run failed, timed out or errored | `failing` | detail: which |
| 6 | a pass, and git could not compare E with the presented revision | `stale` | detail naming E |
| 7 | a pass on an E the presented revision does not contain | `stale` | detail naming E |
| 8 | a pass whose test no longer hashes to its recorded digest, and no input changed | `stale` | `changed`: the test |
| 9 | a pass, and an input changed | `stale` | `changed`: those inputs |
| 10 | a pass, changed' empty, the run's recorded tree clean, the presented tree clean | **`proven`** | |
| 11 | as 10, but the run's recorded tree was `dirty` or `unknown` | `inputs_unchanged` | detail: the run measured a tree that was not its commit (`dirty`), or the run's tree was not measured, so it cannot be shown to be its commit (`unknown`) |
| 12 | as 10, but the presented tree was `dirty` or `unknown` | `inputs_unchanged` | detail: the presented revision was built from a tree that was not its commit |
| 13 | a pass, changed' not empty, and none of it an input | `inputs_unchanged` | |
| 14 | as 13, but the route names no inputs at all | `stale` | detail: a change since the run cannot be ruled out |

</div>


Only row 10 is `proven`, and only `proven` may be rendered as verified on any surface. A
`stale` claim names the paths that changed, or says in its detail why it is stale, so a
reader is told what invalidated the run rather than being told to go and look. Not knowing
is not proof: an unanswerable comparison is row 6, never row 13.

**The presented revision.** `evidence show` judges the working tree: HEAD plus every
tracked, staged and untracked change, compared with `git diff --name-only E --` together
with the untracked files git would show. `evidence show --presented HEAD` judges the
checked-out commit as committed. The commit must be the one checked out — the claims, the
tests' sources and their digests are read from the checkout, so a verdict at any other
commit would mix two trees — and the ledger is the one committed in it, compared with `git
diff --name-only E <commit> --`. The presented tree is measured ignoring the ledger's
working copy, which is not what is judged, and `--presented-tree` can only weaken that
measurement. The report says what it was judged at in `presented`. This is the reading a
site built from a commit needs, and no surface asks for it yet: the evidence publication
(issue I1955) and the site build that binds it to `build.json` (issue I1959) are the later
slices that will pass `build.json.commit` as `--presented` and a build with
`build.json.dirty` set as `--presented-tree dirty`. The two measurements differ:
`build.json.dirty` ignores untracked files, and the presented tree counts them. The
narrower one cannot make a verdict proven, because the presented tree is measured here
whatever the caller declares, and a declared state can only weaken it.

**The working ledger's uncommitted executions.** Under `--presented`, the executions the
working copy of the ledger holds that the committed ledger does not are listed in
`presented.uncommitted`, and read only through the monotone rule: a supplementary record
may withhold `proven` and never grant it. When one of them failed, timed out or errored,
on a clean tree, at a commit that contains E and that the presented commit contains, a
`proven` or `inputs_unchanged` verdict is capped at `stale`, with a detail naming the run.
The cap is `stale` and not `failing`: such a record never decides a verdict, and an
uncommitted pass never strengthens one. A working ledger that cannot be read is refused,
because a run it may hold cannot be ruled out.

**A skip is not a failure.** A test that declined to run proved nothing and failed nothing:
it is `not_run`, with a detail saying it declined, and a guarantee resting on it is named by
a finding that says its test declined to run. A test that errored or timed out did not
decline: it is `failing`.

**Containment.** A pass recorded on a commit the presented revision does not contain is a
fact about another history — a branch that was never merged, a rewritten one — however
empty a diff between the two trees happens to be. It is `stale`, and its detail names the
commit. A commit this clone does not have is neither contained nor not: git cannot answer,
which is row 6, and nothing is diffed against it. A ledger row's commit is data, so a word
git would read as an option (`--output=<file>`) is never handed to git: it names no
commit, and it is row 6 too.

**Aggregation.** Where several states make one — a rule over the tests it names — a
failing part makes the whole failing, whatever the other parts say, because the ranking
puts `failing` above `not_run` and the weakest of a failing test and a skipped one would
otherwise read `not_run` and hide the failure. Otherwise the whole is the weakest of the
parts that can carry proof, and a part nothing can ever record counts only when it is all
there is.

### `proven` and `inputs_unchanged` are deliberately not the same state

This is the point of the whole subsystem, and it is the thing to preserve in any change to
it.

**`proven`** means a passing run exists, *nothing has changed since it*, *the run
measured the commit it is joined to*, and *what is presented is that commit's own
history, as committed*: the presented revision contains the execution's own commit, the
diff between the two is empty but for the ledger, the execution's `working_tree` is
`clean`, and so is the presented tree. It is proof of the tree in front of you — the tree
the run measured is the tree you are looking at.

Each condition is needed, and each fails differently. A run recorded while something else was
pending sat on its commit without measuring it (ADR 0041: "`proven` is a passing run
recorded against this exact commit with a clean tree"), so it is capped at
`inputs_unchanged` however empty the diff is when the report is taken — a later `git
checkout` of the edited file cannot retroactively make the commit describe what ran.

The ledger's own row is excluded from that diff, because the evidence is *about* the tree
rather than part of what the tests measure. Without that exclusion `proven` would be
unreachable by construction: recording writes the ledger and so dirties the tree, and
committing the record moves HEAD past the commit the record names. That was a real defect
in the first cut of this design, found in review; `test/cases/124_evidence.sh` now asserts
both halves — that the tree is dirty after a recording, and that every claim is `proven`
anyway — so a regression that made `proven` depend on a clean tree again would fail.

The same exclusion applies when a run is *recorded*: a recording whose only pending change
is the previous recording's own row is stamped `clean`, because that row is not something
any test measured. Any other pending path is `dirty` and the execution says so.

**`inputs_unchanged`** means a passing run exists and nothing the claim names has moved
since it. That is the **absence of a known invalidation** — not proof at HEAD. Something
the claim does not name may have broken the behaviour: a shared library the implementation
calls, a schema it reads, a script the case invokes, a dependency bump, a change to
`test/run.sh` itself. The dependency model is deliberately shallow and non-transitive: it
is the three paths the claim writes down, and nothing computed from them.

Both alternatives to that choice are worse. Treating a passing run as valid until something
anywhere changes is the green badge whose derivation cannot be inspected. Declaring every
result stale the instant anything anywhere changes is sound and useless — it would mean no
claim is ever proven except in the seconds after a full run, on a repository that pushes to
master every few minutes.

So the model reports which of the two it found, and labels the weaker one as weaker
everywhere it is shown. The command line prints them differently and prints the sentence
that explains the second. **A surface that renders both as one tick has reintroduced the
defect**, whatever the underlying data says.

What `inputs_unchanged` does not catch, stated plainly:

- a change to a file the claim does not name that breaks the behaviour anyway;
- a change to the runner, the harness, the toolchain or the environment;
- a claim whose `implementation` field names one file where the behaviour lives in several;
- a behaviour that depends on something outside the repository entirely.

Each of those is a real hole. The answer to all of them is the same and is not a cleverer
dependency graph: record a run against HEAD, and the state becomes `proven`.

## The ledger

`.ai/repo/evidence/ledger.json`, tracked, one entry per test: the latest execution.

It is shaped like the benchmark baselines under `.ai/repo/benchmarks/` on purpose — JSON,
committed, written by a tool, read by a gate, reviewed in a diff. The repository already
had a precedent for recorded, committed, reviewable measurement, and this is the same kind
of artifact for the same reason rather than something new.

**JSON, not the layer's YAML.** This repository reads a deliberately small YAML subset
([`SCHEMAS.md`](@/docs/schemas.md)), and a canonical file only our own reader can parse is a trap
it has fallen into before. The ledger is machine-written and machine-read, so it is JSON:
`serde` on both sides, no subset to respect, and `jq` works on it.

**Latest per test, no history.** A run per commit for every test grows without bound, and
the question the ledger answers — *is this claim proven now?* — only ever reads the newest.
A history would be a second thing to maintain in service of a question nobody asked here;
the durable record of what happened over time is git, over this file.

**No captured output.** A log of every case is megabytes per run and belongs where the run
happened — the CI artifact, the terminal. What is kept is the durable semantic part, which
is what a reader needs in order to decide whether to believe the result and how to check it
themselves.

**Merged per test, never rewritten per run.** Recording one test replaces that test's entry
and leaves every other alone. This is what makes `bash test/run.sh 84_distribution_model`
worth recording at all: a whole-ledger write would silently delete the evidence for every
test the run did not include, turning a one-case run into a repository that has proven one
thing — which is worse than one that has proven nothing, because it looks deliberate.

**Ordered and stable.** Entries are sorted by test id and the file is written with a
two-space indent and a trailing newline, so two runs of the same set produce the same file
and a diff shows what changed rather than what moved.

**Versioned, and refused rather than guessed.** The file declares `version: 1`. A reader
that meets a version it does not know refuses with both numbers and says why: a misread
ledger is worse than an absent one, because an absent one reports `not_run` and a misread
one could report a pass.

**A missing ledger and an empty one are the same answer.** Both report every claim as
`not_run`, which is true, and neither is an error — a fresh clone has the first and a
repository mid-adoption has the second. The report's ledger summary says which it was,
along with how many executions it holds, the newest timestamp in it, and the distinct
commits they were recorded against.

## Recording a run

The runs already write their results down. `test/run.sh` writes one TSV row per case —
`name`, `result`, `seconds`, `phase` — when `MJ_TEST_REPORT` is set. `cargo test` prints a
`Running tests/<name>.rs` line and a `test result:` line per integration binary. Neither is
durable, neither carries a commit, and neither is joined to anything.

A run measures the checkout it left with `evidence stamp`, and `evidence record` reads what
the run wrote, carries that measurement into each result with the digest, the time and the
origin, and merges it into the ledger.

```sh
MJ_TEST_REPORT=suite.tsv bash test/run.sh
majordomus evidence stamp --producer suite --report suite.tsv --out suite.provenance.json
majordomus evidence record --suite suite.tsv --provenance suite.provenance.json

cargo test --manifest-path apps/majordomus-cli/Cargo.toml 2>&1 | tee crate.log
majordomus evidence stamp --producer crate --report crate.log --out crate.provenance.json
majordomus evidence record --crate-output crate.log --provenance crate.provenance.json
```

Then commit the ledger. A recording that is not committed is a local opinion.

It records; it decides nothing. A case that failed is a case the runner said failed, and a
test that appears in no report is left exactly as it was — the report will say `not_run`
for it, which is true.

**The run measures; the recorder carries.** A recorder that stamped each result with the tree
it sees would vouch for a tree it did not see: CI records in a fresh checkout, and a local
run may be recorded after its checkout changed. So `evidence stamp` measures the checkout
as the run left it: the commit; the tree, with the evidence ledger ignored and the run's own
untracked outputs excluded (`--exclude` names a file or a directory, never a pattern, the
whole checkout or a path holding tracked files, so an exclude never hides a tracked change;
a report inside the checkout is one of those outputs); the producer, its toolchain, the
recorder's version, the report's digest, the host and the CI run. `evidence record
--provenance [<producer>=]<file>` carries that measurement into the report's executions.
When the measured tree was clean, the test's digest is taken from the commit, which holds
the bytes the run executed. A report recorded without a measurement carries the tree
`unknown`, which never reads `proven`. A local or release measurement must name its report
(`--report`), or it cannot say which run it measured. A stamp measures the checkout when it
is taken, after the report was written, and not the tree while the run executed: a run on a
dirty tree that is cleaned before the stamp reads clean.

Refusals, all of them deliberate:

- **nothing to record** — no `--suite`, `--crate-output` or `--coverage` was given;
- **not a git work tree with a commit** — a run recorded there would carry no provenance
  and prove nothing;
- **a malformed report line** — refused rather than skipped, because a silently dropped
  case leaves a claim reporting `not_run` after a run that did run it, which is a lie in
  the safe direction and still a lie;
- **a result for a test this checkout does not have** — named in the recording's `unknown`
  list rather than recorded or dropped, because it means the runner and the matrix have
  diverged;
- **a measurement of another commit** — the report ran somewhere else: record it in a
  checkout of the commit it ran;
- **a measurement of another report** — its digest is not the report's;
- **a measurement for a report not given, or keyed to another producer**;
- **a local or release measurement that names no report**;
- **a CI report without a measurement** — a CI recording carries the measurement its own
  job made. A job's tree file, which names no report, is accepted for a CI recording until
  CI stamps with its report.

Every recording is also a typed run. `evidence record` returns an `EvidenceRunRecord` (its
id, origin, commit, the weakest tree over the reports given, the measurement of each report,
totals by outcome and runner, what was absent, dropped or unknown, the executions) and
writes it with `--run-record <file>`. `--coverage <summary>` joins the summary
`scripts/rust-coverage --summary-json` wrote, bound to the commit and the tree its run
measured, with the floors the threshold files state; it is current only for its own commit,
and only on clean trees. `--ledger local` merges into `.ai/local/evidence/ledger.json`,
which the repository ignores: it changes no tracked file and moves no verdict. A run record
never decides a verdict; a later surface lets one weaken a verdict only through the one
monotone rule (`freshness::weakened_by`, which run records reach through
`evidence::weakened_by_records`), which can withhold `proven` and never grant it.

A crate binary states only what ran. `cargo test` prints `ok` on the result line of a
binary that ran nothing at all, so the recorder reads the counts, not only the word: a
binary with a failed test is a failure whatever its word says; one that passed nothing
(every test ignored, or none there) or ran only a subset a name filter chose is a skip, the
absence of a proof rather than a proof; one whose result line carries no count, or that
printed no result line before the next binary, is an error, because the harness could not
say what ran. Colour codes are stripped from every line before it is matched, so a coloured
run reads as a plain one. What the output held that no claim can name yet is listed, with
the reason, in the recording's `dropped` list (a `dropped` line in the text output) rather
than ignored: the crate's own unit-test binary (`Running unittests src/lib.rs`), its
doctests (`Doc-tests <crate>`), a binary outside `tests/`, and a `test result:` line with no
`Running` line before it, which is never credited to the previous binary.

### In CI

Half the wiring is already there. The `suite` job of
[`.github/workflows/validate.yml`](https://github.com/korczis/prismatic-majordomus/blob/master/.github/workflows/validate.yml) runs
`bash test/run.sh` with `MJ_TEST_REPORT: suite.tsv` and uploads that TSV as the
`ci-metrics-suite` artifact; the `macos` job does the same. The recording step is one line
after the suite step:

```yaml
- if: always()
  run: majordomus evidence record --suite suite.tsv --provenance suite=suite-tree.json --origin ci
```

What is **not** wired, and is a target rather than a description: nothing runs that step
today, and a ledger written in a CI checkout is thrown away with the runner. Landing CI's
evidence in the repository needs a commit, which a pull-request run must not make; the
shape that works is a push-to-master job that records and commits the ledger in its own
commit, the way the benchmark baseline is written. Until that exists, the ledger is written
by whoever runs the suite locally and commits it, and the states are honest about the
result either way.

## The capabilities

One declaration in
[`src/capability/builtin/evidence.rs`](https://github.com/korczis/prismatic-majordomus/blob/master/apps/majordomus-cli/src/capability/builtin/evidence.rs);
every surface is derived from it ([`CAPABILITIES.md`](@/docs/capabilities.md)).

<div class="overflow-x-auto" tabindex="0">

| capability | command line | HTTP | MCP |
|---|---|---|---|
| `evidence.report` | `majordomus evidence show` | `GET /api/v1/evidence` | tool `majordomus_evidence`, resource `majordomus://evidence` |
| `evidence.claim` | `majordomus evidence claim <id>` | `GET /api/v1/evidence/claim` | tool `majordomus_evidence_claim` |
| `evidence.test` | `majordomus evidence proves <id>` | `GET /api/v1/evidence/test` | tool `majordomus_evidence_test` |
| `evidence.record` | `majordomus evidence record` | — | — |
| `evidence.stamp` | `majordomus evidence stamp` | — | — |

</div>


`evidence show` takes `--state`, `--status` and `--findings` as filters and `--check`,
which exits `10` when any claim declares a guarantee the evidence does not support. The
tallies are computed **before** the filter: a filtered answer says how much of the matrix
it examined, never how much it returned.

None of them caches. The ledger is a file that changes outside the process, and a
cached answer would be exactly the stale evidence this subsystem exists to name.

**`evidence.record` is the only one that writes, and it is not reachable over the network.**
It declares a command line and nothing else. This executable's MCP and HTTP surfaces are
read-only — a property of the server, not an accident of what has been built — so the
recorder is a developer capability: offered to whoever runs the executable and to nobody
over a socket. It is declared as a capability rather than hand-written as a command so that
it stays inside the registry, the command graph and the generated reference. It is also
waived from the benchmark, with the reason recorded: timing it in a loop would rewrite the
repository's evidence.

`evidence.stamp` writes nothing, and it is command-line only too: it reads a report whose
path its caller names, and a network client must not choose a file this executable reads.

## Navigating in both directions

A relation that can only be walked one way is half a relation.

```sh
majordomus evidence claim distribution-canonical-model
majordomus evidence proves suite:84_distribution_model
majordomus evidence proves test/cases/84_distribution_model.sh   # the path resolves to the same test
```

`evidence claim` answers *what proves this claim*: the state and its meaning, the execution
behind it, the paths that changed since, the command that reproduces it, and the other
claims the same test proves — a test shared by twelve claims is a test whose failure is
twelve findings, and a reader of one should be able to see the other eleven without
searching.

`evidence proves` answers *what does this test prove*: its latest execution, whether its
source still hashes to what that execution recorded, the command that runs it again, and
every claim that names it with the state each is in.

Both read the same derivation, so the two directions cannot disagree. A claim the matrix
does not declare is a not-found rather than an empty answer: a typo that read as "this
claim has no evidence" is the one answer these capabilities must never give.

## The gate

[`scripts/evidence-check`](https://github.com/korczis/prismatic-majordomus/blob/master/scripts/evidence-check) renders the executable's own answer
and ratchets it. It derives nothing of its own — a gate with a second opinion about what is
proven would be a second model of proof.

```sh
scripts/evidence-check              # report; 0 clean, 10 with regressions
scripts/evidence-check --json       # the report, from the capability itself
scripts/evidence-check --strict     # every unsupported guarantee fails; the baseline is ignored
scripts/evidence-check --baseline   # rewrite the baseline from what is found now
scripts/evidence-check --repo PATH  # judge that repository instead of this one
```

Only `guaranteed` claims are judged. `advisory` states that enforcement is not observable
from outside, `planned` that nothing implements it, and `rejected` that nothing will;
demanding a current proof of those would be demanding proof of a thing the claim already
says is not there. A `guaranteed` claim is supported by `proven` or `inputs_unchanged`, and
by nothing else; `stale`, `failing`, `not_run`, `unrunnable` and `no_test` are findings,
each with the reason and the command that would settle it.

`stale` is the one of those that looks like support and is not, and it was counted as
support until it was measured: 14 guarantees rested on it, against 0 proven. A stale run
*is* a pass — `passing()` says so, and the summary must keep it apart from a failure — but
it is a pass of a subject that has since moved: the claim's implementation or its test
changed after the run offered as its proof, so what passed is not what the claim now names.
Accepting it made the gate accept, as support for a guarantee, a measurement of something
else, which is the whole defect this subsystem exists to name one level down. The floor is
`inputs_unchanged`: weaker than `proven`, but at least a pass of *this* subject.

**It is advisory today, and this is a choice with an end.** The ledger starts empty, so on
the day the gate arrives every guaranteed claim is `not_run` — true, and as a blocking gate
it would mean nothing could be merged by anybody until a full suite run had been recorded.
So the debt that existed at that moment is written to
[`.ai/repo/evidence-baseline.txt`](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/evidence-baseline.txt) by `--baseline`,
never by hand.

**What the ratchet refuses is not membership of that list.** It was, and that was a defect
rather than a strictness: a claim written after the baseline is absent from it for the only
reason a claim can be absent from a snapshot of the past — it did not exist yet — and the
gate reported every such claim as having "lost the evidence that supported them", evidence
it had never had. Every newly merged guarantee reddened the trunk for every branch that
merged after it. A ratchet whose alarm fires on growth rather than on regression measures
the wrong thing: it taxes writing claims down, which is what the matrix exists to reward.

So the baseline records the *state per claim* — both halves, a `+` line for a guarantee a
run supported and a bare line for one no run supported — and two things are refused:

<div class="overflow-x-auto" tabindex="0">

| refused | what it is | why membership could not see it |
|---|---|---|
| **lost** | a claim the baseline recorded as supported, and the evidence no longer supports | a claim losing its proof joins the unsupported set in exactly the way a newly written claim does |
| **withdrawn** | a supported guarantee that is no longer a guaranteed claim of the matrix at all | nothing became unproven; the denominator shrank, which is how a ratchet rots without ever going red |

</div>


And one thing is deliberately not refused: a guaranteed claim the baseline never knew,
arriving with no recorded run. That is new debt — reported, counted, and not a regression —
because refusing it is the broken behaviour above. It is admitted to the baseline by
`--baseline`, in its own commit, with the reason, and the ratchet holds it from then on.
The limit is stated rather than hidden: a claim promoted from `advisory` to `guaranteed`
without a run is new debt too, and is also only reported.

It still tightens in one direction only. The unsupported half shrinks by recording a run
that proves a claim and rewriting the file — never by adding a line. `--strict` refuses
every unsupported guarantee; it is the end state, and CI switches to it when the
unsupported half is empty.

The gate is named in [`.ai/repo/ci/gates.yaml`](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/ci/gates.yaml) as
`evidence-check`, in the `structure` job, with `always: true` — so the verdict arrives on
every push rather than when somebody remembers to run it, which is the other half of
`project.never-reported-is-not-green`. The `evidence` path class names what can change the
answer: the ledger, the baseline, the gate script, the claims matrix, the generated claim
pages and this document.

### `proven` is rare here, and that is the model working

The ledger is a tracked file, and recording writes it into the tree. So a recording made
at HEAD leaves the tree dirty, and committing the ledger moves HEAD past the commit the
executions name — so a rule that asked for a clean tree at HEAD would never be satisfied
here at all.

That is why the rule is the diff rather than HEAD, and why the ledger's own row is excluded
from it. `proven` is reachable in a repository that tracks its own evidence, which is the
only arrangement in which the evidence is worth anything to anybody but the person who ran
the tests. `test/cases/124_evidence.sh` proves it on a fixture whose ledger is tracked
exactly as this repository's is.

## Recorded in CI

The ledger above holds whatever was recorded into the tree. CI's runs are recorded as well, and
a validating run never commits its rows: they are kept where the run happened, as a run record.
A trunk run's rows are committed later, by a separate pull request (ADR 0080), because a run
cannot commit what it proved without adding a commit after the one it proved, on a trunk that
moves faster than the suite finishes. That recording pull request is not wired yet. The
decision is
[ADR 68](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/adrs/0068-ci-evidence-is-kept-where-the-run-happened-and-published-with-the-commit-it-proves.md),
as ADR 0087 amends it.

<pre class="mermaid">
flowchart LR
  suite["suite job&lt;br&gt;suite.tsv · suite-tree.json"]
  crate["rust job&lt;br&gt;cargo-test.txt · crate-tree.json"]
  cov["coverage job&lt;br&gt;coverage.json"]
  collect["evidence job&lt;br&gt;scripts/ci/evidence-collect"]
  artifact["artifact `evidence`&lt;br&gt;report · ledger · coverage · manifest"]
  pages["pages.yml&lt;br&gt;scripts/pages evidence"]
  site["/evidence/&lt;br&gt;current · stale · unknown · unavailable"]
  suite --&gt; collect
  crate --&gt; collect
  cov --&gt; collect
  collect --&gt; artifact --&gt; pages --&gt; site
  collect -. "commit still the tip: dispatch" .-&gt; pages
</pre>


**What the collector derives first.** Before it records anything, `scripts/ci/evidence-collect`
measures the checkout it runs in and derives the report through `majordomus evidence show`, as
`report.txt` and `report.json`. The reports it is handed and the directory it writes are under
the runner's temporary directory, outside that checkout. The report is therefore the tracked
ledger's verdict at the run's commit, the answer a clean checkout of that commit gives, and the
manifest's `report_tree` says whether the checkout was clean when it was derived.

**What the run's rows are.** Only then does it record the suite's and the crate's results
through `majordomus evidence record --origin ci`, and keep the ledger that now holds them as
`ledger.json`. Those rows are a run record. They are counted in the manifest and decide no
verdict, in the artifact or on the site. A trunk run's rows become verdicts only when ADR
0080's recording pull request lands them in the tracked ledger. The collector also summarises
the coverage export through `scripts/rust-coverage --summary-json`.

**What the producers and the recorder measure.** Each job that ran tests measures its own
checkout right after its run and hands the measurement over beside its report:
`suite-tree.json` from the suite job, `crate-tree.json` from the rust job. Each excludes by name
the outputs its own run names, at the paths where that run writes them, and nothing else: the
suite its report, at the root; the rust job its timings and its artifact directory, which
`scripts/rust-check` writes in the crate's directory, where it runs. The measurement lists what
it excluded. The recorder refuses a CI report without the measurement its own job made, so
the collector passes each job's tree file as that report's provenance, and each runner's rows
carry its own job's measurement (the manifest keeps the rows' trees as `rows_working_tree`).
The manifest's `totals`, its `working_tree` (the weakest tree over the recorded reports) and
the suite and crate words of `absent` are read from the recording's run record rather than
computed a second time; it is never read from the recorder's checkout, which is clean by
construction. A report whose job measured a commit other than the one the collector runs on,
or whose job left no measurement, is refused: it is not recorded, and it is named.

The collector writes `manifest.json`, naming:

- the commit that ran (for a pull request, GitHub's merge commit), and `head_sha`, the head it
  was built from, carried beside it and never in its place;
- the event and the run;
- the outcomes of that run's executions;
- the producers' measurements, the run record's `working_tree`, and the `report_tree`;
- which reports were absent, a report that yielded no execution included, and which were
  refused, with the reason.

An absent report is named, never counted as a pass. The job keeps the directory as the artifact
`evidence`. It is not a gate: it reads jobs that have already decided, and a red suite is
exactly the evidence worth keeping.

**An execution names its run.** A `ci` recording made inside a GitHub Actions environment stamps
every execution with `run`: the provider, the run's identifier, its attempt, the workflow, the
job and the address the provider gave. `RunRef::from_env` is the only place that knows the
environment's names. A local recording names no run, even inside a CI shell, and rows recorded
before runs were named have no `run` at all.

```json
"run": {
  "provider": "github_actions",
  "id": "<run id>",
  "attempt": 1,
  "workflow": "validate",
  "job": "evidence",
  "url": "https://github.com/korczis/prismatic-majordomus/actions/runs/<run id>/attempts/1"
}
```

**What a publication says about it.** Before the build, `scripts/pages evidence` finds the
`evidence` artifact recorded against the commit being published or its nearest ancestor. It
chooses the nearest ancestor by history, not the newest upload. It writes
`site/data/evidence.json`, which is never committed. It publishes the report as it was derived,
and reads the manifest only to decide whether the run confirms it: a reason can withhold
`current`, and never changes the report. ADR 0087 defines the states it says:

<div class="overflow-x-auto" tabindex="0">

| state | when | what the page says |
|---|---|---|
| current | the run is of the published commit, every job that ran tests measured a clean tree, the report was derived on a clean checkout and is carried, and nothing was absent, refused or failed | CURRENT, with the commit and the run |
| stale | the run is of an ancestor, or it is of the published commit and recorded a failure | STALE, with how many commits and changed files lie between, or with the failures |
| unknown | the run is of the published commit and cannot confirm the report: a tree not measured or not clean, a report absent or refused, or its own report not derived or not carried | UNKNOWN, with every reason |
| unavailable | no retained artifact of this history, or one that cannot be read | UNKNOWN, with the reason |

</div>


Whatever withholds `current` is listed under the sentence. None of the states stops a
publication. When a master run's evidence is kept while its commit is still the tip, the job
dispatches `pages.yml`, and that publication says current when the run confirms the report.
When master has moved on, the newer publication already carries the evidence as stale and the
newer run will refresh it.

```sh
scripts/pages evidence                          # this commit, fetched with gh
scripts/pages evidence --from <dir> --out FILE  # a gathered directory, offline
scripts/ci/evidence-collect --out <dir> --suite suite.tsv --suite-tree suite-tree.json \
  --crate-output cargo-test.txt --crate-tree crate-tree.json --coverage coverage.json
```

The behavioural proof is `test/cases/357_ci_records_evidence.sh`.

## What this is not

**It is not tamper-proof, and it does not pretend to be.** The ledger is a tracked file. A
person can edit `"outcome": "pass"` into it, and what a reviewer sees is the diff. What the
recorded digest buys is staleness detection that survives a revert — an execution whose
test source no longer hashes to the recorded digest did not run the test that is there now,
whatever the commit says. Cryptographic ceremony against someone who can already commit to
the repository would be ceremony with no threat model. The honest security property is the
one this repository relies on everywhere else: a fabricated result is a reviewable line in
a commit rather than an untraceable green.

Note the exact reach of that digest: it is reported by `evidence proves` as `digest_matches`
and is **not** an input to the proof state, which is decided by the git comparison. A tree
where the two disagree is a tree where something was recorded against a commit that does
not describe it, and the `working_tree: dirty` field is usually the reason.

**It is not the execution control plane.**
[`src/execution/`](https://github.com/korczis/prismatic-majordomus/tree/master/apps/majordomus-cli/src/execution) and
[`EXECUTIONS.md`](@/docs/executions.md) are about capability executions of a running server —
in-flight work, progress, typed events, gone with the process. This is about test runs:
durable, committed, and read long after the process that produced them exited. Two
different subjects that share an English word.

**It is not the obligation evidence of `lib/evidence.sh`.** That subsystem
([ADR 30](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/adrs/0030-a-task-owes-obligations-and-evidence-goes-stale.md)) is
about what a *task* owes before it may be called completed, and its ledger is the
append-only task ledger. The two share an idea — evidence that can go stale — and nothing
else. Neither reads the other's records.

**It does not judge results.** Nothing here re-runs a test, re-interprets an outcome, or
decides that a failure was environmental. The recorder records what the runner said.

**It does not make a claim true.** A claim with a `proven` state is a claim whose test
passed against this commit. Whether the test tests the claim is a question for review, and
`project.no-claim-without-test` is the rule that holds it.

## When something fails

<div class="overflow-x-auto" tabindex="0">

| message | meaning | remedy |
|---|---|---|
| `nothing to record: give --suite, --crate-output, or both` | the recorder was invoked with no input | name the report the run wrote |
| `this is not a git work tree with a commit, so a run recorded here would carry no provenance` | recording outside a repository with a commit | record in the checkout the run was made against |
| `line N of the run report has M field(s); it is name<TAB>result<TAB>seconds<TAB>phase` | a malformed TSV row; nothing is written | regenerate the report with `MJ_TEST_REPORT`, do not hand-edit it |
| `unknown  suite:99_ghost (no such test here)` | the run named a test this checkout does not have | the runner and the matrix have diverged; find out which moved |
| `… declares version N and this executable reads version 1` | a ledger of a shape this build does not know | use the executable of that generation, or migrate the file deliberately |
| `… is not a ledger this version can read: <serde error>` | the ledger is not valid JSON of this shape | fix or delete it; a deleted ledger reports `not_run`, which is honest |
| ``  `x` is not a claim of docs/CLAIMS.yaml `` | a claim id that does not exist | `majordomus evidence show` lists every id |
| ``  `x` names no test `` | an argument to `evidence proves` that is neither an identity nor a test path | give `suite:<case>`, `crate:<binary>`, or the path |
| `evidence-check: no majordomus executable` (exit 12) | the gate could not find a build | build the crate, or set `MAJORDOMUS_BIN` |
| `evidence-check: … is absent; run scripts/evidence-check --baseline` (exit 12) | the ratchet has no baseline | write one, in its own commit, with the reason |
| `evidence-check: N guaranteed claim(s) lost the evidence that supported them` (exit 10) | a claim the baseline recorded as supported no longer is | run the named test and record it, or fix what broke |
| `evidence-check: N supported guarantee(s) left the matrix` (exit 10) | a proven guarantee was deleted or renamed, so the repository proves fewer than the baseline records | restore the claim, or rewrite the baseline in its own commit saying why the guarantee went away |
| `evidence-check: N guaranteed claim(s) the baseline does not know` | new debt: a claim written after the baseline, with no recorded run — reported, never fatal | record a run, or `scripts/evidence-check --baseline` in its own commit |
| `evidence-check: N baseline line(s) now supported — tighten the baseline` | debt was paid and the list did not follow | `scripts/evidence-check --baseline`, in its own commit |

</div>


## Stability

<div class="overflow-x-auto" tabindex="0">

| | status |
|---|---|
| identity derived from a case path and a crate test path; a path no runner drives names no test | implemented, unit tests in `src/evidence/mod.rs` |
| only a recognised pass proves anything; an unrecognised outcome word is an error | implemented, unit tests in `src/evidence/mod.rs` |
| the seven states rank strongest to weakest, and `proven` and `inputs_unchanged` are never collapsed | implemented, unit tests in `src/evidence/mod.rs` |
| only `guaranteed` is held to its evidence | implemented, unit tests in `src/evidence/mod.rs` |
| merging one test leaves every other test's evidence untouched; the file is ordered and round-trips | implemented, unit tests in `src/evidence/ledger.rs` |
| a ledger of an unknown version or unreadable shape is refused, not reinterpreted | implemented, unit tests in `src/evidence/ledger.rs` |
| a missing ledger and an empty one are both "nothing recorded", not an error | implemented, unit tests in `src/evidence/ledger.rs` |
| a suite report becomes executions with provenance; a later partial run updates only what it ran | implemented, unit tests in `src/evidence/record.rs` |
| a malformed report, a run with no commit, and a result for a test that does not exist are each refused or named | implemented, unit tests in `src/evidence/record.rs` |
| the declaration yields exactly the projections it claims; only the recorder writes, and it is not on the network | implemented, unit tests in `src/capability/builtin/evidence.rs` |
| the whole path end to end in a fixture repository — nothing recorded, provenance, a partial run, `stale` with the path named, `proven` against `inputs_unchanged`, `failing` and the exit code, both directions, the refusals | behaviourally verified (`test/cases/124_evidence.sh`) |
| the gate planned on every push, and the paths that can change the answer | declared in `.ai/repo/ci/gates.yaml`; the planner that reads it is behaviourally verified (`test/cases/94_ci_plan.sh`) |
| CI recording its own runs, stamped with the run, into the `evidence` artifact of the commit | behaviourally verified (`test/cases/357_ci_records_evidence.sh`); see *Recorded in CI* |
| the site publishing that evidence as current, stale with its distance, or unavailable with its reason | behaviourally verified (`test/cases/357_ci_records_evidence.sh`); rendered on `/evidence/` |
| CI's evidence committed into the tracked ledger | refused — ADR 68: a run cannot commit what it proved without adding a commit after the one it proved |
| the site rendering a claim's proof state beside its status | not implemented; the guarantees page still shows the declared status alone |
| tamper resistance, a signed ledger, a run history, captured output | not implemented, and refused — see *What this is not* and ADR 40 |

</div>

{% endraw %}
