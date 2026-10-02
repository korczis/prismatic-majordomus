# Operator decisions and non-negotiable invariants

These decisions are approved target architecture.

## D1 — Majordomus distribution is stateless relative to managed repositories

Majordomus implementation/distribution contains:

```text
CLI
libraries
parsers
validators
schemas
migrations
default skeleton
provider adapter defaults
standard rule package
tests
product documentation
```

It must be usable read-only.

No mutable managed-repository state may live inside the Majordomus distribution.

## D2 — `.majordomus/` is not project data

`.majordomus/` may exist in a consumer repository only as one optional
installation form:

- Git submodule
- symlink
- local checkout
- vendored read-only distribution

It may also not exist at all because `majordomus` can come from PATH/global
packaging.

No command may require repository project data under `.majordomus/`.

## D3 — `.ai/` is the project-specific portable AI layer

`.ai/` is provider-neutral/polyglot.

It MUST remain intelligible without Majordomus installed.

It MUST contain no Majordomus implementation code.

## D4 — `.ai/repo/` is tracked canonical repository context

Tracked in Git and shared across users/checkouts as repository truth.

Candidate/approved domains:

```text
policy
profiles
rules
prompts
skills
workflows
knowledge declarations/curated records
ADRs
project planning model
repo-specific templates when genuinely customized
```

## D5 — `.ai/local/` is Git-ignored local state

Approved local domains:

```text
.ai/local/prompts/            timestamped raw local USER prompt history
.ai/local/cache/
.ai/local/session-contexts/
.ai/local/state/
```

`.ai/local/**` MUST NOT be implicitly loaded into model context.

`.ai/local/**` MUST NOT be published by docs/site generators.

`.ai/local/**` MUST NOT alter repository normative policy.

Do not create `.ai/local/rules/`.

## D6 — state ownership moves local

Current `.majordomus/state/**` moves conceptually to:

```text
.ai/local/state/**
```

This includes current task, ledger, checkpoints, handovers, sessions, local
decisions, open questions, archive/completion records, and similar operational
records unless a record is explicitly promoted to repository-shared knowledge.

Durable architecture decisions belong in:

```text
.ai/repo/adrs/
```

Do not automatically promote every local decision to an ADR.

## D7 — `.ai/repo/` is better terminology than `.ai/shared/`

Use `repo`, not `shared`, for repository-tracked context.

## D8 — Majordomus baseline rules use vendoring

Repository rule structure:

```text
.ai/repo/rules/
├── project/
└── vendor/
    └── majordomus/
```

The vendored package is:

- tracked,
- versioned,
- self-contained,
- readable without executable Majordomus,
- treated as read-only by agents,
- upgraded explicitly,
- diffable.

The installed Majordomus binary MUST NOT silently change effective repository
rules.

## D9 — no rule override machinery in transformation v1

Preserve the current doctrine property that there is no hidden override system.

For the first transformation:

```text
effective rules = vendored Majordomus baseline + project rules
```

Project rules may add constraints. They do not silently weaken/replace baseline
rules.

Design the schema so explicit override policy can be introduced later, but do
not build policy federation now.

## D10 — rule objects are portable Markdown with machine metadata

Rules use Markdown plus YAML frontmatter.

Generic rule semantics must be understandable without Majordomus.

Majordomus-specific enforcement metadata belongs under a namespaced extension
such as:

```yaml
x-majordomus:
```

## D11 — discovery is manifest/graph driven, never blind recursion

`.ai/README.md` explains the protocol.

Majordomus discovers registered sources deterministically.

Rules resolve dependencies as a graph.

Missing dependency and cycle are failures.

`.ai/local/**` is excluded from implicit discovery.

## D12 — `AGENTS.md` becomes a bootloader

`README.md` mentions `AGENTS.md`.

`AGENTS.md` backlinks to `README.md` and tells the agent to read
`.ai/README.md`.

Normative rule corpus must not be duplicated into `AGENTS.md`.

Provider files are thin adapters/bootstrap surfaces.

## D13 — `majordomus init` initializes repository AI data, not the tool install

`majordomus init` creates/extends `.ai/`.

It does not install Majordomus.

It does not silently mutate `.envrc`.

Shell/package installation is a separate concern.

## D14 — skeleton is a seed, not remote authority

The distribution ships a default `.ai` skeleton.

After `init`, repository-owned files under `.ai/repo/**` belong to the
repository.

A Majordomus executable upgrade must not overwrite them.

Schema/default upgrades require explicit migration/vendor-update operations.

## D15 — Prismatic boundary remains absolute

Prismatic may inspire architecture but no direct dependency, code coupling,
runtime service, internal API dependency, shared mutable state, or deployment
coupling may be introduced.
