# The provider bootstraps are a target of `majordomus generate`, rendered by one renderer, and `generate --check` refuses a stale one in CI

## What it means

`AGENTS.md`, `CLAUDE.md`, `GEMINI.md` and every other target the policy's `projections[]` declares are caches of two inputs: `.ai/repo/policy.yaml` with its profiles, and the provider template (`.ai/repo/providers/<provider>.tmpl`, else the distribution's `share/providers/<provider>.tmpl`). The Rust executable's `majordomus generate providers` writes them; `majordomus generate --check`, which CI runs on every push, derives them again, compares byte for byte, writes nothing, and exits 10 naming every file that differs or is missing. A rule typed into `AGENTS.md` by hand therefore does not merge.

## How it works

`apps/majordomus-cli/src/policy.rs` reads the policy through the same YAML subset reader the shell tool uses and hashes the policy file followed by the profiles in name order, which is exactly what the shell tool's `mj_policy_cat` hashes. `apps/majordomus-cli/src/providers.rs` fills `{{DEFAULT_PROFILE}}`, `{{CHECKPOINT_DEFAULT}}` and `{{POLICY_SHA}}`, prefixes a file-mode target with the stamp naming the policy hash and the content hash, and splices a region-mode target between the `majordomus:begin` and `majordomus:end` markers of its host document, leaving the rest of the host alone. The output is the same bytes the shell tool's `update` writes, so `doctor`, `watch` and `update --dry-run` accept a Rust-written target and `generate --check` accepts a shell-written one.

A declaration that cannot be produced is refused with the reason, and nothing is written: a target outside the repository, a provider with no template anywhere, a token the policy cannot fill (a template asking for `{{PROFILE_TABLE}}`), a value the policy does not set, and an `always_loaded` target over `context.always_loaded_budget_lines`.

## How to see it

```bash
apps/majordomus-cli/target/debug/majordomus generate providers --check   # OK generated AGENTS.md — matches the registry
printf '\nMy own rule.\n' >> AGENTS.md
apps/majordomus-cli/target/debug/majordomus generate providers --check   # exit 10: AGENTS.md (differs)
apps/majordomus-cli/target/debug/majordomus generate providers           # rewrites AGENTS.md to the very bytes it had
```

## What it does not cover

The shell tool's `update` still exists as an entry point with `--dry-run`, `--diff` and `--force`; it is the interactive writer, and the Rust `generate` is the gate. Both read the same inputs and write the same bytes, which `test/cases/93_rust_provider_projections.sh` proves in both directions; when that case fails, two renderers have diverged. The shell tool's block tokens (`{{PROFILE_TABLE}}`, `{{FINISH_CONTRACT}}`, `{{REQUIRED_SECTIONS}}`) are not rendered by the Rust executable: no shipped template uses them, and a template that does is refused with the token's name.

## Why it exists

The provider files are the one projection every AI worker reads before anything else, and until now only the shell tool's `doctor` compared them with their stamp; CI ran `generate --check` for the OpenAPI document and the reference but not for these. The repository's doctrine that provider files are adapters of `.ai/` and never a source (rule `project.interfaces-are-projections`, ADR 0001) is now enforced where it matters: a stale or hand-edited bootstrap fails the build.
