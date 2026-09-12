+++
title = "A generator that cannot match the tree refuses rather than rewriting it"
description = "A generator that cannot match the tree refuses rather than rewriting it"
weight = 58
[extra]
kind = "rule"
slug = "project-a-tool-that-cannot-match-the-tree-refuses-1"
identity = "project.a-tool-that-cannot-match-the-tree-refuses@1"
status = "active"
source = ".ai/repo/rules/project/a-tool-that-cannot-match-the-tree-refuses.v1.md"
+++
{% raw %}

## Rationale

On 2026-09-10 an executable built from an earlier revision of this crate was pointed at a
current tree through `MAJORDOMUS_BIN`. It reported artifacts stale on a tree that was in fact
current, and printed the remedy `run: majordomus generate`. Running it removed some sixteen
thousand lines across ten documents — most of them the changelog's — rewriting each from a
model the tree no longer holds. **It exited zero and said nothing was wrong.** It was caught because one worker read `git diff --stat` before committing, and it
would otherwise have landed looking like an ordinary regeneration.

Two things made it silent, and both are worth naming because neither is unusual.

**A version string is not a build.** The guard that existed compared the executable's own
version against the version the crate manifest declares. Both said the same number, because
the number changes when a release is cut and not when a declaration moves. Two executables can
agree about their version and disagree about every capability they project, and it is the
second disagreement that rewrites documents.

**The verdict blamed the wrong side.** "This artifact is stale" is a statement about the tree.
When the tool is the stale party the sentence is false, and it is false in the most expensive
direction available: it tells the reader that the repair is to regenerate, and regenerating is
the act that destroys. A check that can be wrong about which of two things is out of date must
say which one it decided, so that a reader can disbelieve it.

The habit that caused it is not going away, and should not: a worker who borrows a built
executable instead of spending ten minutes rebuilding one is right to, and several sessions a
day do it. What has to change is that borrowing an unsuitable one is loud.

## Required behaviour

An executable asked to generate or to check this repository's derived artifacts first
establishes that it was built from the same declarations the tree carries. When it cannot, it
refuses: it writes nothing, names both what it is and what the tree expects, names the remedy,
and exits non-zero.

Three uses stay working, and a mechanism that breaks any of them is the wrong mechanism:

1. the derivation building its own executable, which is the ordinary path;
2. a released executable generating inside a repository that is not this one, which is what a
   released executable is for;
3. a correctly matching prebuilt executable named by `MAJORDOMUS_BIN`, which is how a session
   avoids a rebuild it does not need.

A worker who borrows an executable establishes that it matches before running a derivation
with it, and reads the derived diff before committing: a regeneration that removes thousands of
lines from a document nobody edited is a refusal that did not happen.

## Failure behaviour

The refusal is the executable's own, so it holds wherever the executable is reached — the
derivation, the check, the git hook and a person at a terminal — rather than in one caller that
another caller can bypass.

**The two verdicts are separate exit codes, and that separation is the point.** A stale artifact
is `10`, the contract-unmet code, and its remedy is to regenerate. A foreign generation is `15`,
the refusal code, and its remedy is to rebuild or to name a different executable. Flattening
them is what made this destructive: a reader told the tree is stale does the one thing that must
not happen when the tool is the stale party. Every caller propagates the distinction rather than
collapsing it to failure, and each names the executable it was handed, because a borrowed
executable was previously invisible in a log.

What is compared is a property of the code that is the model, in three questions asked in order:
the declared version, which is stamped into every provenance header and so restamps everything
when it moves; a digest of the crate's own sources, which answers the ordinary case where the
derivation built the executable from the tree it derives; and, when those differ, the registry
the executable projects against the registry the tree records — which is what keeps a correctly
matching prebuilt executable working without a rebuild. Nothing derived from the repository's
own content may enter that comparison: a document of a new kind is the repository changing, not
the executable becoming foreign.

One limitation is inherent and is stated rather than hidden: an executable built before the
guard existed cannot refuse itself. The guard protects the trees it is run against, not the
worker who has kept an old binary.

## Verification

`test/cases/33_stale_executable.sh` points a deliberately mismatched executable at a tree and
asserts that the derivation refuses, that it exits the refusal code rather than the
contract-unmet one, and that nothing under the derived trees changed — not even an output
directory created. The same case runs the matching executable against its own tree and asserts
it proceeds, so the guard cannot be satisfied by refusing everything. The three uses above each
have an assertion, because a guard that broke the released-binary case would be discovered only
by somebody adopting the tool.
{% endraw %}
