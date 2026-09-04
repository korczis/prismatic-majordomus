# What the tool knows about itself is written once and derived everywhere else

## What it means

A number, a list, or a vocabulary that appears in two places will disagree in one of them, and the disagreement is silent until it costs someone an afternoon. So each of these facts has exactly one home, every other reader computes it, and a second copy fails a test.

Four kinds of fact are covered today: the values in the policy, the set of statuses a claim may carry, the inputs the site generator reads, and the sets of commands, doctrines, claims and vocabulary terms the tool declares about itself.

## How it works

**Policy values have no reader-side defaults.** `mj_pol_req` returns a value or refuses, naming the missing key. There is deliberately no `|| cap=200` beside the reader: that is a second source of truth for the same number, nothing keeps the two in step, and a reader that substitutes its own value enforces something the configuration does not say. The `policy_completeness` doctrine derives the key list by scanning `lib/` for `mj_pol_req` calls, so a key joins the check by being read.

The check compares full key paths rather than last segments, which is not fussiness: `checkpoint.retention_max_files` and `handover.retention_max_files` are different keys with different values, and a comparison on the tail reports a drift between two settings that are each correct.

**Claim statuses are data.** `docs/CLAIMS.yaml` declares them under `statuses:`, in the order the site presents them. They used to appear in a comment in that file, in a JSON literal inside the generator, in two loops beside it, and in `site-check` — five copies of one closed vocabulary.

**Fixtures derive their inputs.** `scripts/generate-site-data --inputs` prints the canonical inputs it reads, and `fixture_repo` in `test/lib.sh` builds a test repository from that list. Six fixtures used to carry their own copy list; when the generator gained a new canonical input, all six went stale at once and five cases failed with "canonical input missing" on a repository that had the file.

**The self-description checks enumerate nothing.** `test/cases/28_no_hardcoded_values.sh` reads the command list from the dispatch table, the doctrines from the registry, the claims from the matrix, and the vocabulary from the concepts table. A new command joins the "must be dispatched, in usage, documented in `docs/CLI.md`, and invoked by some case" check by existing.

## How to see it

```bash
# a policy key the code reads but the policy does not declare
sed -i '' '/builder_budget_lines/d' .majordomus/policy.yaml
majordomus context
# majordomus: policy is missing required key 'context.builder_budget_lines'

majordomus doctor | grep 'policy '
# ok  policy  11 key(s) — every policy value the code reads is declared, with no reader-side default

scripts/generate-site-data --inputs | wc -l     # what the fixtures copy
bash test/run.sh 28_no_hardcoded_values
```

## What it does not cover

It covers facts the tool states about itself, not every number anywhere. A magnitude in a design document, a measurement of some other repository, or a figure in an example of command output is prose and stays prose.

The vocabulary check proves a term in the concepts table appears somewhere else in the repository. It cannot prove the term is used to mean the same thing in both places.

It does not detect a fact duplicated between two files that both look canonical. The rule it enforces is "derive, do not repeat", and it can only see the repetitions it knows how to look for; each new one is a new assertion.

## Why it exists

Every failure this repository has produced has the same shape. Enforcement declared in a policy and wired to nothing. A projection whose hash nobody compared. A store written and never read. Profile toggles that no code consulted. A commit reported as green that existed only in one working copy. Six test fixtures with six copies of one list.

The tool exists to catch declared-but-not-wired, so a declared-but-not-derived list inside the tool is the same bug wearing the tool's own clothes. The check that finds it has to enumerate nothing itself, or it becomes the next copy.

There is a second rule underneath, and it is the sharper of the two: **an empty set is an error, not a result.** Every instance of this failure produced a plausible answer instead of stopping. A `grep -c` counted a shape and returned a number that was wrong for a reason nobody could see. A key comparison on last segments reported a conflict between two settings that were each correct. A claims-to-responsibility regex with a `(?!)` default matched nothing by construction and published a page with no owner rather than failing. The test runner printed `0 passed, 0 failed` and exited zero when its filter matched no case, so selecting a test that did not exist read as success. A generated document named the commit it was written at, which is the parent of the commit that contains it, so it was one behind forever and no regeneration fixed it.

None of them failed. Each returned something that looked like an answer, which is why they survived long enough to be found together. A check that cannot distinguish "nothing matched" from "nothing was wrong" is not a check, and any list this tool derives has to treat an empty derivation as a fault in the derivation.
