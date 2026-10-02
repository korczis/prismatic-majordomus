# Migration DAG and commit plan

Execute phases in dependency order. Each phase should end in a coherent commit
with focused tests.

Do not squash everything into one giant commit while developing.

## P0 — preflight and evidence snapshot

Before mutation:

```text
git status
git log -n ...
bin/majordomus doctor
bin/majordomus plan validate
bash test/run.sh
```

Record actual results under ignored:

```text
tmp/transform/_run/BASELINE.md
```

If full suite is too slow, run it anyway where possible and record any
environmental timeout separately from a failing test.

Never "fix" pre-existing unrelated dirty work by resetting it.

Commit: none.

---

## P1 — central path model

Introduce explicit separation:

```text
tool distribution root
repository .ai root
tracked .ai/repo root
local .ai/local root
state root
project root
policy file
profiles root
prompt root
```

Temporarily support legacy path reads only where required to enable migration.
Do not add scattered fallback logic to every command.

Add tests proving the CLI can run from:

- normal PATH install/checkout,
- an alternate absolute path,
- optional repository `.majordomus/` install if practical.

Commit suggestion:

```text
refactor(paths): separate tool distribution from repository ai state
```

---

## P2 — create `.ai` protocol and skeleton

Add distribution skeleton/templates for:

```text
.ai/README.md
.ai/manifest.yaml
.ai/repo/README.md
.ai/repo/rules/README.md
.ai/repo/...
```

Add `.ai/local/` ignore behavior.

`init` on a fresh repository must create `.ai/`, not project-data
`.majordomus/`.

Preserve policy/profile/prompt formats initially.

Commit:

```text
feat(init): seed portable .ai repository layer
```

---

## P3 — migrate canonical repository data

Move with minimal semantic change:

```text
.majordomus/policy.yaml   -> .ai/repo/policy.yaml
.majordomus/profiles/     -> .ai/repo/profiles/
.majordomus/prompts/      -> .ai/repo/prompts/
.majordomus/project/      -> .ai/repo/project/
```

Rewire all readers, docs, schemas, tests, generated site source, examples,
catalogues, and path repro commands.

Do NOT redesign policy schema in this phase.

Commit:

```text
refactor(ai): move canonical repository policy into .ai/repo
```

---

## P4 — migrate operational state local

Move:

```text
.majordomus/state/ -> .ai/local/state/
```

Ensure `.ai/local/` is ignored.

Update all state readers/writers atomically.

Preserve:

- state consistency,
- scope,
- checkpoints,
- handovers,
- sessions,
- decisions,
- questions,
- ledger,
- retention,
- overlap reporting,
- context assembly.

Update claims/docs where tracked-via-Git durability is no longer true.

Add fresh-clone and worktree behavior tests.

Commit:

```text
refactor(state): make operational ai state checkout-local
```

---

## P5 — bootstrap/provider inversion

Turn `AGENTS.md` into a thin bootloader.

Ensure README ↔ AGENTS discovery.

Create `.ai/README.md` protocol.

Make CLAUDE/GEMINI/etc thin provider adapters.

Keep deterministic update/region support and hand-edit protection.

Delete monolithic generated operating-contract duplication only after equivalent
portable sources exist.

Commit:

```text
refactor(context): make provider files thin .ai bootstraps
```

---

## P6 — standard rule package

Create tool-distribution canonical source:

```text
share/standard/rules/
```

Convert all current `share/doctrines.yaml` entries to rule objects with
`x-majordomus` enforcement metadata.

Seed vendored copy under:

```text
.ai/repo/rules/vendor/majordomus/
```

for new repositories and dogfood the same layout in Majordomus itself.

Add manifest, version, provenance, fingerprints/hash validation as needed.

Do not yet delete `share/doctrines.yaml` until parity is proven.

Commit:

```text
feat(rules): add portable vendored rule package
```

---

## P7 — flip doctrine authority

Rewire doctrine/rule resolution so effective rule objects are the semantic
authority.

Recreate every doctrine wiring diagnostic against the new rule format.

Then remove `share/doctrines.yaml` as an independent authority.

If a generated compatibility projection is useful temporarily, it must be
clearly derived from rule objects and checked stale by CI.

Commit:

```text
refactor(doctrine): derive enforcement from portable rule objects
```

---

## P8 — provider-body decomposition

Classify old `providers/body.md` content:

- normative rule → `.ai/repo/rules/...`
- lifecycle procedure → `.ai` protocol/workflow
- reusable operation → skill/workflow candidate
- repository fact → tracked repo context
- generated summary → projection
- obsolete duplication → delete

Do not create a "misc" bucket.

Remove provider body once nothing authoritative remains there.

Commit:

```text
refactor(context): remove monolithic provider operating body
```

---

## P9 — knowledge integration

Preserve the existing M003 principle:

```text
canonical Git sources -> declared discovery -> compiled/retrievable knowledge
```

Move repository-specific source declarations to:

```text
.ai/repo/knowledge/sources.yaml
```

as appropriate.

Keep tool-specific source classes/schema in the distribution if they are not
repository context.

Put rebuildable compiled knowledge/cache under `.ai/local/cache/` unless a
tracked artifact has a documented guarantee requiring persistence.

Do not add embeddings/vector DB/semantic hallucinated edges.

Commit:

```text
refactor(knowledge): anchor repository knowledge in the portable ai layer
```

---

## P10 — projection provenance cleanup

Resolve old:

```text
.majordomus/generated/fingerprints.yaml
```

without weakening hand-edit protection.

Prefer self-describing managed target hashes if robust.

Otherwise store minimal tracked projection provenance under `.ai/repo` and
document it as derived-but-committed evidence.

Test:

- ordinary update,
- changed policy,
- hand-edited generated target,
- region mode,
- fresh clone,
- explicit force.

Commit:

```text
refactor(projection): detach provenance from legacy .majordomus state
```

---

## P11 — legacy migration command/path

Provide explicit migration for existing repositories.

Requirements:

- detect legacy `.majordomus/policy.yaml`,
- distinguish optional new tool distribution,
- preview changes,
- refuse ambiguous mixed layout,
- preserve unknown files,
- migrate canonical files,
- migrate local state,
- update gitignore,
- update bootstraps,
- validate after migration,
- be idempotent / report already migrated.

CLI can be `majordomus migrate` or a carefully designed `init --migrate`.
Prefer a dedicated migration command if it keeps `init` semantics clean.

Commit:

```text
feat(migrate): upgrade legacy repository layout to .ai
```

---

## P12 — docs/site/claims/cross-surface reconciliation

Update:

```text
README
CONTRIBUTING
CLI
SCHEMAS
CONCEPTS
DESIGN
DOCTRINE/rules docs
CONTINUITY
DOGFOODING
ADOPTION
examples
site generators
site data
claims
responsibilities
GitHub projection
roadmap docs
```

No stale `.majordomus` project-data claims may remain except documentation of
optional tool installation or legacy migration.

Generated artifacts are regenerated, not hand-edited.

Commit:

```text
docs(ai): document portable repository layer and stateless tooling
```

---

## P13 — final hardening

Run:

```text
bash test/run.sh
bin/majordomus doctor
bin/majordomus plan validate
site checks
shellcheck if available
git diff --check
```

Search for stale semantics:

```text
rg '\.majordomus/(policy|profiles|project|prompts|state|generated|providers)'
```

Every remaining occurrence must be one of:

- legacy migration documentation/test fixture,
- optional tool installation path,
- historical source deliberately preserved,
- explicit negative test.

No live project-data reader may remain.

Dogfood `.ai/` in Majordomus itself.

Commit:

```text
test(ai): prove legacy separation and portable context guarantees
```
