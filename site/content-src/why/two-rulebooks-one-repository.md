+++
title = "Two different rulebooks for one repository"
description = "Why CLAUDE.md and AGENTS.md drift apart, and how one policy generates every instruction file with its own stamp."
weight = 5
[extra]
hook = "opened CLAUDE.md and AGENTS.md and found two different rulebooks for one repository"
responsibilities = ["policy", "projection", "doctor"]
commands = ["update", "doctor"]
claims = ["projection-generation", "projection-fingerprint", "no-silent-overwrite", "context-budget", "wiring-reconciliation"]
+++
## The moment

Someone added a rule to `CLAUDE.md` in March. Someone else added a different rule to
`AGENTS.md` in May. Today the repository has two operating contracts, and which one applies
depends on which tool happens to be open.

## Why it happens

Each AI tool reads its own file, each file is hand-edited, and nothing relates them. In a
workspace of about twenty repositories, the two files for the same repository shared between
none and a tenth of their content — not duplicates, disjoint rule sets. One always-loaded
contract oscillated between empty and about eleven hundred lines across two hundred hand
edits. Rules also decay in a second way: a hook is documented as enforcing, exists on disk,
and is dispatched by nothing. Seventeen such cases were found before this tool was designed.

## What Majordomus does

There is one canonical policy, `.ai/repo/policy.yaml`. `majordomus update` generates
every instruction file the policy names — `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, or any
target you add — from the same body, deterministically, and stamps each with the policy hash and the hash of its own content.
A hand edit is detected by `doctor` and `watch`; `update` refuses to overwrite it until you
have seen the diff. The always-loaded file has a line budget with a failing check.

`majordomus doctor` also reconciles the policy's enforcement list against what actually
runs: the path exists, is executable, and is invoked by the hook it names without its exit
code being swallowed. Declared-but-not-wired was the most common failure in the source
material; here it is a failing check, applied to this repository's own hooks first.

## What it does not do

It projects a deliberately narrow subset of the policy and lists what is not projected. It
does not merge existing hand-written instruction files; on first `update` you decide what
moves into the policy body. Providers it has no template for get the generic Markdown.

## Try it

```bash
majordomus init && majordomus update && majordomus doctor
echo "my own rule" >> CLAUDE.md && majordomus doctor   # FAIL projection … hand-edited?
```
