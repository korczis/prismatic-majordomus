# Two-folder consolidation — forensic findings and migration plan

Phase 1 deliverable of the two-folder consolidation programme. Everything below was
verified against the tree at `948dbd93` by reading the code, and by running the binary
where a claim could only be settled that way. Where a thing was looked for and does not
exist, that is stated as a finding rather than left silent.

The target: after `majordomus init`, a foreign repository carries Majordomus in two
namespaces — `.ai/**` and `.majordomus/**` — plus small, stable, ownership-tracked
bridges in root files that a tool genuinely requires, and `majordomus uninstall` takes
the bridges back out.

## 1. What already holds

These are not aspirations here; they are implemented and tested, and the migration must
build on them rather than beside them.

**Discovery is convention-driven.** `.ai/repo/knowledge/sources.yaml` declares source
classes as glob pathspecs, and `apps/majordomus-cli/src/discovery/mod.rs` enumerates
exactly those, through the git index or an equivalent walk. No file is registered by
name. ADR 0007 states the goal for skills — "no registry, no site list, no MCP entry, no
provider file, no Rust match arm to touch" — and `test/cases/95_skills.sh` proves it end
to end: a new skill is `mkdir`, one `SKILL.md`, one `git add`, and nothing else.

**MCP, HTTP, OpenAPI and the Cockpit are genuine projections of one registry.** Dispatch
is generic in every one of them: `registry.by_mcp_tool` (`mcp/surface.rs`),
`registry.by_http` (`http/router.rs`), `registry.iter()` (`http/openapi.rs`). The
Cockpit's own header records the invariant: "Nothing here holds a list of capabilities,
routes, kinds or graphs, and adding one to the registry adds it here."

**Schemas are generated from canonical sources, in both directions that matter.**
Capability input and output schemas come from the Rust types through `schemars`
(`capability/schema.rs`); document kinds are authored as `.proto` and projected into
JSON Schema, allow-lists and section lists by `majordomus generate` (`proto/mod.rs`,
ADR 0004). Even the generator's own outputs have contracts, under
`share/schemas/generated/`.

**Provider files are already output, not source.** `AGENTS.md` and `CLAUDE.md` carry a
`majordomus update` stamp over the policy hash and their own content; the two projections
are declared in `.ai/repo/policy.yaml`, rendered from templates, and `update` refuses to
overwrite a hand edit without `--force`. Both are pointers only — `AGENTS.md` itself says
a rule that exists there and nowhere else is a bug.

**The directory contract is enforced, not merely written down.** `children.require_contract`
and `exempt:` let a subtree declare that it owes no README, and `majordomus context
validate` reports `missing-contract` for a directory that owes one and lacks it
(`lib/context_docs.sh`, `test/cases/69_context_documents.sh`).

**Hand edits are already detectable.** `lib/update.sh` compares a target's content against
the hash in its own stamp and refuses when they diverge. That comparison is the primitive
a safe uninstall needs, and it exists.

## 2. What does not exist at all

**`.majordomus/` is not a project namespace.** It is reserved in the code for an optional
*tool installation* — a tree containing `.majordomus/bin/majordomus` — and
`repository.rs` explicitly does not treat it as a repository marker. The second half of
the two-folder target has to be designed, not migrated.

**There is no uninstall, reset, clean or remove — in either implementation.** Every
mutation Majordomus performs is add-or-replace only: the stamps in `AGENTS.md` and
`CLAUDE.md`, the `.ai/local/` line appended to `.gitignore`, the hooks and settings
written into `.claude/` by `capture install`. Nothing reverses any of them.

**There is no ownership ledger for installation.** `init` accumulates what it created in
a shell variable, prints it, and discards it. `share/events.yaml` declares the whole
event vocabulary and contains no `init` event, so the append-only ledger structurally
cannot record an installation. `.ai/manifest.yaml` is a section registry for discovery,
not a record of provenance.

**`init` is not idempotent — it is refusing.** A second run exits 15 with "already
exists"; `--extend` then seeds only what is missing, by existence check, never by
comparing desired state against actual. There is no plan, no diff, no dry run of a
reconciliation.

**There is no environment integration.** No `.envrc`, no `env` subcommand, no banner, no
shell integration, and no completion generation (`clap_complete` is not a dependency).
`test/cases/01_init.sh` asserts that `init` must not create an `.envrc`. Performance
budgets exist, but only for `doctor` and `watch`.

**There is no schema migration mechanism.** `LAYER_SCHEMA` is pinned to `ai-repository/v1`
and a `v2` manifest is *refused*, with a test proving the refusal. The only thing named
migrate is the one-time `.majordomus/` → `.ai/` layout move in the shell tool.

## 3. Where one fact is written twice

This is the section that matters most, because the programme's premise is that duplicate
truth is the defect.

**The CLI path of a capability is declared twice and reconciled never.** Once as
`exposure.cli.path` on the capability, once in the hand-written clap tree in `cli.rs`.
The registry builder checks only that the path is non-empty and unique; it never consults
`cli::tree()`. This is not theoretical. `docs/generated/capabilities.md` documents
`repository.scope_classify` as `majordomus scope classify`, and typing that runs
`repository.scope` with `classify` swallowed as a path to judge:

```
$ majordomus scope classify docs/CLI.md --format json
[ {"path": "classify", "verdict": "out", "reason": "undeclared", "exists": false},
  {"path": "docs/CLI.md", "verdict": "in", "rule": "docs/**", "exists": true} ]
```

The same wrong string reaches four rendered artifacts — the generated capability table,
the OpenAPI `x-majordomus-cli` extension, the Cockpit, and the docs — from one
unchecked declaration.

**`majordomus distribution` implements half its surface outside the registry.** Of eight
clap subcommands, `Validate`, `Targets`, `Matrix` and `Metadata` call `Model::load` and
`render::matrix_json` directly, bypassing `ctx.execute`, while the module's own header
claims "the command line is a projection of the registry, not a second implementation
beside it."

**The inventory of generated artifacts is split between two generators.**
`docs/generated/artifacts.json` is itself generated and covers 71 documents, but by its
own text only "every file `majordomus generate` writes". Stage B of `scripts/derive` —
the shell script `scripts/generate-site-data` — writes `docs/SITE_CLAIMS.md`,
`docs/PLAN_STATUS.md`, all of `site/data/generated/**` and all of `site/content/**`, and
none of those appear in the manifest. Completeness is reached today only by running two
independently scoped checks and unioning their exit codes in `scripts/derive-check`;
nothing materialises the combined list.

The consequence is not abstract. `.gitattributes` marks derived artifacts `merge=derived`
so they resolve instead of conflicting, and the coverage gate in
`test/cases/57_derived_merge_driver.sh` asks `git check-attr` about every path in
`artifacts.json` — so the files stage B writes were outside both the marking and the gate,
and conflicted on every merge until they were added by hand.

`README.md` states that "Every generated file in the repository comes out of the same
executable." By the evidence above that sentence is false.

**Two files hand-authored among generated siblings.** `share/allow/` holds 28 files; 26
carry `# GENERATED FILE — DO NOT EDIT DIRECTLY` with a generator version.
`commands.txt` and `events.txt` carry no banner, have no generator — `command.v1.schema.json`
declares no `x-majordomus-allow` — and grep finds only readers, never a writer. They are
indistinguishable from their generated neighbours to anyone who does not check.

**The provider renderer exists twice.** `lib/update.sh` and
`apps/majordomus-cli/src/providers.rs`, the latter documented as "byte-identical to the
shell tool's `majordomus update`" and held there by a parity test. A parity test between
two implementations is not one source of truth; it is an alarm on the divergence of two.

**Provenance has two grades.** The Rust banner names source, generator and the command
that regenerates. Stage B writes only "Generated by `scripts/generate-site-data` from
`docs/CLAIMS.yaml`" — no version, no schema id, no regeneration command, and nothing that
can enumerate the files carrying it.

## 4. Ownership is not only about files

The merge driver landed today is a per-clone `git config` (`merge.derived.driver`). Git
says nothing when it is absent: it falls back to the default text merge, so a clone looks
configured, every file is committed, and the driver never runs. That hole is now closed by
an `enforcement` entry verified by a `git-config` wiring kind in doctor —
but `core.hooksPath` has exactly the same hole and remains a manual "once per clone" step
in `CONTRIBUTING.md`.

This generalises: an ownership ledger that records only files will report a conforming
repository that is not actually wired. **The ledger has to cover clone configuration as
well as file mutations**, or `init` will honestly report zero changes about a repository
where nothing is enforced.

## 5. Classification

| Category | What is in it today |
|---|---|
| `canonical-authored-ai` | `.ai/repo/**` except generated projections: rules, prompts, profiles, skills, adrs, use-cases, applications, why, knowledge/curated, workflows, sessions, project, deployments, ci, benchmarks, `policy.yaml`, `scope.yaml`, `manifest.yaml`, `knowledge/sources.yaml` |
| `canonical-majordomus-config-or-ownership` | does not exist yet — `.majordomus/manifest.json` and the ownership ledger are new |
| `derived-persisted` | `docs/generated/**`, `share/allow/*.txt` (26 of 28), `share/sections/*.txt`, `share/schemas/**/*.schema.json`, `site/data/registry/**` |
| `generated-disposable` | `site/data/generated/**`, `site/content/**`, `docs/SITE_CLAIMS.md`, `docs/PLAN_STATUS.md`, `docs/PAGES_STATUS.md` — stage B's outputs, today outside the manifest |
| `runtime-ephemeral` | `.ai/local/state/**`, `.ai/local/session-contexts/**`, the lease file, peer board |
| `cache` | `.ai/local/cache/**`, `.ai/local/prompts/**` renderings |
| `external-compatibility-bridge` | `AGENTS.md`, `CLAUDE.md` (stamped projections), the `.ai/local/` line in `.gitignore`, `.claude/settings.json` + `.claude/hooks/*` (written by `capture install`) |
| `legacy-to-migrate` | the pre-`.ai` `.majordomus/` layout handled by `lib/migrate.sh`; `share/allow/{commands,events}.txt` as hand-authored files in a generated directory |
| `user-owned-do-not-touch` | `README.md`, `justfile`, `.mcp.json`, `.gemini/settings.json`, `.codex/config.toml`, everything outside the namespaces above |

Two entries in that table are the programme's real work: the empty
`canonical-majordomus-config-or-ownership` row, and the fact that
`generated-disposable` is currently invisible to the mechanism that is supposed to know
every generated file.

## 6. Ordered slices

Each slice is meant to stand on its own and leave the repository better even if the next
one never happens.

1. **One inventory of every generated artifact, across both generators.** Extend the
   manifest so stage B's outputs are in it, with the same provenance grade. This is the
   prerequisite for `.gitattributes` coverage, for the merge-driver gate, for
   `derive-check`, and for any claim that generated files are disposable. It also makes
   the false sentence in `README.md` true instead of deleting it.
2. **Reconcile the two CLI declarations.** Either derive the clap tree from
   `exposure.cli`, or gate that every `CliExposure.path` resolves to a real subcommand.
   The gate is the smaller change and would have caught `scope classify` the day it
   appeared. Fold the four registry-bypassing `distribution` subcommands back in.
3. **Ownership ledger, covering files and clone configuration.** Typed, versioned, under
   `.majordomus/`. Records for each mutation: path or config key, kind, the hash as
   installed, the hash now, generator identity and version, and the inputs it derives
   from. This is the object `init`, `sync`, `doctor` and `uninstall` all read.
4. **`init` becomes a reconciler.** Desired state constructed from discovery, diffed
   against actual, minimal mutation applied, result validated and recorded. `--dry-run`
   prints the diff. The idempotence gate — a second run changes nothing — replaces the
   current exit-15 refusal.
5. **`uninstall`, conservative by default.** Removes the bridges the ledger owns, leaves
   `.ai/**` alone, refuses to touch anything whose current hash does not match the
   installed one, and offers `--purge-generated` and `--purge-ai` as explicit opt-ins.
6. **`.majordomus/` as a real namespace**, with its own `.gitignore` so runtime and cache
   never reach git, and its README carrying the same contract every other directory owes.
7. **Environment integration, built once and never touched again.** `.envrc` gets one
   stable line pointing into `.majordomus/`; everything else lives in the binary, behind
   a performance budget of the kind `doctor` and `watch` already have.
8. **Fold the second provider renderer into one.** Parity between the shell and Rust
   renderers is currently maintained by a test; it should be maintained by there being
   one implementation.

## 7. Risks

The shell tool and the Rust executable are two implementations of one product, and the
lifecycle lives entirely in the shell one. Any slice that moves `init` into Rust crosses
that boundary and must not leave the repository with two lifecycles. Slice 1 and slice 2
do not cross it, which is one reason they come first.

`majordomus init` is the tool's contract with every repository that has already adopted
it. Slice 4 changes what a second run does, from refusal to reconciliation. That is a
behaviour change for existing users and needs the migration path stated before the code
is written, not after.
