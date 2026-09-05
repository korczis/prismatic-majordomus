# The committed projections, the OpenAPI document, the capability reference and the allow-lists, are regenerated from the registry and the schemas, and generate --check refuses a stale one

## What it means

`docs/generated/openapi.json`, `docs/generated/capabilities.md` and `share/allow/*.txt` are caches of the registry and the schemas, committed so that the interface can be reviewed in a diff. `majordomus generate` writes them; `majordomus generate --check` derives them again, compares byte for byte, writes nothing, and exits 10 naming every file that differs or is missing. CI runs the check on every push, so a change to a descriptor or a schema without the regenerated files does not merge.

## How it works

`generate.rs` is the one pipeline: the OpenAPI document from `http/openapi.rs`, the reference from the registry's builtin entries, and one allow-list per schema carrying `x-majordomus-allow`, each a sorted, timestamp-free rendering. `--check` reads what is on disk and compares.

## How to see it

```bash
apps/majordomus-cli/target/debug/majordomus generate --check     # generate --check: in sync
printf '\n' >> docs/generated/openapi.json
apps/majordomus-cli/target/debug/majordomus generate --check     # exit 10: docs/generated/openapi.json (differs)
git checkout docs/generated/openapi.json
```

## What it does not cover

The check compares the committed files with the registry of the executable that runs it: a stale executable checks against a stale registry, which is why CI builds before it checks. The reference lists the builtin capabilities in full and describes declarative resources by rule rather than enumerating them, so a rule added to the layer changes no committed file.

## Why it exists

The repository's own doctrine, `project.derived-files-regenerated`, applied to the Rust executable's interface: a generated file is changed by changing its source and rerunning the generator, and both are committed together. `apps/majordomus-cli/tests/generate_check.rs` proves missing and tampered files detected without a write; `test/cases/76_capabilities_projections.sh` generates into a disposable repository, checks, tampers, and sees the refusal.
