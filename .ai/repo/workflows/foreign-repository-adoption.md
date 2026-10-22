# Adopting a foreign repository

A foreign adoption is the T1 trial of the proof plan: Majordomus installed from the
published installer into a repository that was not created or structured around it, carried
to an accepted finish by a real coding agent, and recorded whatever the outcome. This
workflow says which repository may be adopted, what each adoption records and publishes,
and with whose consent. It does not choose the repository: that is the owner's act.

The steps are the ones [`docs/ADOPTION.md`](../../../docs/ADOPTION.md) gives a stranger,
taken literally and timed. The first task has the shape of the use case
[`catch-an-agent-that-says-it-is-done`](../use-cases/catch-an-agent-that-says-it-is-done.md):
a claimed scope, a refusal, and an accepted finish only once the repository's own test
passes. The install conditions are the pinned ones of
[`docs/ADOPTION_FIRST_RUN.md`](../../../docs/ADOPTION_FIRST_RUN.md): the published installer,
a throwaway `HOME`, and no local build or checkout of this repository on the path.

## 1. The floors come first, and they are the owner's

No candidate is measured until the floors exist, and only a tracked record proves that they
did. The owner states them as a decision (`majordomus decision add`), but that store is
`.ai/local/state/decisions.md`: checkout-local, absent from every other clone and from CI,
and dated to the day, so it cannot show that the floors came before a candidate measured the
same day. The decision is therefore copied, with its text and its date, into the T1
protocol of the proof plan `proof-plan.yaml` under `.ai/repo/benchmarks/` (issues I3352 and
I3360), and that tracked copy is what counts. Git proves the order: the commit that
registers the floors is an ancestor of the first commit of the candidates file,
`candidates.yaml` in the T1 trials directory `.ai/repo/benchmarks/trials/t1-external-onboarding/`
(`git merge-base --is-ancestor <floors commit> <first candidates commit>` exits 0).
A worker who has already looked at candidates does not propose a floor, and this workflow
has no default: while the protocol carries no floors, measuring stops and nothing is
recorded.

Once the T1 protocol is registered, its declared fields and redaction classes are
canonical, and sections 3 and 5 of this page explain them. Where the two differ, the
protocol wins and this page is corrected in the same change.

## 2. Which repository may be adopted

A candidate is measured in a local clone at one commit, the adopted commit, written `<c>`
below. Each measurement is a command run there, and each is compared with the floor named
beside it. A candidate is admitted only when every row passes; otherwise it is recorded as
refused, with the row that failed. A candidate that passes and lacks a publication basis is
recorded as `awaiting_consent` and is not installed into. Refused and `awaiting_consent`
candidates are entries of `candidates.yaml`, never trial records: a trial record exists
only for an admitted candidate that was installed into.

| measurement | command, run in the clone | floor, and where it comes from |
|---|---|---|
| tracked files | `git ls-tree -r --name-only <c> \| wc -l` | at or above the protocol's tracked-files floor (the owner's decision) |
| commits | `git rev-list --count <c>` | at or above the protocol's commits floor (the owner's decision) |
| primary language | `git ls-tree -r --name-only <c> \| sed -n 's/.*\.\([A-Za-z0-9]*\)$/\1/p' \| grep -vixE 'md\|rst\|txt\|json\|ya?ml\|toml\|ini\|cfg\|lock\|csv\|xml\|svg\|png\|jpe?g\|gif\|ico\|pdf' \| sort \| uniq -c \| sort -rn \| head -1` | the source extension with the most tracked files names the language, documentation, data, configuration and image extensions excluded; the protocol's accepted languages (the owner's decision); at least one admitted candidate is neither Rust nor shell (issue I3362) |
| existing test command | `git grep -n -E 'test' <c> -- .github/workflows .gitlab-ci.yml .circleci Makefile justfile package.json pyproject.toml setup.cfg tox.ini noxfile.py go.mod Cargo.toml pom.xml build.gradle build.gradle.kts mix.exs Rakefile composer.json CMakeLists.txt`, then the command found, run in the clone, and its exit status | a command the repository already runs, cited by file and line at `<c>`, that exits 0 there (the intent criterion `the-product-proves-it-pays#a-foreign-repository-is-admitted-before-it-is-adopted`); no file found, or no command that exits 0, refuses the row, and no command is composed by the worker |
| no Majordomus path | `git ls-tree -r --name-only <c> \| grep -cE '^(\.ai\|\.majordomus)/'` | `0` (the same intent criterion); a top-level `.ai/` that belongs to another tool refuses the row too, because `init` would write into it |
| no Majordomus region | `git grep -l 'majordomus:begin' <c>` | no file printed (the same intent criterion) |
| a publication basis | section 3 | licence or owner consent; `none` admits the measurement and refuses the install |

The existing test command is someone else's code: run it only in the throwaway clone under
the throwaway `HOME`, with no credential in the environment.

One of the owner's own repositories that predates Majordomus may be a candidate only at the
parent of its first Majordomus commit, the first commit that adds a path under `.ai/` or
`.majordomus/` or a `majordomus:begin` marker:

```sh
git log --reverse --topo-order --format=%H -- .ai .majordomus | head -1
git log --reverse --topo-order --format=%H -S'majordomus:begin' | head -1
```

When both print a commit, the first Majordomus commit is the one that is an ancestor of the
other (`git merge-base --is-ancestor`); when neither is an ancestor of the other, the
repository is refused, because its history has no single point before Majordomus. `<c>` is
the first parent of that commit. A first Majordomus commit that is a root commit has no
parent, and the repository is refused. Such a repository counts as foreign only when the
owner has decided that it does. A measurement taken at any later commit is refused.

Two worked negative cases, as `candidates.yaml` would hold them:

- a candidate with 40 commits against a commits floor of 100 is `refused`, row `commits`,
  with the verdict `below floor`; under basis `none` the value 40 is not carried;
- a candidate that passes every measurement row, carries no licence file and has no
  written consent is `awaiting_consent`; it is not cloned again, installed into or timed
  until a basis is recorded, and then it is measured again at the same `<c>`.

## 3. The publication basis, and what each permits

Consent is the owner's, given in writing and dated, or a licence's. Neither is inferred
from a repository being public, from silence, or from a worker's judgement. The owner's
consent covers a repository only when the owner may grant it: a repository that belongs to
an employer, a client or another person needs that party's licence, and without one its
basis is `none`. The consent is carried in the candidate's tracked entry in
`candidates.yaml`, with its date and the fields it releases, in the same commit as or
before the run's first step; a consent kept only in the checkout-local decision store does
not establish the basis.

| basis | what establishes it | what a record may carry |
|---|---|---|
| `licence` | an SPDX licence at `<c>` that permits a local clone and the publication of measurements, named by its SPDX id: the file found by `git ls-tree --name-only <c> \| grep -iE '^(licen[cs]e\|copying)([.-].*)?$'`, every such file read when there are several (`LICENSE-MIT` and `LICENSE-APACHE`), or an `SPDX-License-Identifier` or package-manifest licence field when no file exists | the repository's public name and URL when the forge account holding it is an organisation's, the raw adopted commit, every measured value, the language, the test command's text, the paths Majordomus wrote, and friction notes in prose |
| `owner_consent` | the owner's dated written consent, carried in the candidate's tracked entry, naming the fields it releases | the fields the consent names and nothing beyond the `licence` row; a field the consent does not name is carried as under `none` |
| `none` | no licence and no consent | the trial's numbers and their classes, step sources, declared codes, path classes, and a salted digest of the adopted commit whose salt is never published; each selection measurement as its verdict against the floor, not its value; no name, URL, raw commit id, path, test command text or free text |

A licence permits the use of code, not the naming of a person. A repository held under a
person's forge account has that person's handle in its name and URL, so under `licence`
its name and URL are carried as under `none` unless that person's dated written consent,
carried in the candidate's entry, names them.

What no record carries, under any basis: the contents of any file of the adopted
repository, a secret or credential or environment value, the name, handle or address of
any person (author, committer, maintainer) beyond what the paragraph above admits, the
clone's absolute path, and any conversation or session identifier of a vendor.

## 4. The steps, and where each time is observed

Clone the candidate into a throwaway directory, check out `<c>`, and remove its remotes
(`git remote remove origin`) before anything else runs: nothing is ever pushed to the
adopted repository's remote, and no issue, pull request or message is sent to it as part of
the trial. Start the run clock, then follow `docs/ADOPTION.md` in its order:

| step | command | where its end is observed |
|---|---|---|
| install | `curl -fsSL https://majordomus.dev/install.sh \| sh` | run clock |
| init | `majordomus init` | run clock (init writes no ledger line) |
| update | `majordomus update` | the ledger's `projections.updated` |
| doctor | `majordomus doctor`, its exit status and `FAIL` count | run clock |
| hooks | the two hook lines `init` printed, the one `README.md` line naming `AGENTS.md` that `docs/ADOPTION.md` tells the reader to add for the first `FAIL`, then `majordomus doctor` again | run clock |
| commit | `.ai/repo/` and the generated files, committed in the clone only | run clock |
| first task started | `majordomus start "<task>" --scope <paths>` | the ledger's `task.started` |
| first refusal | `majordomus check` refusing a path outside the scope, or `majordomus finish` refusing a failing verification | `check`: run clock (a refused check writes no ledger line); `finish`: the ledger's `task.refused` |
| accepted finish | `majordomus finish --outcome completed --verify-command "<the repository's test command>"` | the ledger's `task.finished` whose `outcome` is `completed` and whose `verify.exit` is `0` |
| removal | delete `.ai/`, the generated projections and the two hook lines, as `docs/ADOPTION.md` says | run clock |

A step's elapsed seconds run from the end of the step before it (the run clock's start, for
install) to its own end. Both clocks are UTC wall time: the run clock is read as `date -u
+%Y-%m-%dT%H:%M:%SZ` when a step's command exits, and a ledger line carries the same form
in its `ts`, so the two can be subtracted. The `README.md` line belongs to the hooks step
because the page names it; it is neither a manual intervention nor a command typed beyond
the published site. Any other edit made to clear a `doctor` `FAIL` is a manual
intervention.

Removal deletes `.ai/`, and with it the ledger at `.ai/local/state/ledger.jsonl`. Before
the removal step, the recorder exports it to the throwaway directory, outside the clone,
with `majordomus history --json --all`, and reads every ledger time from that export. The
export is never published: its lines carry the checkout's branch and session identifiers,
and the record carries only the times and fields section 5 names.
Exporting is the trial's instrument, not an adoption step: it is neither timed as a step
nor counted as a command beyond the published site. Where the ledger has no line, the run
clock is the source and the record says so. A refusal the agent's own work did not produce
is induced the way the use case does it, by an edit outside the scope or a verification
that fails, and the record marks it `induced` rather than `observed`. Every command typed
that the steps above do not name, `capture install` included, is counted as a command
typed beyond the published site, with its time. Every manual intervention is counted. A
run that stops partway, an account limit included, is recorded as failed and never retried
silently.

## 5. What each adoption records

One trial record per run, under `.ai/repo/benchmarks/trials/t1-external-onboarding/`, in
the form the trial loader (issue I3353) admits. Every number is observed, counted or
derived, and carries that class beside it; an estimated or counterfactual number is
refused.

| field | holds | class of its numbers |
|---|---|---|
| `test` | `t1-external-onboarding` | none |
| `protocol_digest` | the digest of the T1 protocol the run followed | none |
| `recorded_at` | when the record was written, UTC | none |
| `candidate` | the entry in `candidates.yaml`, committed before the run's first step | none |
| `publication_basis` | `licence` with its SPDX id, `owner_consent` with its date, or `none` | none |
| `adopted_commit` | the raw commit, or its salted digest under `none` | none |
| `tool_version` | `majordomus --version` as installed | none |
| `platform` | `uname -sm` | none |
| `agent`, `agent_version` | the coding agent that did the first task | none |
| `steps` | each step of section 4: elapsed seconds, source (`ledger:<event>` or `run_clock`), exit status | elapsed seconds and exit status observed |
| `refusal` | `check` or `finish`, and `observed` or `induced` | none |
| `reached_accepted_finish` | whether the ledger holds a `task.finished` with `outcome` `completed` and `verify.exit` `0`; a `task.finished` with any other outcome is not an accepted finish | none |
| `seconds_to_accepted_finish` | from the run clock's start to that `task.finished`'s `ts`, the sum of the steps' elapsed seconds through the accepted finish | derived |
| `commands_beyond_docs` | the count of commands typed beyond the published site, each with its step | counted |
| `manual_interventions` | the count, each with its step | counted |
| `frictions` | each with an id and a declared code, tied to the step where it was met | none |
| `outcome` | one of the closed set below | none |

The outcome of a run is one of three, and each means one thing:

| outcome | when |
|---|---|
| `accepted` | every step of section 4 ran and `reached_accepted_finish` is true |
| `not_accepted` | every step ran through removal, but no `task.finished` met the accepted-finish condition |
| `failed` | the run stopped before removal: the installer failed, a step could not be completed, or the agent or its account stopped |

A refused candidate is not an outcome: it is an entry of `candidates.yaml`, and no trial
record is written for it.

`reached_accepted_finish` and `seconds_to_accepted_finish` are the names issue I3129 gives
the same facts for an outside person's first run, so the two forms do not diverge. The
record is committed and published whatever its outcome; a failed adoption is a row of the
scorecard, not an omission from it.
