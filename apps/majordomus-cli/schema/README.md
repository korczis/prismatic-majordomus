# Kind schema

`kinds.yaml` is the one piece of declarative knowledge this executable ships rather than
reads from the repository: for each object kind, how a file of that kind is read and which
of its metadata fields carry identity, title and description. It is embedded at build time
(`include_str!`) together with the key allow-lists under the repository's `share/allow/`,
so that the Rust code and the shell tool validate the same keys from the same files.

Everything else is read from the supervised repository at run time:

| decides | read from |
|---|---|
| where the layer is | the nearest ancestor holding `.ai/manifest.yaml` |
| which sections exist and where | `.ai/manifest.yaml` (`sections:`) |
| which files carry which kind | `.ai/repo/knowledge/sources.yaml` (`kind` and `pathspec` per class) |
| which keys a kind may carry | `share/allow/<kind>.txt`, embedded |
| how a kind is read | this file |

## Contract

- **Version.** `schema: majordomus-kinds/v1`. A value the executable does not read is a
  build-time failure, not a run-time guess.
- **A kind the repository names that this file does not know** (a `kind:` in
  `sources.yaml` with no entry here) is reported as a diagnostic on every file of that
  class; the files are not exposed and nothing is guessed about them.
- **Unknown keys** in a file whose kind has an `allow` list are errors for that file: it is
  excluded and a diagnostic names the keys. A kind without an `allow` list (documents)
  declares no key contract, and its front matter, if any, is carried through as metadata.
- **Identity** is the listed fields joined with `@` (`id@version` for a rule, `name` for a
  prompt); a missing identity field excludes the file. Two files of one kind with the same
  identity are both excluded, and one diagnostic names both paths. A kind with an empty
  identity list is identified by its repository-relative path, which is unique by
  construction.
- **Adding an object** of a known kind is a repository change, never a change here.
- **Adding a kind** is an entry here plus, when it has a key contract, an allow-list under
  `share/allow/`. No Rust changes as long as the kind's format and front-matter rule are
  already among those listed above.
- **Changing how a kind is read** (a new format, a new identity rule) is a Rust change.

## Minimal example: a rule

```markdown
---
id: project.example
version: 1
kind: rule
title: Example
description: One line.
statement: The normative sentence.
status: active
class: advisory
depends_on: []
tags: []
---

# Rationale
```

Discovered through the `rule` class of `sources.yaml`, validated against
`share/allow/rule.txt`, identity `project.example@1`, exposed as
`majordomus://rule/project.example@1`.

## Invalid examples

| file | diagnostic |
|---|---|
| a rule whose front matter carries `owner: me` | `unknown_key` — `owner` is not in `share/allow/rule.txt` |
| a rule without `version` | `missing_field` — identity field `version` absent |
| a prompt whose `kind: rule` sits in the `prompt` class | `kind_mismatch` |
| a file beginning with `---` and no closing `---` | `malformed_front_matter` |
| two rules both `id: project.x`, `version: 1` | `duplicate_identity`, both excluded |
| a policy with `version: 2` | `unsupported_version` |

The behavioural cases are in `../tests/`.
