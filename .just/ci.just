# Continuous integration, reproduced locally. Every recipe here runs the same script CI
# runs; none of them is a second implementation of a gate.

# What CI would run for this working tree against master, from .ai/repo/ci/gates.yaml; extra arguments pass to scripts/ci-plan (--full, --base, --head, --files).
[group('ci')]
ci-plan *args:
    scripts/ci-plan --format text {{args}}

# The gates every plan runs: shell syntax and shellcheck, then doctor, watch, the context documents, the continuity commands, plan validate, the offline GitHub projection and the derived site data.
[group('ci')]
ci-structure:
    scripts/ci/shell-lint
    scripts/ci/core-check

# The gates the plan selects for this working tree, with the commands CI runs, in the plan's order.
[group('ci')]
ci-fast *args:
    scripts/ci/run-plan {{args}}

# Every gate, as a push to master runs them (the macOS gate only on macOS).
[group('ci')]
ci-full:
    scripts/ci/run-plan --full "just ci-full"

# What GitHub observed of recent runs, recorded into .ai/repo/ci/baseline.json (needs gh); `just ci-baseline --table` renders it.
[group('ci')]
ci-baseline *args:
    scripts/ci-baseline {{args}}

# The install command this project advertises, fetched and run from the published site: does a stranger who copies the line off the front page get a working tool right now? Pass --base URL to ask the same of a local fixture (docs/DISTRIBUTION.md).
[group('ci')]
install-check *args:
    scripts/ci/install-check {{args}}

# The distribution as a gate: the declarations are valid, and every artifact generated from them is current. A composition of two commands, which is why it is written here and not projected from the graph — the bridge carries `distribution-validate` and `generate-distribution` on their own.
[group('ci')]
distribution-check:
    @bin/majordomus-cli distribution validate
    @bin/majordomus-cli generate distribution --check
