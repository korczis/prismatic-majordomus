# 5. One projection plan over the canonical inputs, named owners for every truth, and the site as a view of the registry

Status: accepted, 2026-09-05. Extends ADR 2 and ADR 4.

## Context

ADR 2 and ADR 4 made every interface of the Rust executable a projection of one
capability registry and reconciled the generated documents under `docs/generated/` with
`generate --check`. Two projections every worker meets before anything else were still
outside that pipeline. The provider bootstraps, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`,
were rendered only by the shell tool's `update` and judged only by the shell `doctor`'s
stamp comparison; CI did not compare them with the policy, so a rule typed into
`AGENTS.md` by hand would have merged. The site had no view of the registry at all, and a
page that showed one would have had to hand-maintain a second list of operations and
kinds. The operator asked for one compiled control plane: canonical inputs, a
deterministic registry, and every provider file, document and page a one-way projection
of it, with a manual edit of any projection failing the build rather than warning.

## Decision

- **Owners are named, and there are two canonical domains, not one directory.** The
  tool distribution owns the grammar: `share/kinds.yaml` says what a kind is and how it is
  read, `share/schemas/*.schema.json` what its metadata must satisfy,
  `share/providers/*.tmpl` how a provider bootstrap reads. The repository owns the
  sentences: `.ai/repo/**` holds its rules, policy, profiles, prompts, ADRs, claims and
  knowledge, and may add kinds, schemas and templates of its own without redefining the
  distribution's. `.ai/local/**` is checkout state, never a source. There is no
  `.majordomus/` directory in a supervised repository: the bootstraps already say that one
  is, if present, an installation of the tool and not context, and the invariant that the
  same commit and the same Majordomus version yield the same registry puts the grammar
  with the version, not with the commit. A repository pins the tool; it does not vendor
  the grammar.
- **The policy and the templates are canonical inputs of the provider projections.**
  `apps/majordomus-cli/src/policy.rs` reads `.ai/repo/policy.yaml` and the profiles
  through the same subset reader the shell tool uses; `providers.rs` renders each
  `projections[]` target from its template, prefixes a file-mode target with the stamp
  (policy hash, content hash) and splices a region-mode target between its markers,
  leaving the host document alone. The bytes are the shell tool's bytes, stamp included,
  so the two agree in both directions (`test/cases/93_rust_provider_projections.sh`). A
  declaration that cannot be produced is refused with the reason and nothing is written:
  a target outside the repository, a provider without a template, a token the policy
  cannot fill, an `always_loaded` target over the budget.
- **One plan assembles every artifact.** `generate::plan(app, targets)` is the one
  function behind `majordomus generate` and `generate --check`: the OpenAPI document, the
  reference and module pages, the registry manifest and the benchmark matrix from the
  registry; the allow-lists from the schemas; the provider bootstraps from the policy and
  the templates; the site's registry dataset from the registry and the index. Nothing else
  assembles artifacts, and `--check` compares the whole plan byte for byte and exits 10
  naming each stale file. CI runs it in the validate workflow and in the Pages workflow,
  so neither a merge nor a deploy proceeds from a stale projection.
- **The site renders the registry; it does not describe it.** `site/data/registry/
  registry.json` (`majordomus-site-registry/v1`) carries the registry's fingerprint and
  counts, every builtin capability with its exposures, every module, the index's
  fingerprint and every object of the layer without its content, every kind with its
  schema, and the declared provider projections. `/registry/` lays it out. No timestamps,
  no absolute paths, no git state, so the same tree gives the same bytes. The directory is
  the Rust executable's alone; the shell site generator owns `site/data/generated/`
  wholesale and reports a file it did not write as an orphan, so no directory has two
  writers.
- **Generation flows one way.** Nothing reads a bootstrap, a generated document, the site
  dataset or an MCP listing back into `.ai/`. A projection is regenerated or refused,
  never merged upstream.
- **A canonical change reaches exactly the projection it concerns.** A rule added to the
  layer moves the index fingerprint, the registry fingerprint and the site dataset and
  leaves the bootstraps alone; a policy change moves the bootstraps and leaves the OpenAPI
  document alone. `tests/projections.rs` follows both mutations through `plan`, sees
  `check` name the one stale artifact each time, regenerates, and sees a second plan
  produce the same bytes.

## Ownership matrix

| truth | canonical owner | projections (never edited) |
|---|---|---|
| what a kind is, how it is read, its schema | `share/kinds.yaml`, `share/schemas/` (repository additions under `.ai/repo/knowledge/`) | `share/allow/*.txt`, the index, the site's kinds table |
| the repository's rules, policy, profiles, prompts, ADRs, claims, knowledge | `.ai/repo/**` | every MCP resource, `majordomus context`, the bootstraps, the site |
| how a provider bootstrap reads | `share/providers/*.tmpl` (repository override under `.ai/repo/providers/`) with `.ai/repo/policy.yaml` | `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, ... |
| every executable operation, its schemas, exposures, cache and benchmark policy | `capability!` and `module!` declarations under `apps/majordomus-cli/src/capability/builtin/` (ADR 4) | MCP tools, HTTP routes, OpenAPI, Swagger UI, the `capabilities` commands, `docs/generated/*`, the benchmark targets, the site's capability table |
| the registry's identity for this tree | the fingerprint the registry computes | the executor's cache key, `site/data/registry/registry.json` |
| claims, plan, roadmap, documents shown on the site | `docs/CLAIMS.yaml`, `docs/claims/*.md`, `docs/*.md`, the plan records | `site/data/generated/*.json` through `scripts/generate-site-data` |

## Shims, with their removal criteria

- **`majordomus update` (shell)** remains the interactive writer with `--dry-run`,
  `--diff` and `--force`, and the shell `doctor` still compares a target with its stamp.
  Both render the same bytes as `generate providers`; case 93 fails the moment they
  diverge. It goes when the shell tool's lifecycle commands move to the executable and
  `doctor` calls `generate --check` instead of comparing stamps itself.
- **`scripts/generate-site-data` (shell)** still derives the site's claims, plan, roadmap
  and document data from the files that own them. It goes, section by section, as the
  index learns those kinds; the registry dataset is the first section that never had a
  shell version.
- **Block tokens** (`{{PROFILE_TABLE}}`, `{{FINISH_CONTRACT}}`, `{{REQUIRED_SECTIONS}}`)
  are still rendered by the shell `update` and refused by name by the executable. No
  shipped template uses them; they are removed from the shell tool when its `update` goes.

## Alternatives rejected

- **A `.majordomus/` directory in every supervised repository carrying the grammar.**
  Fifty repositories would carry fifty copies of forty schemas, which would diverge, and
  the distribution's kinds would no longer be the distribution's. The manifest already
  exists as `.ai/manifest.yaml`, in YAML; changing its format for the sake of a diagram
  is churn.
- **The site dataset inside `site/data/generated/`.** The shell generator replaces that
  directory wholesale and reports orphans; two writers in one directory is the drift this
  decision removes.
- **Reading `docs/generated/registry.json` from the site instead of a second dataset.**
  Zola reads data from the site directory only, and that manifest deliberately carries no
  fingerprint and no declarative object because it must change when the code changes and
  not when a document is added; the site needs both. The builtin summaries appear in both
  files, derived by the same code from the same registry: two projections, one source.
- **Filling an unset token with an empty string**, as the shell tool does. A bootstrap
  that reads "the checkpoint interval it sets is ``" is a defect of the policy; the
  executable refuses it with the token's name.
- **Refusing to overwrite a hand-edited bootstrap**, as `update` does without `--force`.
  `generate` is the gate's remedy and rewrites the projection; the edit is visible in the
  diff and in `--check`'s refusal, which is where it should be judged.

## Consequences

Editing a rule, a claim or an ADR now changes the site's registry dataset, and the change
must be regenerated and committed with it, exactly as `docs/generated/` already required
of a capability change; `just generate` does both, and CI names the stale file. A
bootstrap is never edited: the policy or the template is, and `majordomus generate
providers` is run. The site has one page that cannot disagree with the executable.
