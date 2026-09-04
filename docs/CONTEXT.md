# Scoped context

Context is not one file. It is a tree of documents, each true for the directory it sits
in and everything below, read from the root down to the place a worker is about to
touch. The nearest document adds to its ancestors; it does not replace them, and two
sibling directories never see each other's.

This document describes the model: what a context document is, how the effective
context for a path is composed, what wins when documents disagree, how a change is traced
to the documents it affects, and what fails when the tree is wrong. The format is in
[`SCHEMAS.md`](SCHEMAS.md), the commands in [`CLI.md`](CLI.md), the rule that says where
a sentence belongs in `.ai/repo/rules/project/context-locality.v1.md`.

## What a context document is

A Markdown file under the `.ai/` tree (the manifest's directory, minus `local/` and minus
`rules/vendor/`) whose front matter declares `schema: context/v1` and `kind: context`.
File names are conventions, not identity: the `id` in the front matter is the identity,
and it survives a move. The manifest names the file names that must carry the contract
wherever they appear in the tree:

```yaml
context:
  documents: [README.md]
```

A `README.md` under the tree without the contract is an error. Any other Markdown file
without front matter is ignored, and a file whose front matter is of another kind — a
rule, a prompt — is not a context document.

Every document declares its `scope`: `directory` applies to its own directory only,
`subtree` to its directory and everything below, `explicit` to the directories listed in
`paths`. It declares the `providers` it addresses (`"*"` or names from the policy's
projections), the `audience` (`human`, `agent`), a `status` (`active`, or `deprecated`:
discovered and listed, never applied), an integer `order`, and optionally the git
pathspecs it `tracks`: the code or documents it describes, so that a change to them names
it for review.

## Composition

The effective context for a path is computed, never assembled by hand:

1. The target is a repository-relative directory; a file resolves to its directory. The
   directory must exist, and a symlink or a `..` that escapes the repository is refused.
2. Inside the tree, the candidates are the documents whose directory is the target or an
   ancestor of it, each admitted by its scope: `directory` only when the directories are
   equal, `subtree` when the document's directory is the target or above it, `explicit`
   when the target lies inside a listed path.
3. Outside the tree — `lib/`, `docs/`, anything not under `.ai/` — the candidates are the
   root chain (the subtree documents at `.ai/` itself) plus every document whose `tracks`
   matches the target.
4. The order is depth ascending, least specific first; then `order` ascending; then path
   ascending. Never the order a filesystem happens to list files in.
5. The filters apply: provider, audience, and `status: active`. A filtered document is
   absent from the result and present, with its reason, in the explanation.
6. Composition applies last. `extend` appends. `replace` appends and marks every id it
   names in `supersedes` as superseded: kept in provenance, dropped from the effective
   list. `final` may not be superseded by any descendant.

`majordomus context resolve <path>` prints the result, numbered, in effective order.
`majordomus context explain <path>` prints the same and, for every document considered,
why it is in (ancestor at depth N, tracks, directory) or out (filtered, superseded).

## Precedence

Less specific comes first and more specific comes later, so a reader who reads the
result top to bottom meets the general statement before the local one. That is also why
the nearest document does not silently win: a local document that contradicts an
ancestor without saying `replace` and naming it is two statements in force at once, which
the reviewer sees in the resolved chain and fixes at the source. Within one depth,
`order` decides; a tie in `order` is broken by path, so the result is the same on every
machine.

The root document, `.ai/README.md`, is `final`: nothing below the root may stand in for
the protocol that says how the tree is read.

## Conflicts

Validation refuses a tree in any of these states, naming the documents:

| class | meaning |
|---|---|
| `invalid-front-matter` | a document the manifest requires has no contract, or a required key is missing or malformed |
| `unsupported-schema` | the `schema` value is not one this version reads |
| `unknown-key` | a front-matter key outside `share/allow/context.txt` |
| `duplicate-id` | two documents claim one `id` |
| `broken-reference` | `supersedes` names an id that is not an ancestor, or does not exist |
| `cycle` | `supersedes` chains back on itself |
| `illegal-override` | a descendant supersedes a document marked `final` |
| `unknown-provider` | `providers` names a provider the policy has no projection for |
| `invalid-manifest` | the manifest's `context` block does not parse or names nothing |
| `refused-path` | the target escapes the repository, or does not exist |

Two documents at the same depth, `order` and path cannot happen: that is one file.

## Providers

`providers: ["*"]` addresses every worker; a list names the projections from the policy
whose workers the document is for. `resolve --provider claude-code` returns what that
worker should read and nothing addressed elsewhere. The provider files at the root
(`CLAUDE.md`, `AGENTS.md`, `GEMINI.md`) are bootstraps: they tell a worker to resolve the
context for the path it is about to touch, or to read the `README.md` chain from `.ai/`
down, and they carry no context of their own. A provider's native nested-file loading is
an optimisation of the same reading; when the two differ, the resolution above applies.

## Impact

`majordomus context affected` reads a change set from git — the working tree against
`HEAD` by default, `--staged` for the index, `--base <ref>` for `<ref>..HEAD` plus the
working tree — and reports what the change touches:

- a changed context document: its scope, every descendant directory, every document it
  supersedes or that supersedes it, and the projections, which carry the policy stamp;
- a moved or renamed directory: the ancestry that changed; identity moves with the file,
  and a duplicate or a broken reference after the move is an error;
- a deleted document: every `supersedes` that named it is a broken reference;
- a changed manifest or a changed allow-list: every document;
- a path matched by a document's `tracks`: a `WARN` finding naming the document and the
  path, because the tool cannot decide whether prose still describes code; it names what
  a person must read.

A change to a source no document tracks produces nothing. The review findings are
`WARN`, which never touches the exit code: the point is that nobody has to remember which
README described the file that just changed.

## Check-sync

`majordomus context check-sync [--base <ref>]` is the one command a hook or CI runs:
validation of the whole tree, the projections up to date against the policy, and the
affected review items for the change set. An invalid tree is `FAIL`, exit `10`; a
hand-edited projection is `DRIFT`, exit `11`; an absent projection is `INFO`, because
`doctor` owns "missing"; the review items are `WARN`; otherwise exit `0`. `.github/workflows/validate.yml` runs it on this
repository, and the vendored rule `majordomus.context-integrity` dispatches the same
validation from `doctor` and `watch`.

## Reading it from the command line

```
majordomus context list                              every document: id, path, scope, composition, providers, status
majordomus context resolve <path> [--provider P] [--audience A]
majordomus context explain <path> [--provider P] [--audience A]
majordomus context validate                          the whole tree; exit 10 on any failure
majordomus context affected [--base <ref>|--staged|--worktree]
majordomus context check-sync [--base <ref>]         validate, projections, affected; exit 10 / 11 / 0
```

All of them are read-only and accept `--json`. Bare `majordomus context` is still the
briefing builder, and its briefing gains a `CONTEXT DOCUMENTS` section listing the
effective chain for the active task's scope paths.

## Failure modes

The tree is refused, not repaired: no default is filled in for a missing required key,
no id is invented for a document without one, no order is guessed. A worker that reads a
tree the tool refuses reads it at its own risk, and `check-sync` in the hook is what
keeps that from being the state of the branch. What the tool cannot decide it names:
whether a document still describes the code it tracks is a `WARN` finding, never a pass.

## Migration

A repository on the layer before this model has READMEs without contracts. Adding the
contract is adding front matter; the body stays. Start at the root (`.ai/README.md`,
`final`), then `repo/README.md`, then each section; run `majordomus context validate`
until it is quiet; then add `tracks` where a document describes code, so that the next
change to that code names it. Nothing else moves, and a repository that never adds a
second document has exactly the context it had before, now resolvable.

`test/cases/69_context_documents.sh` proves the resolution order, the composition rules,
each conflict class, the filters, the impact report and the sync check by mutation.
