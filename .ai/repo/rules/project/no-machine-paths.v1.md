---
id: project.no-machine-paths
version: 1
kind: rule
title: Nothing this repository commits names a path of the machine it was written on
description: No committed file records an absolute path of somebody's home directory or working copy, whether a person wrote it or the generator did; a path a generated artifact records is repository-relative, and the helper that computes one is defined once in the crate.
statement: A committed file names a path relative to the repository or not at all; a generated artifact that records where it came from records it repository-relative; and the function that turns an absolute path into a repository-relative one is defined once.
status: active
class: blocking
depends_on: [project.derived-files-regenerated@1, project.generated-artifacts-are-typed@1]
tags: [derived, generation, portability, reproducibility]

x-majordomus:
  tests: [test/cases/58_home_path_gate.sh, scripts/ci/no-machine-paths]
---

# Rationale

A repository that names the machine it was written on is a repository that does not read
the same anywhere else. This one paid for that twice in a single afternoon, in two
different ways, and the second is the instructive one.

The first was an authored document. The reasoning of `I0908` — an issue whose own stated
purpose is that "no absolute developer path exists anywhere in the image" — named a home
directory in prose. It reached `plan.json`, reached the site, and turned master's deploy
red at the moment of publication, billing whoever was deploying rather than whoever wrote
it.

The second was a generated artifact. `share/allow/*.txt` and `share/sections/*.txt` were
committed naming their source schema by the absolute path of the worktree the generator
last ran in. The cause was a path helper whose fallback was to emit the absolute path:

    path.strip_prefix(root).unwrap_or_else(|_| path.display().to_string())

`Share::open` canonicalises the share directory; the repository root arrives as it was
discovered. When the two spellings differ — one symlinked or firmlinked component is
enough, and the macOS temporary directory is exactly that — `strip_prefix` fails and the
fallback writes the generator's own location into a file that is then committed.

Two things let it survive. The helper existed **twice**, as `generate::relative_to` and as
`metadata::rel_or_abs`, so fixing one left the other writing the same leak — a repeated
semantic definition across projections, which this repository already calls a design defect
([ADR 0004](../../adrs/0004-canonical-architecture-and-performance-truth.md)). And
`derive-check` is structurally blind to this class: it regenerates in the same checkout and
gets the same answer back, so the tree looks current. The banner disagrees only for the
next person, in another worktree, where it appears as unexplained drift in a file nobody
edited.

# Required behaviour

- No tracked file contains an absolute path naming a home directory or a developer's
  checkout. Write what the path *means* — "a developer's working directory" — not an
  example of one.
- The rule is one check over the whole tracked tree, not a list of directories. Two
  pathspec allowlists were tried first and left `site/content/` — generated, committed,
  published — covered by neither.
- A path a generated artifact records is repository-relative whenever the file it names is
  inside the repository. A share directory genuinely outside the repository, supplied by
  `--share` or `MAJORDOMUS_SHARE`, keeps its absolute path: that is the only honest answer
  when no relative one exists.
- The function that computes a repository-relative path is defined once in the crate. A
  second copy is a second fallback, and the second fallback is what leaked after the first
  was fixed.

- A home directory is any `/Users/<name>` or `/home/<name>`, where the name is drawn from the
  POSIX portable filename set — either case, digits, dot, underscore, hyphen. A placeholder
  in an example or a doc comment is spelled as a path no account owns (`/srv/repo`): no
  reader can tell a placeholder shaped like a home directory from a real one, so it is
  reworded where it is declared rather than exempted where it is read.
- A reader of this property that could not examine the whole of its subject says what it
  did not examine and fails. "Found nothing" and "could not look" are different answers,
  and only one of them is a pass (`project.a-verdict-states-its-subject`).

One exception, narrow and necessary. The fixtures under `test/` name such a path
deliberately, because a gate that cannot be shown to fail is decoration. The gate scripts
are not exempt: the pattern is written so that it cannot match its own spelling, and the
prose that explains it — `/Users/<name>/`, a bracket expression, `$HOME` — puts no account
character straight after the slash.

# Failure behaviour

`scripts/ci/no-machine-paths` exits 1 and prints every match as `file:line:match`, under a
message that names both faults and their different fixes: an authored document is a wording
problem, a generated artifact is a generator problem, and the second is never fixed by
editing the file. It exits 12 when it could not examine its whole subject — no repository,
no directory, nothing in it, or a read that failed — naming what it did not examine; and on
a pass it says how many files it examined. `scripts/ci/core-check` runs it over the tracked
tree as a blocking step, so it fails a commit, a push and CI alike.

`scripts/site-check` runs the same script with `--dir` over the built site for its `private`
check. That check stays: it is the last line, it catches whatever reaches publication by a
route nobody anticipated, and nothing else reads the build output. It carries no pattern of
its own, and a result of 12 there is a failure saying the output was not examined — never
`OK`.

# Verification

`test/cases/58_home_path_gate.sh` runs `scripts/ci/no-machine-paths` itself rather than
re-implementing its grep, and proves each half over a fixture repository: an authored
document carrying a home directory fails and its reworded form passes; a generated banner
naming its schema by an absolute path fails and its repository-relative form passes; and a
file under `site/content/` fails, which is the case the two pathspec allowlists missed. One
table of account names that must be caught and of prose that must not is driven through
both the tracked tree and `--dir`. The refusals are each shown: an absent directory, an
empty one, a directory `find` cannot walk, a file `grep` cannot open, a path that is no
repository and a repository that tracks nothing all exit 12. It also asserts the gate is
wired into `scripts/ci/core-check` and that `scripts/site-check` reads through it.

`apps/majordomus-cli/src/generate.rs` holds the single helper and its two unit tests: one
resolves a share inside the repository reached by a differently-spelled root, and fails on
the previous implementation with an absolute temporary path where a repository-relative one
was expected; the other holds the external-share case, so the fix cannot quietly turn an
honest absolute path relative.
