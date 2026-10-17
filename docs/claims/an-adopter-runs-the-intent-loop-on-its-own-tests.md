# A repository that adopted the tool runs the intent loop on its own tests

## What it means

A repository fresh from `init`, keeping its tests in its own layout, declares where they are in its policy, and from then on an intent's `test` criterion naming one of them resolves, a recorded run of it meets the criterion, and every surface prints the declared command to run it again. Without a declaration the tool's own layout applies, exactly as before.

## How it works

`evidence.tests` declares, per runner kind, a root, the suffix test files end in and the argv that runs one test (`{name}` or `{path}` standing for it). One resolver maps a path to a test, a test to its source and to its reproduce line, for the intent engine and the recorder alike. A path no root covers is refused by `intent validate`, naming the layout in force and the key; a declaration that names a runner twice, climbs out of the repository or gives no argv refuses the recording. Prefixed references resolve by their prefix first, so ADR 0119's command and deployment runners join the same vocabulary.

## How to see it

```bash
majordomus-cli intent validate        # FAIL unresolved_evidence_ref ... declare where its tests are under evidence.tests
# declare evidence.tests in .ai/repo/policy.yaml, then:
majordomus-cli evidence stamp --producer suite --report suite.tsv --out p.json
majordomus-cli evidence record --suite suite.tsv --provenance p.json
majordomus-cli intent show <id> --format json | jq '.verdict.state'   # satisfied
```

## What it does not cover

One flat root per runner: tests of one kind in two directories, or nested layouts, are not expressible yet. Command and deployment evidence are ADR 0119's.

## Why it exists

The evidence engine knew only the tool's own two test directories, so no repository that adopted the tool could ever have an intent satisfied by its own tests. `.ai/repo/adrs/0121-a-test-criterion-names-a-test-where-the-repository-keeps-its-tests.md` makes the layout the repository's to declare.
