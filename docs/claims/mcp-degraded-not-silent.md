# A file of the layer the MCP server cannot read is excluded with a diagnostic naming its path and the index says it is degraded, never silently smaller

## What it means

A rule with a key the allow-list does not know, a front matter that never closes, a policy declaring a version the executable does not read, a symlink, a file that is not UTF-8: each is excluded from what is served, and each produces one diagnostic with a stable code and the repository-relative path, on stderr at startup and in `majordomus://repository` for the client. The index reports `degraded`, `initialize` says so in its instructions, `mcp --inspect` exits 10, and `mcp --strict` refuses to serve at all. Two files of one kind claiming one identity are both excluded and both named.

## How it works

`src/index.rs` decides the policy in one place: the manifest and `sources.yaml` are errors, because without them nothing can be discovered; every other file that cannot become an object becomes a `Diagnostic` with a code from a closed list (`malformed_front_matter`, `unknown_key`, `duplicate_identity`, `unsupported_version`, `missing_field`, `kind_mismatch`, `unknown_kind`, `invalid_utf8`, `oversized`, `symlink`, `required_source_empty`, and the rest in the application README). Nothing is repaired, defaulted or normalised.

## How to see it

```bash
printf -- '---\nid: project.broken\nversion: 1\n' > .ai/repo/rules/project/broken.v1.md
git add -A
apps/majordomus-cli/target/debug/majordomus mcp --inspect | grep -E '^(state|FAIL)'
```

prints `state       degraded` and `FAIL malformed_front_matter .ai/repo/rules/project/broken.v1.md — front matter opened on line 1 and never closed`.

## What it does not cover

Degraded still serves by default; a client that must not see a partial layer passes `--strict`. The diagnostic says what is wrong with the file, not how the shell tool's `doctor` would judge it: `doctor` remains the gate, this is the reader's account of what it could read.

## Why it exists

A server that skipped what it could not read would look healthy while serving less than the repository declares, which is the failure `no-claim-without-test` and `derived-not-declared` guard against everywhere else. `apps/majordomus-cli/tests/metadata_contract.rs` produces every code above from a fixture; `test/cases/72_rust_mcp.sh` breaks one rule in a repository `init` wrote and asserts the state and the path.
