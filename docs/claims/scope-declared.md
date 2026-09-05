# What a worker reads of the repository is declared once in .ai/repo/scope.yaml, out wins over in, and the Rust executable discovers, indexes and serves nothing outside it

## What it means

A repository says, in one tracked file, which paths a worker reads (`in`: pathspecs such
as `src/**`, `docs/**`, `.ai/repo/**`) and what it never reads (`out`: version control,
the local half of the layer, dependencies and build outputs, secrets, generated assets,
archives, images, video, PDF, database dumps, fixtures over a size, any file over a size,
binary content). `out` wins over `in`, and a path that matches nothing is out with the
reason `undeclared`. The executable reads that file once when it starts, and from then on
a file outside the scope is not an object of the index: it is not listed, not read, not
searched and not served, through MCP, HTTP or the command line alike. Any path can be
asked about: `majordomus scope <path>` answers in or out, the reason, and the pattern or
limit that decided. A repository that declares no scope is read under the distribution's
default, the same file `majordomus init` seeds, and the answer says so.

## How it works

`apps/majordomus-cli/src/scope.rs` types the declaration (`deny_unknown_fields`, and
the `scope` JSON Schema under `share/schemas/`, from which the shell tool's allow-list is
generated), validates every pattern, and compiles it into ordered rules: `out.paths` and
each category's `paths` against the repository-relative path, each category's `names`
against the file name alone, then the `in` pathspecs, then size against
`fixtures.max_bytes` and `max_bytes`, then the first bytes of content. `Index::build`
judges every discovered file before reading it and records `out_of_scope` with the reason;
it also tallies every tracked file by name and size once, so `repository.scope` answers
from memory, and reports a tracked secret. `Scope::load` reads the manifest's `scope`
section or the distribution's `share/skeleton/ai/repo/scope.yaml`. The shell tool's
`doctor` checks the file's version and keys against `share/allow/scope.txt`, and `init`
seeds it.

## How to see it

```bash
just build
B=apps/majordomus-cli/target/debug/majordomus
$B scope                                   # origin, the tally, every tracked file that is out and why
$B scope docs/CLI.md .env target/x site/data/generated/plan.json
$B scope --check --format json docs/CLI.md .env; echo "exit $?"    # 10: a path is out
# in an MCP client: majordomus_scope_classify {"path": "priv/dump.sql.gz"}
curl -s 'http://127.0.0.1:8741/api/v1/scope/classify?path=.ai/local/state/current.yaml'
```

## What it does not cover

The scope governs what the executable reads and serves; it does not stop a person or a
provider from opening a file themselves, and it does not scan content for secrets beyond
the names the declaration lists. The tally judges names and sizes, not content: a binary
with an innocent name is caught when it is read, not counted in advance. Task scopes
(`majordomus start --scope`) are a different thing: the paths a task may touch.

## Why it exists

The operator gave the boundary as a list: in `.ai/repo/**`, source, tests, configuration,
the root manifests; out `.git/`, `.ai/local/`, dependencies, build outputs, binaries,
archives, images, video, PDF, database dumps, fixtures over a limit, generated assets,
secrets. Until then the boundary was three unrelated mechanisms: the local half skipped
in discovery, a size limit compiled into the reader, and whatever a source class happened
to match. One declaration, read by the executable and checked by the shell tool, replaces
them, and `test/cases/93_scope_policy.sh` proves the executable honours it.
