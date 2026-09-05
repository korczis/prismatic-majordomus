---
schema: context/v1
id: ai.layer
kind: context
title: Repository AI context
description: The protocol of the layer: what it holds, who owns which half, how it is discovered, and what outranks what.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: final
order: 10
---
# Repository AI context

This `.ai/` directory is the provider-neutral AI context and governance layer for this
repository. It is written for a person or a capable model reading it without any tool
installed, and it is the same layer whichever assistant, editor or vendor is doing the
reading. Majordomus is the reference tool that validates, projects and enforces it; the
format is the repository's, not the tool's.

## Purpose and boundaries

`.ai/` holds what a worker needs in order to work on this repository correctly: the
policy, the execution profiles, the rules, reusable prompts, skills, workflows, knowledge
declarations, architecture decisions and the plan. It holds nothing that is derived from
those things and nothing that belongs to one machine.

It is not an agent framework, a runtime, a memory service or a prompt library, and no
file in it is executable.

## Ownership: tracked and local

```text
repo/    tracked in Git; canonical repository context shared by every checkout
local/   this checkout's own state; ignored by Git, never shared, never normative
```

`repo/` is the whole shared contract. A fresh clone contains it and nothing else.

`local/` holds operational state: the active task, the ledger, checkpoints, handovers,
sessions, local decisions and open questions under `local/state/`, rebuildable caches
under `local/cache/`, bounded working contexts under `local/session-contexts/`, and raw
local user prompts under `local/prompts/` when an integration can observe them. Nothing
under `local/` may be loaded into a model's context implicitly, published by a generator,
or read as policy. `local/` never contains rules.

## Discovery

Discovery starts from `manifest.yaml`: the manifest names the tree and where each section
lives, and a document's own contract, its front matter, identifies it as context. The
manifest names the tree; the contract identifies a document:

1. read this file,
2. read `manifest.yaml`,
3. load the registered sections under `repo/` that the task needs,
4. load the mandatory rules and resolve their dependencies (below),
5. load only the skills and knowledge the task is about,
6. never load `local/`.

A file under `repo/` that no manifest section covers is not context and carries no
authority. Discoverability is not eager loading: discover what may apply, resolve what
does apply, then load the minimum that suffices.

## Scoped context

Every `README.md` in this tree is a context document: Markdown whose front matter declares
`schema: context/v1` and `kind: context`, an `id` that survives a move, a `scope`
(`directory`, `subtree` or explicit `paths`), the `providers` and `audience` it addresses,
and a `composition`. The manifest's `context.documents` names the file names that must
carry that contract wherever they appear in the tree.

The effective context for a path is the chain of documents from `.ai/` down to the
nearest directory that has one, least specific first: this file, then `repo/README.md`,
then the section's own. A document with `composition: extend` adds to its ancestors; one
with `replace` names in `supersedes` the ancestors it stands in for; one marked `final`
may not be superseded by any descendant. The nearest document never silently replaces the
chain above it, and sibling directories never see each other's documents. A path outside
the tree, such as `lib/`, gets the root chain plus every document whose `tracks` names it.

```text
majordomus context resolve <path>     the effective chain for a path, in order
majordomus context explain <path>     the same, with why each document is in or out
majordomus context validate           the whole tree: contracts, ids, references, overrides
```

A provider's own nested-file loading is an optimisation; this resolution is what applies.
`--provider` and `--audience` filter; a deprecated document is listed, never applied.

## Rules

Rules live under `repo/rules/` as Markdown documents with YAML front matter; read
`repo/rules/README.md` before interpreting one. Two locations:

```text
repo/rules/vendor/majordomus/   the pinned Majordomus baseline; read-only, upgraded explicitly
repo/rules/project/             rules this repository wrote
```

The effective rule set is additive: every active vendored rule plus every active project
rule. A project rule may add a constraint; nothing here weakens or replaces a baseline
rule, and there is no override mechanism.

Rule identity is the `id` and `version` in the front matter, never the file name. A rule
may depend on others through `depends_on`, each entry an exact `id@version`. Resolution is
deterministic: a missing dependency, a cycle, or two rules claiming one identity at the
same scope is an error, and a set that does not resolve is not applied partially.

A rule's `class` is `blocking` or `advisory`. Blocking means a violation stops the command
that enforces it; advisory means the violation is reported and work continues. There is no
third class. A rule with an `x-majordomus` block is enforced by the tool; a rule without
one is normative for whoever reads it and enforced by nobody, which the rule says.

## Scope

`repo/scope.yaml` declares what a worker reads of this repository and what it never
reads: `in`, the pathspecs it reads; `out`, which wins, the paths, names, sizes and
content it does not. A path matching nothing is out. The tool reads the declaration once
when it starts and discovers, indexes and serves nothing outside it; a repository that
declares none is read under the tool's default and told so. `majordomus scope <path>`
answers in or out and names the rule.

## Use cases

`repo/use-cases/` holds the tasks people perform with the tool, one file each, and
`repo/applications/` the contexts it suits. A use case names what it relies on and
carries a scenario the tool executes against itself; the page it becomes shows that
execution, its maturity is observed from the evidence, and a public command no active use
case runs is a gap `doctor` reports and, when the policy says so, `finish` refuses. The
discipline is `repo/workflows/use-cases.md`.

## Versioning

`manifest.yaml` carries the format version, currently `ai-repository/v1`. A tool that
reads an older format than the manifest declares must refuse rather than guess. The
vendored rule package carries its own manifest with the package version and the revision
it was taken from; upgrading it is an explicit, reviewable change to tracked files.

## Precedence

Git is the authority. Every record that names a branch, a commit or a worktree had that
value computed, and a record that disagrees with the repository as it is now is evidence
of what was true then, not an instruction. Within this layer: the manifest, then the
rules, then the policy and profiles, then everything else. `local/` outranks nothing.

## Failure behaviour

A manifest that does not parse, a section it names that is absent, a rule that does not
resolve, or a policy key nothing reads is an error to be reported by name and fixed at its
source. Nothing here is repaired silently, defaulted silently, or overwritten silently.

## Majordomus

`majordomus init` created this layer from the tool's skeleton; from that moment the files
under `repo/` belong to this repository and a newer tool does not rewrite them.
`majordomus doctor` proves the layer is real: the manifest resolves, the vendored package
matches its manifest, every enforced rule reaches the command that claims to run it, and
`local/` is ignored. `majordomus context` assembles what the next worker needs from
`local/state/` within a budget. A `.majordomus/` directory, if one exists, is only an
optional installation of the tool itself and is not repository AI state.
