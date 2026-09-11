---
id: project.rust-coverage-honest
version: 1
kind: rule
title: The coverage number counts what the crate ships, and nothing that runs only because it is a test
description: Coverage of the Rust executable is measured over its production code alone — a function leaves the denominator only when it is declared inside a `#[cfg(test)]` span, read from the source, never because a path or a name was listed — and the accepted debt is a per-file count that can shrink and cannot grow.
statement: A coverage figure for this crate is computed over the code the crate ships; a function is excluded from the denominator only because it is declared inside a `#[cfg(test)]` item, not because somebody listed its path or recognised its name; and no file may carry more uncovered production lines than the recorded baseline accepts.
status: active
class: blocking
depends_on: [project.rust-command-tested-in-file@1, project.no-claim-without-test@1]
tags: [rust, testing, coverage, measurement]
---

# Rationale

The gate that existed before this rule ran

    cargo llvm-cov --all-targets --summary-only --fail-under-lines 90

and that measurement cannot answer the question it appears to answer. `--all-targets`
instruments the library, the binary, every integration test and every benchmark, and no
`--ignore-filename-regex` narrowed it. Test code is covered by construction — running is
what it is for — so it sat in the numerator and the denominator together. When this rule
landed the crate held 81,850 production lines under `src/`, 14,703 lines of in-file
`#[cfg(test)]` modules beside them, and 18,863 more under `tests/` and `benches/`: a third
of the denominator was code that could not fail to be covered.

The problem is not that the figure was flattering. It is that **the figure moved the right
way for the wrong reason**. Adding a test raised it whether or not the test covered a single
production line, and a change that covered real code could be swamped in either direction.
A floor computed over a denominator that the act of testing inflates is worse than no floor,
because a reader takes it for a guarantee. The number was 87.81% against a threshold of 90
for at least six merges before anybody noticed, and nobody could have said from the number
whether that was a testing problem or an arithmetic one.

The second half of this rule is about how the exclusion is decided, and it is the half that
will be under pressure later. The obvious implementation is a list of paths the measurement
skips. This rule forbids that, for the reason `project.rust-public-api-quality` gives about
exemption files: a list of excluded paths is a place to add a line, and the line that gets
added is never a test file. So a function leaves the denominator only when it **is** test
code, and that is read from the declaration: its file is under `tests/` or `benches/`, or it
is declared inside a `#[cfg(test)]` item whose exact line span is scanned out of the source,
or inside a file that a `#[cfg(test)] mod x;` pulls in whole. There is no ignore list, and
deliberately nowhere to put one.

The obvious cheaper implementation — recognise a `tests::` segment in the symbol name — was
tried first and is wrong twice over, which is worth recording so that nobody re-introduces
it as a simplification. It does not work at all: llvm-cov's export carries **v0-mangled**
symbols, in which the module path appears as `5tests` inside a longer encoding and `::tests::`
never appears, so a name rule silently classified **nothing** as test code and quietly
inflated the denominator by every in-file test in the crate — 1,046 functions. And it would
be wrong even after demangling, because this crate has a *production* module called `tests`:
`web::report::tests` models the `/tests/` page, and a name rule would have dropped it out of
the denominator, hiding real uncovered production code behind an exclusion nobody wrote down.
That is the failure this rule exists to prevent, arrived at from the other direction.

The span is evidence. The name is a guess that happens to be right most of the time, and the
two places it is wrong here are both silent.

Three things this repository measures do not agree with each other, and the rule states it
rather than hiding it. A doc example satisfies `project.rust-public-api-quality` and is
invisible here, because `cargo llvm-cov --all-targets` does not instrument doc tests
(`--doctests` is a separate flag its own author marks unstable). A `test/cases/*.sh` case
exercises a command end to end through the real binary and is also invisible here, because
the profile is not collected from that child process. So an item can be fully proved by two
other gates and still read as dead code to this one. The answer is not to weaken this
measurement to agree with them: it is to know that this gate answers exactly one question —
*is this line reached by something the instrumented build ran* — and to read the other two
gates for the rest.

# Required behaviour

Coverage of `apps/majordomus-cli` is measured over its production functions: those whose
file is under the crate's `src/` and whose demangled path carries no test-module segment.
Line, function and region coverage are all reported, because a line executed once does not
prove the branches on it.

No file is excluded from the denominator by path, and no configuration exists in which one
can be. The classification does not depend on what an in-file test module is called, so a
module may be named whatever reads best beside the code it tests; what it depends on is that
test code carries `#[cfg(test)]`, which the compiler already requires of it.

The accepted debt is recorded in `.ai/repo/rust-coverage-baseline.txt` as one line per file:
the number of uncovered production lines that file may carry, and the path. A **count**, not
a set of line numbers, so that editing a comment above an uncovered branch is not a finding
and reformatting a file is not a regression.

A measurement is taken with `--no-fail-fast`. A coverage verdict must not depend on the
suite being green: a single failing unit test otherwise aborts the run before the
integration binaries execute, and no report is produced at all, which is what `origin/master`
did on the day this rule was written.

# Failure behaviour

`scripts/ci/rust-coverage-check` reports the three percentages and the per-file debt, and
exits `10` when a file carries more uncovered production lines than the baseline accepts, or
when a file the baseline does not name carries any at all — the code every other unmet
contract in this repository uses. A file that improved is reported and never failed, with
the command that records the improvement.

`--strict` ignores the baseline and fails on any uncovered production line. That is the
target state and the way to see the whole remaining gap at once; the rule is not finished
until the baseline file is empty and removed, because an empty exemption list invites
somebody to add a line to it.

`--write-baseline` is the only thing that writes the baseline, and it is a deliberate act.
Exit `12` means the measurement could not be taken — no profiles, no `cargo-llvm-cov`, no
`python3` — and is never reported as coverage.

This gate does not replace the existing `--fail-under-lines` invocation in the `rust` job
and does not change it. The two numbers measure different denominators; this one is added
beside it so that the older figure keeps meaning what it has always meant and no verdict
moves under anybody while the honest number is still new.

The validator does not live in `lib/`. That library belongs to the shipped tool, and a
project rule that needs a check gets a gate — the argument
`project.rust-command-tested-in-file` makes, which this rule follows.

# Verification

`test/cases/127_rust_coverage_honest.sh`, which builds a fixture coverage export and a
fixture source file carrying a real `#[cfg(test)]` span at a known line, rather than
compiling a crate, and asserts the classification and the ratchet in both directions: that a
function declared inside the span is not counted, that a function under `tests/` is not
counted, that another crate's function is not counted, that the two production functions
are, that a rise above the accepted count fails with exit 10 naming the file and both counts,
that a fall is reported and passes, that `--strict` ignores the baseline in both directions,
and that a missing baseline or an export with no data is unusable rather than clean.

`scripts/ci/rust-coverage-check --strict` against this crate, which prints the whole
remaining gap and is the number the rule is finished at.
