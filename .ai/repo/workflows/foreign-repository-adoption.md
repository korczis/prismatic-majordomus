# Foreign repository adoption

A foreign repository is one that was not created for Majordomus or structured around it.
Adopting one is the product's first test outside its own walls, so the adoption has to be
admissible before it starts and measurable after it ends. This workflow says which
repository may be called foreign, what an adoption records and publishes, and with whose
consent. The milestone `a-foreign-repository-is-adopted-and-measured` uses it, and so does
anyone who adopts a repository and wants the result to count.

Nothing in this workflow is pushed to the adopted repository's remote, ever. The adoption
runs in a clone, and the record of it lives in this repository.

## 1. The floors come first, from the owner

The selection floors are the minimum number of tracked files, the minimum number of
commits, the languages that count, and whether an existing test command is required. They
are the owner's decision. They are recorded as a committed, accepted ADR before any
candidate is measured, and the worker who measures candidates never chooses them. If a
floor is chosen after the candidates are known, it fits the candidates. That is how any
repository could be called foreign afterwards.

The ordering can be checked. The commit that adds the floors ADR must be an ancestor of
the commit that adds the first candidate's measurements, and it must be older:

```bash
git merge-base --is-ancestor <floors-adr-commit> <first-measurement-commit> && echo pre-registered
```

Until that ADR exists, no candidate is admitted, however obvious it looks. Changing a floor
takes a new ADR that supersedes the old one, and every candidate is measured again against
it.

## 2. The selection measurements

Each measurement is a command run in a fresh clone at the commit that will be adopted
(`<clone>`, checked out at `<commit>`). The command's output is the value, and the floors
ADR is where the threshold comes from. A candidate is admitted only when every row passes.

| measurement | command | passes when |
|---|---|---|
| tracked files | `git -C <clone> ls-files \| wc -l` | at least the ADR's tracked-files floor |
| commits | `git -C <clone> rev-list --count <commit>` | at least the ADR's commits floor |
| primary language | `git -C <clone> ls-files \| sed -n 's/.*\.\([A-Za-z0-9]*\)$/\1/p' \| sort \| uniq -c \| sort -rn \| head -n 1` | the extension with the most tracked files is one the ADR lists |
| existing test command | the repository's own test entry point (a `test` script, target or task it already declares), run as `(cd <clone> && <its command>); echo $?` | the ADR requires one and it exits 0 at `<commit>`, or the ADR does not require one |
| no Majordomus path | `git -C <clone> ls-files -- .ai .majordomus \| wc -l` | 0; this is fixed, not a floor |
| no Majordomus region | `git -C <clone> grep -l -e 'majordomus:begin' <commit> -- . \| wc -l` | 0; this is fixed, not a floor |

A repository that fails the last two rows already carries this product and is not
foreign, whatever else it is.

## 3. The publication basis

Before the first step, decide which basis the record is published under. The basis
decides what the record may carry, and it never widens afterwards.

| basis | how it is established | the record may carry |
|---|---|---|
| licence | the adopted commit carries a licence file whose terms allow redistribution and analysis (`git -C <clone> ls-files \| grep -iE '^(LICEN[CS]E\|COPYING)'`), named by its SPDX identifier | the repository's public URL, the adopted commit, every measurement, every step time, and findings that name paths in it |
| owner consent | the repository's owner gave written, dated consent for this adoption to be recorded, kept outside the repository | what that consent names, and nothing past it; the record cites the consent's date and an opaque reference, never the consent's text or who gave it |
| none | neither of the above | an opaque candidate id, every measurement, every step time and finding counts, but no URL, no commit id, no path and no name |

When the basis is unclear, it is `none`. A record can still prove an adoption reached an
accepted finish without saying whose repository it was.

## 4. The steps and where their time is observed

The steps are those [`docs/ADOPTION.md`](../../../docs/ADOPTION.md) gives a new adopter,
in that order, with nothing added. Every step starts at the run clock read before its
command, because no step writes a line when it begins. It ends at the ledger line its
command writes, or at the run clock read after the command where the step writes none.
The table names that end source for each step, and the record keeps it as the step's
`source`.

| step | command | end time source |
|---|---|---|
| install | `curl -fsSL https://majordomus.dev/install.sh \| sh` | run clock |
| initialise | `majordomus init` | run clock |
| project the instructions | `majordomus update` | ledger: `projections.updated` |
| first diagnosis | `majordomus doctor` | run clock |
| add the hook lines `init` printed, diagnose again | edit, then `majordomus doctor` | run clock |
| first task | `majordomus start "<task>" --scope <paths>` | ledger: `task.started` |
| progress | `majordomus check` | run clock (`check` writes no ledger line, passing or refused; only `check --checkpoint` writes one, `task.checkpoint`) |
| finish | `majordomus finish --outcome completed --verify-command "<the repository's own test command>"` | ledger: `task.finished`, or `task.refused` when it refuses |

The run clock is UTC, read immediately before and after the command with
`date -u +%Y-%m-%dT%H:%M:%SZ`. A ledger time is the line's own `ts`, the envelope field
every ledger line carries (`share/events.yaml`), stamped in that same spelling to the
second; a ledger line has no `at`. A ledger step's end is, by definition, the moment its
effect was recorded, not the moment its process exited: `start` still reports overlap,
claims its scope on the mesh and names a prior handover after its line, and `finish`
releases its mesh claim after `task.finished`. Both clocks are UTC, so one adoption's steps
can be laid end to end.

An accepted finish is a `task.finished` event with outcome `completed`. A refusal is a
result, not a failure of the protocol: the record keeps it with its reason, and the
adoption may continue toward a later accepted finish.

## 5. What a record carries

One record per adoption, with these fields:

- `candidate`: the repository's public URL under the licence basis, otherwise an opaque id
- `adopted_commit`: under the licence or consent basis only
- `floors_adr`: the floors ADR and the commit that added it
- `measurements`: each row of section 2, with its command, value and verdict
- `admitted`: true only when every measurement passed
- `publication_basis`: `licence` (with its SPDX id), `consent` (with its date and opaque reference), or `none`
- `tool_version`: what `majordomus version` printed
- `platform`: what `uname -sm` printed
- `steps`: for each step of section 4, its name, `started_at` (always the run clock), `ended_at` and `source`, where `ended_at` was observed (`ledger:<event>` or `run-clock`)
- `reached_accepted_finish`: true or false
- `seconds_to_accepted_finish`: from the install step's start to the `ts` of the accepted `task.finished`; absent when no finish was accepted
- `refusals`: each refused `finish` with its reason, in order
- `findings`: each friction the adoption met, with an id and one sentence, naming a path only under the licence basis

`reached_accepted_finish` and `seconds_to_accepted_finish` are named exactly as the
outside-first-run record of I3129 names them. Both protocols answer the same question,
and a reader should never have to translate between them.

## 6. What is never recorded

- the content of any file of the adopted repository: code, configuration values, test
  output beyond pass, fail and counts;
- a secret of any kind: a token, a key, an environment value, a credential the adoption
  met;
- a person's name, email address or handle, commit authors included, and the text of a
  consent.

If a finding cannot be stated without one of these, state that it exists and what kind it
is, and leave it there.
