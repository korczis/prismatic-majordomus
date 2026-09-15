# Current code hotspots to inspect during implementation

This is not an exhaustive dependency list. Use `rg` and Git history before
editing. It is a map of high-leverage areas observed in the supplied snapshot.

## `bin/majordomus`

Current help text still says:

```text
init set up .majordomus/
```

and command descriptions are based on the legacy layout.

Update the command surface only after implementation semantics are real.

The entrypoint already resolves `lib/` relative to its own binary directory.
Preserve that useful property because it enables PATH/global/submodule/package
installs without repository coupling.

## `lib/common.sh`

Highest-leverage migration point.

Current code sets roughly:

```text
MJ_ROOT = Git repository root
MJ_DIR  = $MJ_ROOT/.majordomus
```

and many commands interpret `MJ_DIR` as policy + project + state.

Replace this overloaded concept with centralized explicit paths.

Current policy/profile readers also use `MJ_DIR`. Migrate them centrally before
mass-editing callers.

Preserve:

- Git identity derivation,
- path normalization,
- YAML subset behavior,
- region projection behavior,
- exit-code contracts.

## `lib/init.sh`

Currently:

- creates `.majordomus/`,
- copies policy/profiles/templates/providers/prompts,
- initializes state,
- optionally ignores state.

Target:

- seeds `.ai/`,
- seeds vendored standard rules,
- ensures `.ai/local/` ignore boundary,
- does not install tool distribution,
- does not mutate `.envrc`,
- does not require `.majordomus/`.

Review `--force` semantics. A new init must not overwrite repository-owned
`.ai/repo/**` just because the executable skeleton changed.

## `lib/update.sh`

Current responsibilities include:

- policy/profile hash,
- provider body assembly,
- provider template rendering,
- always-loaded budget,
- region/file projections,
- hand-edit protection,
- fingerprint writes.

Target should preserve the useful projection engine but render thin bootstraps.

Be especially careful with fingerprint/provenance behavior. Do not downgrade it
to a local cache without fresh-clone tests.

## `lib/doctor.sh`

A major migration gate.

Currently references legacy paths heavily and proves:

- policy,
- profiles,
- layout,
- projections,
- enforcement wiring,
- doctrine wiring,
- project model,
- DAG/roadmap,
- retention,
- context budget.

Doctor must evolve into the proof that the new `.ai` installation is real.

Add explicit checks for:

- `.ai` schema/manifest,
- tracked/local boundaries,
- vendor rule manifest,
- rule dependency resolution,
- no legacy live project-data layout,
- local ignored state,
- provider bootstrap wiring.

## State commands

Observed current `MJ_DIR`/legacy-path coupling exists across:

```text
check.sh
checkpoint.sh
context.sh
decision.sh
finish.sh
handover.sh
history.sh
question.sh
search.sh
session.sh
start.sh
watch.sh
```

Do not update path strings independently if a centralized state-path helper can
make callers path-agnostic.

## Project/plan commands

Relevant:

```text
project.sh
project.awk
plan.sh
```

Move canonical records to `.ai/repo/project/` without changing DAG/status
semantics.

Do not add authored `status`.

## Prompt command

`lib/prompt.sh` currently means reusable repository prompt assets.

Target source:

```text
.ai/repo/prompts/
```

Do not confuse this command with new raw local prompt history:

```text
.ai/local/prompts/
```

Majordomus must not claim it observes provider prompts unless an integration
actually supplies them.

## Doctrine

Relevant:

```text
lib/doctrine.sh
lib/doctor.sh
share/doctrines.yaml
```

Treat current doctrine registry as a migration source until portable rule
objects prove full parity.

Do not delete registry first.

## Knowledge

Relevant:

```text
lib/knowledge.sh
share/knowledge-sources.yaml
test/cases/64_knowledge_discovery.sh
```

Preserve the Git-index/declared-source approach.

## Site/docs projection

High-coupling areas include:

```text
scripts/generate-site-data
scripts/site-check
docs/RESPONSIBILITIES.yaml
docs/CLAIMS.yaml
docs/CLI.md
docs/SCHEMAS.md
docs/CONCEPTS.md
README.md
examples/
site/data/generated/
```

The generated site currently parses some architectural content from provider
body/projection sources. Repoint generation to canonical `.ai` metadata/rules
instead of reproducing another prose authority.

## Tests

The current suite contains many direct path assertions.

Do not apply a blind global `.majordomus -> .ai` replacement because:

```text
.majordomus/
```

remains a valid optional TOOL INSTALL location in the target architecture.

Each occurrence must be classified as:

```text
legacy project data
new optional installation
migration fixture
historical documentation
```

before replacement.
