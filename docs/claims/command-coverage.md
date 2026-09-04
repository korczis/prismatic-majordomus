# Every public command has a behavioural test and a negative test, computed rather than remembered

## What it means

For each command in `share/commands.yaml` marked public, the suite must contain a case that
exercises its normal path and a case that asserts a named failure mode of it. The suite must
also carry one workflow that runs a task to an accepted finish and one that violates the
contract, is refused, is repaired and is then accepted.

Nothing about this is written down per command. The requirement is computed from the
registry, so a command added tomorrow is owed coverage from that moment and the maintainer
is told which layer is missing rather than discovering it later.

## How it works

`test/cases/31_command_coverage.sh` reads the public surface from the registry and the
coverage from metadata each case declares about itself, in three comment headers:

```
# majordomus-covers: handover start       the normal path
# majordomus-negative: handover           a named failure mode
# majordomus-lifecycle: refused           a multi-command workflow
```

Coverage is declared rather than inferred because it cannot be read reliably from the
source. The negative assertions for `handover` and `checkpoint` are written as
`bash -c "... | '$MJ' cmd"` because those commands read stdin, and the surface case asserts
a failure for every command through a loop variable. Both are invisible to any scan, and a
filename tells you less than either. A header naming a command that does not exist is a
broken reference and fails.

Every case in this suite is end to end by construction: `test/run.sh` creates a fresh git
repository in a temporary directory and invokes the real binary inside it. There is no
separate end-to-end layer to declare and no second runner to keep in step.

## How to see it

```bash
bash test/run.sh 31_command_coverage      # prints the matrix it computed
grep -rn 'majordomus-covers' test/cases/
```

## What it does not cover

It counts layers, not quality. A case that declares a negative for a command satisfies the
requirement whatever it asserts, and nothing here measures how much of a command's
behaviour a case reaches. What it prevents is a public command with no case at all, which
is the failure that actually happens.

## Why it exists

Coverage that a maintainer has to remember is coverage that lapses at the first busy week.
The rule "also add a test" is exactly the kind of secondary registration step this
repository refuses to rely on anyone remembering.
