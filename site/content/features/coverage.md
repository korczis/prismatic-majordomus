+++
title = "New code is covered, measured once and asked two questions"
description = "scripts/rust-coverage is the one coverage authority: a single cargo-llvm-cov export, test code excluded from both sides of every fraction, one rule for lines, functions and regions. Three questions read that judgment — a crate-wide line floor, the session/continuity domain held to 100%, and the changed-code differential gate that refuses new debt while leaving untouched legacy debt alone. The differential intersects the export with the lines git says a change touched against the branch's merge-base, working tree included, and distinguishes PASS, FAIL, UNKNOWN and BLOCKED so a crashed tool or an unresolvable base is never a silent green."
weight = 72
[extra]
id = "coverage"
status = "stable"
source = ".ai/repo/features/coverage.md"
+++
{% raw %}

## What it does

`scripts/rust-coverage` measures the Rust executable's coverage once — one instrumented
export, test code excluded from both the covered and the total so a test added never flatters
the number — and reports line, function and region coverage by one rule. Three gates read
that one judgment. The crate floor (`scripts/rust-coverage-threshold`) is a lower bound on
the whole executable. The session/continuity domain (`scripts/session-coverage-domain`) is
held to full line, function and region coverage. And the differential gate
(`scripts/ci/coverage-differential`, rule `project.new-code-is-covered`) holds every
executable line, function and region a change adds or touches — measured against the
branch's merge-base with the trunk, working tree included — so a change that introduces
uncovered code is refused even while the aggregate percentage barely moves.

## What it does not do

It never demands that untouched legacy debt be paid: the differential gate looks only at the
lines a change touched, so `legacy_debt_after <= legacy_debt_before` holds by construction
and `new_debt == 0` is what it enforces. A changed line that maps to nothing executable — a
comment, a `use`, a type declaration, test code — is not a finding. It measures the
dimensions `cargo-llvm-cov` gives this crate (line, function, region); it does not mutation-
test, so for a predicate whose sense matters it proves the line ran, not that a wrong line
would fail a test — those subsystems carry their own adversarial suites, and coverage is
necessary, not sufficient, for them.
{% endraw %}
