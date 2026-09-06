+++
title = "The repository scope"
description = "the repository scope: what a worker reads and what it never reads, declared once in `.ai/repo/scope.yaml`, how a path is judged (name, size, content), what the executable does with it, and `majordomus scope`"
weight = 27
[extra]
source = "docs/SCOPE.md"
+++

{% raw %}

What a worker reads of this repository, and what it never reads, declared once as data:
[`.ai/repo/scope.yaml`](../.ai/repo/scope.yaml). The Rust executable reads it when it
starts, discovers and indexes nothing outside it, serves nothing outside it through MCP,
HTTP or the command line, and answers for any path whether it is in or out and why. The
shell tool seeds it on `init` and checks it in `doctor`. The rule behind it is
`project.scope-is-declared` under [`.ai/repo/rules/project/`](../.ai/repo/rules/project/);
the claim is `scope-declared` in [`CLAIMS.yaml`](CLAIMS.yaml).

The task scope (`majordomus start --scope`, the paths a task may touch) is a different
thing with the same word; this document is about the repository's own boundary.

## The declaration

```yaml
version: 1

in:                      # the allow-list: pathspecs anchored at the repository root
  - .ai/manifest.yaml
  - .ai/README.md
  - .ai/repo/**
  - src/**
  - lib/**
  - apps/**/src/**
  - test/**
  - docs/**
  - README.md
  - AGENTS.md
  - Cargo.toml
  - justfile

out:                     # wins over in; every category optional, each a reason
  paths:                 # never read, by path
    - .git/
    - .ai/local/
    - '**/node_modules/'
    - '**/target/'
    - vendor/
  binary: true           # content with a NUL byte in its first 8 KiB
  max_bytes: 1048576     # any file over this
  archive:
    names: ['*.zip', '*.tar.gz', '*.jar']
  image:
    names: ['*.png', '*.svg']
  video:
    names: ['*.mp4']
  pdf:
    names: ['*.pdf']
  database_dump:
    names: ['*.dump', '*.sqlite']
  generated:
    paths: [docs/generated/, site/data/generated/]
    names: ['*.min.js', '*.map']
  secret:
    names: ['.env', '.env.*', '*.pem', '*.key', 'id_rsa*']
  fixtures:
    paths: ['**/fixtures/', '**/testdata/']
    max_bytes: 65536     # a fixture over this is data, not context
```

Pathspecs are the layer's glob subset: `*` and `?` stay inside one path segment, `**`
spans any depth, a trailing `/` names a directory and everything beneath it. They are
anchored at the repository root; `**/target/` is how "at any depth" is said. A `names`
list is matched against the file name alone and may not carry a slash. A pattern with a
leading slash, a `:(` prefix or a `..` segment is refused with the file and the key named,
and so is a key the schema does not declare ([`share/schemas/scope.schema.json`](../share/schemas/scope.schema.json);
the shell tool's allow-list `share/allow/scope.txt` is generated from it by
`majordomus generate allow`).

## How a path is judged

In this order, and the first rule that decides wins:

<div class="overflow-x-auto">

| step | what is consulted | verdict |
|---|---|---|
| 1 | `out.paths`, then each category's `paths`, against the path | out: `path`, or the category |
| 2 | each category's `names`, against the file name | out: `secret`, `generated`, `archive`, `image`, `video`, `pdf`, `database_dump` |
| 3 | `in`, against the path | none matches: out, `undeclared` |
| 4 | the size, when the path is an existing file | over `fixtures.max_bytes` under a fixture path or name: `fixture_over_limit`; over `max_bytes`: `over_limit` |
| 5 | the first 8 KiB, when `binary: true` | a NUL byte: `binary` |
| | | in, with the `in` pathspec that admitted it |

</div>


A directory is in when an `in` pathspec can match something beneath it, and out when an
`out` path names it. A path that does not exist is judged by name alone and says so
(`exists: false`). A symlink is judged by name and reported as not existing: sources are
regular files. Encoding is not the scope's concern: a Latin-1 file passes step 5 and is
refused where it is read, as `invalid_utf8`.

## What the executable does with it

- **Discovery.** Every file a `sources.yaml` class discovers is judged before it is read.
  One outside the scope is not an object: it is reported as `out_of_scope` with the
  reason and the rule (`discovered by class 'document' but out of the repository scope
  (over_limit: max_bytes 1048576); not read`) and skipped. The diagnostic is a warning:
  the index stays `ok`, and the conflict between the class and the scope is visible in
  `majordomus mcp --inspect` and in `majordomus://repository`.
- **The index, and so every projection.** MCP `resources/list` and `resources/read`,
  `majordomus_get`, `majordomus_search`, `GET /api/v1/objects` and the rest are
  projections of the index; what the scope excluded is not found on any of them.
- **The tally.** At start-up every tracked file is judged by name and size, once, so that
  the report answers from memory: how many are in, how many out for each reason, and
  which. A tracked secret is a `tracked_secret` warning: it is never read or served, and
  it should not be in the repository. Content is not read for the tally; a binary with an
  innocent name is caught when it is read.
- **The origin.** The manifest's `scope` section names the file. A repository whose
  manifest names none is read under the distribution's default,
  [`share/skeleton/ai/repo/scope.yaml`](../share/skeleton/ai/repo/scope.yaml), the file
  `init` seeds, and the report says `distribution`. Nothing is assumed silently.

The declaration is compiled once when the process starts and never re-read per request:
`repository.scope` and `repository.scope_classify` read from the index, as
`project.rust-hot-path` requires.

## Asking

```bash
majordomus scope                          # origin, the tally, every tracked file that is out and why
majordomus scope docs/CLI.md .env target/x   # one line per path: in|out, reason, path, rule
majordomus scope --check --format json a b   # exit 10 when any path given is out
```

```
in                        docs/CLI.md  (docs/**)
out  secret               .env  (.env)  [absent]
out  path                 target/x  (**/target/)  [absent]
```

The same two capabilities, `repository.scope` and `repository.scope_classify`, are the
MCP tools `majordomus_scope` and `majordomus_scope_classify`, the resource
`majordomus://scope`, and the routes `GET /api/v1/scope` and
`GET /api/v1/scope/classify?path=...`; the reference is
[`generated/modules/repository.md`](generated/modules/repository.md). A path that is
absolute or carries a `..` segment is an invalid input on every one of them.

The shell tool's `doctor` reports the file under `layout`: its version, how many `in`
pathspecs it declares, and that every key is one the schema declares; a manifest naming
no scope section is an `INFO` saying the default applies. `init --extend` seeds the file
into a layer written before the section existed and says the manifest must name it.

## Changing it

Edit `.ai/repo/scope.yaml`, never the tool. A repository made of something the default
does not name adds it under `in` (`garden/**`); one whose generators write somewhere else
adds the directory under `out.generated.paths`; one that keeps large fixtures raises
`out.fixtures.max_bytes`. `majordomus scope` shows the effect on every tracked file
before anything is committed; `majordomus doctor` and `cargo test --test scope` are the
gates. The distribution's default changes in the skeleton, with the same review.

## What it is not

The scope governs what the executable reads and serves. It does not stop a person or a
provider from opening a file themselves, it does not scan content for secrets beyond the
names the declaration lists, and it is not `.gitignore`: a tracked file can be out
(generated data, an image), and an untracked file can be in (`--discovery filesystem`
reads the work tree under the same scope).
{% endraw %}
