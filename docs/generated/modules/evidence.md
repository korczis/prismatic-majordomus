<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `evidence` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.1 -->
# Module `evidence` — Evidence

What actually ran, against which commit, and whether it still proves anything. The claims matrix binds a claim to a test by path; the ledger under .ai/repo/evidence records the latest execution of every test with the commit, the tree state, the digest of the test's own source, the time, the origin and the command that runs it again. Joining the two answers, per claim, whether the repository can honestly call it proven — and distinguishes a run recorded against this very commit from one whose inputs merely have not changed since, because collapsing those two is how a green badge stops meaning anything.

Stability: behaviorally_verified. Capabilities: 4.

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

Reads what the runs already wrote — the suite's TSV report, cargo test's output — stamps each result with the provenance the run itself did not carry (the commit, the tree state, the digest of the test's own source, the time, the origin) and merges it into the ledger. It records; it decides nothing: a case that failed is a case the runner said failed, and a test no report named is left exactly as it was, so recording one case never erases the evidence for the rest. A tree with no commit to name is refused, because an execution with no commit proves nothing.

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

Output: `RecordReport`.

## `evidence.report` — Every claim against the evidence recorded for it

The whole claims matrix joined to the ledger: per claim, the proof state, the sentence explaining how that state was derived, the execution behind it, the files that changed since it, and the command that produces it again. The tallies count the whole matrix even when the answer is filtered, and the findings name every claim that declares a guarantee the evidence does not support. Read fresh on every call: the ledger is a file that changes outside this process.

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

Output: `EvidenceReport`.

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

