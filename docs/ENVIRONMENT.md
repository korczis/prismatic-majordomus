# The repository environment — one snapshot, every surface a rendering

What this checkout is right now — the project and its version, the repository and its
layer, version control, the toolchains the repository declares, what the layer holds, the
workflows a person can run here, the provider projections and the local services — is one
typed value, `environment::RepositoryEnvironment`, built by one resolver under
[`apps/majordomus-cli/src/environment/`](../apps/majordomus-cli/src/environment/). Every
surface that says any of those things renders that value: `majordomus env` on the command
line, the HTTP route `/api/v1/environment`, the MCP resource `majordomus://environment`,
and the banner direnv draws on entering the directory. None of them discovers anything of
its own. Behaviour as implemented and tested; where this document and the executable
disagree, the document is wrong and changes in the same commit.

The commands and their executable examples are in the generated reference
([`generated/cli.md`](generated/cli.md), under `majordomus env`); the capabilities, their
routes and their benchmark cases in [`generated/capabilities.md`](generated/capabilities.md),
module `environment`. Neither is restated here.

## Two resolutions, one model

Counting what the layer holds means building the index, which costs seconds; the banner
runs on every `cd`. So a snapshot is resolved in one of two modes, and both produce the
same type:

- **full** reads everything, the index included, and writes the cache. `env status` and
  `env explain` resolve in full, because a person is waiting for them.
- **fast** reads only what is cheap — one `git status`, one `just --dump`, a few file reads —
  and takes the rest from the cache the last full resolution wrote. `env banner` and
  `env export` resolve fast, because direnv runs them on every entry.

What no cache can supply is reported as unknown, never guessed and never defaulted: every
tier of the snapshot carries its state (`resolved`, `cached` or `unavailable`), and an
unavailable value is absent rather than zero. The two modes differ in what they may read,
never in what they mean; the crate's tests hold them to that.

## Provenance

Every field carries where it came from: the file, command or compile-time constant that
decided it, the resolver that read it, and whether it was read now, taken from the cache
or not resolved at all. `env explain` prints it for one field, a prefix or every field;
the capability `environment.explain` answers the same over HTTP and MCP.

## The cache

The cache lives under `.ai/local/state/environment/`, beside the shared server's lease and
for the same reason: it belongs to this checkout, is never tracked, and a fresh clone
starts without it. It holds counts, recipe names and their descriptions, and toolchain
version strings — nothing read from the process environment, no file contents, no path
outside the repository. Each tier carries its own fingerprint over its own inputs, so
editing the justfile expires the workflows and leaves the counts alone. A cache that
cannot be read is a miss, never an error, and writes are atomic. A served request — HTTP
or MCP — never writes it: the cache is the command line's, and a `GET` with a side effect
on the repository would be a defect.

## The shell entry point

`.envrc` is an adapter and nothing else. It puts `bin/` on the path, watches the lease so
that the banner is re-evaluated when the shared server comes up or goes away, and
evaluates one call: `bin/majordomus-env export --shell direnv --banner`. That one process
writes the assignments — `MAJORDOMUS_ROOT`, `MAJORDOMUS_SHARE`, and `MAJORDOMUS_URL` when
a server is running — on standard output, where direnv reads the environment it applies,
and the banner on standard error. `bin/majordomus-env` finds the executable through
`lib/rust_bin.sh` and never builds it: a missing executable is one line on standard error
naming `just build`, and exit 0, because a non-zero exit on a `cd` makes direnv report
that the whole environment failed.

The rule that holds this shut is `project.envrc-is-an-adapter`
([`.ai/repo/rules/project/envrc-is-an-adapter.v1.md`](../.ai/repo/rules/project/envrc-is-an-adapter.v1.md)):
a file a shell evaluates on entering the repository resolves the tool, evaluates what it
exports and asks it to render; it reads nothing about the repository, builds nothing and
reaches no network.

`MAJORDOMUS_BANNER` chooses `auto`, `full`, `compact` or `off`; `NO_COLOR` makes the output
plain, and under `CI` there is no banner at all. The `env` group of the justfile
(`.just/env.just`) carries the recipes a person runs.

## What is canonical, and what is derived

| Fact | Canonical source |
|---|---|
| project name, version, licence, summary | the crate manifest, at compile time |
| repository root, layer sections | `.ai/manifest.yaml`, through the repository model |
| version control | one `git status --porcelain=v2 --branch` |
| toolchains | the manifest that declares each one |
| what the layer holds | the index, through the capability registry |
| workflows | `just --dump --dump-format json` |
| provider projections | the policy's `projections[]` |
| services | the executable's own route constants and the shared server's lease |

Nothing in the module writes outside `.ai/local/`, and nothing in it opens a socket to
anything but the loopback address the lease names — and only when asked to probe.
