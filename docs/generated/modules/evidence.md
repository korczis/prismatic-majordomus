<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `evidence` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.16.0 -->
# Module `evidence` — Evidence

What actually ran, against which commit, and whether it still proves anything. The claims matrix binds a claim to a test by path; the ledger under .ai/repo/evidence records the latest execution of every test with the commit, the tree state, the digest of the test's own source, the time, the origin and the command that runs it again. Joining the two answers, per claim, whether the repository can honestly call it proven — and distinguishes a run recorded against this very commit from one whose inputs merely have not changed since, because collapsing those two is how a green badge stops meaning anything.

Stability: behaviorally_verified. Capabilities: 5.

## `evidence.claim` — What proves this claim

One claim with its proof state, the execution behind it — outcome, duration, commit, tree state, digest, time and origin — the command that reproduces it, and the other claims the same test proves. A claim the matrix does not declare is a not-found rather than an empty answer, because a typo that read as 'this claim has no evidence' is the one answer this capability must never give.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_evidence_claim` |
| HTTP | `GET /api/v1/evidence/claim` |
| CLI | `majordomus evidence claim` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::evidence |
| tags | evidence, claims, verification, provenance |

| input | type | required | description |
|---|---|---|---|
| `claim` | string | yes | The claim id, as `docs/CLAIMS.yaml` spells it. |

Output: `ClaimEvidence`.

## `evidence.record` — Record a run that happened

Reads what the runs already wrote — the suite's TSV report, cargo test's output, a coverage summary — carries into each result what that run measured about its own checkout (the commit and the tree, from the measurement `evidence stamp` wrote), with the digest of the test's own source, the time and the origin, and merges it into the ledger. It records; it decides nothing: a case that failed is a case the runner said failed, and a test no report named is left exactly as it was, so recording one case never erases the evidence for the rest. It refuses a tree with no commit, a measurement of another commit or of another report, a local measurement that names no report, and a CI report without a measurement; a local report recorded without one carries the tree unknown, which never reads proven. Every recording returns a run record (its commit, the tree each report's run measured, its totals and what was absent, dropped or unknown, and a coverage summary bound to its commit), and --ledger local writes only the ignored local ledger. A crate binary that ran no test, or only a filtered subset, is recorded as a skip, and what no claim can name yet (the crate's own unit tests, its doctests) is listed as dropped.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| CLI | `majordomus evidence record` |
| cache | — |
| benchmark | waived (destructive) |
| provenance | builtin majordomus_cli::capability::builtin::evidence |
| tags | evidence, tests, provenance |

| input | type | required | description |
|---|---|---|---|
| `suite` | string or null | no | The runner's TSV report: `MJ_TEST_REPORT=<file> bash test/run.sh`. |
| `crate_output` | string or null | no | A file holding `cargo test`'s output, for the crate's own integration tests. |
| `origin` | string or null | no | Where the run happened: `local` (the default), `ci` or `release`. |
| `provenance` | array | no | A measurement per report: the file `majordomus evidence stamp` wrote at the end of
that run, as `<file>` or `<producer>=<file>` (`suite`, `crate`, `coverage`). Without a
producer word, the producer is the file's own, else the only report given. |
| `coverage` | string or null | no | A coverage summary: what `scripts/rust-coverage --summary-json` wrote. |
| `ledger` | object | no | The ledger to merge into: `repo` (the default, the tracked ledger) or `local` (the
ignored one under `.ai/local/`, which moves no verdict). |
| `run_record` | string or null | no | Also write the run record to this file. |

Output: `RecordReport`.

## `evidence.report` — Every claim against the evidence recorded for it

The whole claims matrix joined to the ledger and judged at the presented revision: the working tree by default, or the checked-out commit as committed, read from the ledger that commit holds. Per claim: the proof state, the sentence explaining how that state was derived, why when the state alone does not say, the execution behind it, the files that changed since it, and the command that produces it again. A run on a commit the presented revision does not contain is stale, a test that declined to run is not run, and a run the working ledger holds that the presented commit's does not can only withhold proven. The tallies count the whole matrix even when the answer is filtered, and the findings name every claim that declares a guarantee the evidence does not support. Read fresh on every call: the ledger is a file that changes outside this process.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_evidence` |
| MCP resource | `majordomus://evidence` |
| HTTP | `GET /api/v1/evidence` |
| CLI | `majordomus evidence show` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::evidence |
| tags | evidence, claims, tests, verification, provenance |

| input | type | required | description |
|---|---|---|---|
| `state` | object | no | Only claims in this proof state (`proven`, `inputs_unchanged`, `stale`, `failing`,
`not_run`, `unrunnable`, `no_test`). Absent: every claim. |
| `status` | string or null | no | Only claims declaring this status (`guaranteed`, `advisory`, `planned`, `rejected`).
Absent: every claim. |
| `findings_only` | boolean | no | Only the claims whose declared status the evidence does not support, with the
findings. The tallies still count the whole matrix, so a filtered answer never
misreports how much of it was examined. |
| `presented` | string or null | no | Judge at this revision, as committed, rather than at the working tree: `HEAD`, or
any name of the checked-out commit. It must be the checked-out commit; the ledger is
read as that commit holds it, and a run the working ledger holds that the commit's
does not can only withhold `proven`. Absent: the working tree. |
| `presented_tree` | object | no | What the caller knows about the presented commit's tree (`clean`, `dirty`,
`unknown`). It can only weaken the measured state, never assert a clean tree over a
dirty checkout, and it means nothing without `presented`. |

Output: `EvidenceReport`.

## `evidence.stamp` — Measure the tree a run left behind

The one measurement a runner takes at the end of its run, for evidence record --provenance: the commit; the tree, ignoring the evidence ledger and the run's own untracked outputs, never a tracked change; the producer; its toolchain; the recorder's version; the report's digest; the host; the CI run. It writes nothing. It is offered only on the command line because it reads a path its caller names.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| CLI | `majordomus evidence stamp` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::evidence |
| tags | evidence, tests, provenance |

| input | type | required | description |
|---|---|---|---|
| `producer` | object | no | The producer whose run this is: `suite`, `crate` or `coverage`. |
| `report` | string or null | no | The report the run wrote; a local recording needs it named. |
| `exclude` | array | no | The run's own untracked outputs, as repository paths: a file, or a directory with
everything under it. A pattern, the whole checkout or a path holding tracked files is
refused. |

Output: `EvidenceProvenance`.

## `evidence.test` — What this test proves

One test — by identity or by the path a claim names it with — with its latest execution, whether its source still hashes to what that execution recorded, the command that runs it again, and every claim that names it. This is the reverse of evidence.claim and reads the same derivation, so the two directions cannot disagree.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_evidence_test` |
| HTTP | `GET /api/v1/evidence/test` |
| CLI | `majordomus evidence proves` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::evidence |
| tags | evidence, tests, verification, provenance |

| input | type | required | description |
|---|---|---|---|
| `test` | string | yes | `suite:84_distribution_model`, `crate:why`, or the path itself
(`test/cases/84_distribution_model.sh`), which is resolved to the same identity. |

Output: `TestEvidence`.

