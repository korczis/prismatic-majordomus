# Continuous integration, reproduced locally. Every recipe here runs the same script CI
# runs; none of them is a second implementation of a gate.

# What CI would run for this working tree against master, from .ai/repo/ci/gates.yaml; extra arguments pass to scripts/ci-plan (--full, --base, --head, --files).
[group('ci')]
ci-plan *args:
    scripts/ci-plan --format text {{args}}

# The install command this project advertises, fetched and run from the published site: does a stranger who copies the line off the front page get a working tool right now? Pass --base URL to ask the same of a local fixture (docs/DISTRIBUTION.md).
[group('ci')]
install-check *args:
    scripts/ci/install-check {{args}}

# The structure gates the plan selects for this working tree, run by the dispatcher CI uses: the jobs structure and structure-b, which CI runs side by side (I4381), both run whatever the first one says; which gates those are is the model's (.ai/repo/ci/gates.yaml), not a list here.
[group('ci')]
ci-structure:
    rc=0; for job in structure structure-b; do scripts/ci/run-plan --job "$job" || rc=$?; done; exit "$rc"

# The gates the plan selects for this working tree, with the commands CI runs, in the plan's order.
[group('ci')]
ci-fast *args:
    scripts/ci/run-plan {{args}}

# Every gate, the on-demand ones included — a push leaves those to the nightly run, a person asking for everything means everything (the macOS gate only on macOS).
[group('ci')]
ci-full:
    scripts/ci/run-plan --full "just ci-full" --on-demand

# What GitHub observed of recent runs, recorded into .ai/repo/ci/baseline.json (needs gh); `just ci-baseline --table` renders it.
[group('ci')]
ci-baseline *args:
    scripts/ci-baseline {{args}}

# The distribution as a gate: the declarations are valid, and every artifact generated from them is current. A composition of two commands, which is why it is written here and not projected from the graph — the bridge carries `distribution-validate` and `generate-distribution` on their own.
[group('ci')]
distribution-check:
    @bin/majordomus-cli distribution validate
    @bin/majordomus-cli generate distribution --check
