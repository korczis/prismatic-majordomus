# Documentation index

## Documents

Each of these is rendered on the website as well, from this same Markdown.

| Document | Audience | Purpose |
|---|---|---|
| [`DESIGN.md`](DESIGN.md) | humans and AI workers | the v0.1 specification: problem, thesis, models, boundaries, what is intentionally absent |
| [`TWO_FOLDER_CONSOLIDATION.md`](TWO_FOLDER_CONSOLIDATION.md) | humans and AI workers | what the repository writes into a tree today, verified against the code and the running binary, and the ordered slices that reduce it to `.ai/**` and `.majordomus/**` with reversible bridges |
| [`CLI.md`](CLI.md) | implementers, AI workers | every command: behaviour, reads, writes, exit-code contract, target output |
| [`SCHEMAS.md`](SCHEMAS.md) | implementers, AI workers | every file: schema, a concrete example, which command reads and writes it |
| [`CONCEPTS.md`](CONCEPTS.md) | everyone | the vocabulary, and the two outcomes people confuse |
| [`CONTEXT.md`](CONTEXT.md) | everyone | scoped context: what a context document is, how the effective context for a path is composed and ordered, what wins in a conflict, how a change is traced to the documents it affects, and the sync check |
| [`DOCTRINE.md`](DOCTRINE.md) | everyone | what rules are enforced, what enforces each one, how the wiring is verified, and what was deliberately left out |
| [`CONTINUITY.md`](CONTINUITY.md) | everyone | how work survives the session doing it: the durable records, why transcripts are not state, how context is selected and records resolved |
| [`PLANNING.md`](PLANNING.md) | everyone | milestones as executable outcome specifications, issues as execution contracts, the dependency graph, derived status, execution waves, evidence, and the projections |
| [`PERFORMANCE.md`](PERFORMANCE.md) | contributors, AI workers | where a command's time goes (`MJ_TIMING=1`), what was slow and the shape of every fix, `majordomus bench` and its cold and warm distributions, local runs against the tracked baseline, the regression check and the budgets, and how to work on performance |
| [`ROADMAP.md`](ROADMAP.md) | everyone | the graph between milestones: identity against version, the gate that makes a dependency real before the next step starts, derived ordering, claim linkage, and how a milestone is added |
| [`DOGFOODING.md`](DOGFOODING.md) | contributors, AI workers | the one rule: Majordomus cannot recommend a development discipline it does not use itself — and what following it costs |
| [`INSTALL.md`](INSTALL.md) | everyone | installing the tool: the one-line installer, pinning a version, choosing where it goes, the supported platforms, upgrading, uninstalling, CI use, the security model and troubleshooting |
| [`DISTRIBUTION.md`](DISTRIBUTION.md) | contributors, AI workers | how the tool is packaged, published and installed: the one canonical model, what derives from it, the trust path, adding a platform, releasing, and recovering from a bad release |
| [`RELEASE.md`](RELEASE.md) | everyone, maintainers | the release: the changelog composed from the layer's release records, the decisions dated inside each release's window and the conventional commits in its range; the two statements of the version and the one command that writes them; the bump the commits imply and the evidence for it; and every surface derived from both |
| [`CATALOGUE.md`](CATALOGUE.md) | everyone | the use-case and application registries: what they are for, how they differ from the why pages, the schema, and how to extend them |
| [`ADOPTION.md`](ADOPTION.md) | teams | day one, week one, several workers, removal |
| [`ADOPTION_FIRST_RUN.md`](ADOPTION_FIRST_RUN.md) | maintainers | forensic finding, closed: what `doctor` reported in a repository that is not this one, why the findings tracked the tool's own distribution rather than the adopting repository, and the release stamp that now scopes them |
| [`ENTRY.md`](ENTRY.md) | everyone, AI workers | entering the repository: what converges on its own and what is asked for, the three commands of the shared server, what each standing means, what is deliberately not automatic and why, and what to run when it does not come up |
| [`ENTRY_AUDIT.md`](ENTRY_AUDIT.md) | maintainers, AI workers | forensic finding, 2026-09-10: what converges on its own when a person or an agent enters this repository — executable, shared server, episode, peer, continuation — and what does not; the root causes ranked, the matrix, the dependency graph, and the plan by stage |
| [`ECONOMICS.md`](ECONOMICS.md) | leads | the claim it refuses to make, what v0.1 controls without measuring, where the cost actually is, what the ledger alone can measure, and what honest measurement would take |
| [`EXTRACTION_REPORT.md`](EXTRACTION_REPORT.md) | humans | how the design was derived: root cause, pattern ledger, rejected patterns, risks, plan |
| [`COMMANDS.md`](COMMANDS.md) | contributors, AI workers | the command graph: the three declarations it composes, stable identities, the effect model and the exposure policy derived from it, the generated workflow bridge and its compatibility aliases, the one completion engine every surface asks, repository entry, and what adding a command costs (nothing) |
| [`DYNAMICITY.md`](DYNAMICITY.md) | maintainers | canonical ownership: what may be written down twice, what must be derived, and which entities still have no owner |
| [`COCKPIT_IDE_AUDIT.md`](COCKPIT_IDE_AUDIT.md) | maintainers, AI workers | forensic finding, 2026-09-10: what the Cockpit would need in order to become a development surface — the governance preflight and its two defects, the two-program split, the system run end to end with every command and response recorded, the gap matrix, the drift audit, and the phased plan with the failures that must be fixed first |
| [`DEVELOPMENT_RUNTIME.md`](DEVELOPMENT_RUNTIME.md) | maintainers, integrators, AI workers | the development runtime: which program owns the semantics of the plan, tasks, sessions, context, executions, peers, evidence and completion, the measured inventory of every one of them, the storage each is decided to keep, the two event tiers, the derivation chain anything exposed must follow, what each surface may and may not decide, and the ranked gaps between that boundary and the code |
| [`CAPABILITIES.md`](CAPABILITIES.md) | everyone, integrators | the Rust executable's capability model: one definition, and MCP, HTTP, OpenAPI, Swagger UI, the command line and the generated reference derived from it; what is canonical, how to extend it, how it fails |
| [`USE_CASES.md`](USE_CASES.md) | everyone, AI workers | executable use cases: one file each under `.ai/repo/use-cases/`, the scenario that proves it against the tool, evidence, observed maturity, coverage gated by policy, impact analysis, scaffolding, and what the site derives from it |
| [`PRODUCT.md`](PRODUCT.md) | everyone, maintainers | the product features as objects of the layer: what a feature file may hold, what every surface derives from it, the public projection boundary and its allow-list, what the homepage and the `/features/` pages render, and what is refused |
| [`WHY.md`](WHY.md) | everyone | the operational failure modes this tool answers, as objects of the layer: the three kinds, the flow from one Markdown file to every projection, what is authored and what is derived, and what enforces each |
| [`SCOPE.md`](SCOPE.md) | everyone, AI workers | the repository scope: what a worker reads and what it never reads, declared once in `.ai/repo/scope.yaml`, how a path is judged (name, size, content), what the executable does with it, and `majordomus scope` |
| [`EXECUTIONS.md`](EXECUTIONS.md) | everyone, integrators | the execution plane: running a capability as work that can be watched — the model, the lifecycle, the typed event protocol, the live channel and its guarantees, what each interface offers, the limits and what happens at each, and what it deliberately does not do |
| [`COCKPIT.md`](COCKPIT.md) | everyone, integrators | the Cockpit: the registry rendered as pages for a person, what makes it a projection rather than a dashboard, the graph and health models, the browser layer and what happens without it, the security decisions, the asset pipeline |
| [`WEB.md`](WEB.md) | everyone, integrators | the web surface: every surface discovered from its producer rather than registered, the two reserved namespaces (`/docs` is documentation, `/swagger` is Swagger UI), what a surface declares, how to add one, how the documentation is built for its mount and served safely, and what is enforced where |
| [`ENVIRONMENT.md`](ENVIRONMENT.md) | everyone, contributors | the repository environment: one typed snapshot of what a checkout is — project, layer, version control, toolchains, workflows, provider projections, local services — with a provenance entry for every value; the full and the fast resolution, the cache under `.ai/local/`, and the direnv adapter that renders it without reading the repository itself |
| [`WORKTREES.md`](WORKTREES.md) | everyone, AI workers | the branch-to-worktree topology: `<repo>-wt/<branch>` derived from git identity and never registered, the standings and diagnostic codes, the commands, the lifecycle, the layered enforcement, the fingerprint-verified migration, failure modes and recovery |
| [`MCP.md`](MCP.md) | everyone, MCP clients | the read-only MCP surface of the Rust executable: what it serves, what decides that, how it fails, what it refuses to serve |
| [`MESH.md`](MESH.md) | everyone, operators | the mesh: how running Majordomus instances discover each other — Ed25519 node identity, one signed bounded envelope over multicast, broadcast and rendezvous, one registry, deny-unknown trust, the off-by-default declaration, the threat model, and how to operate and extend it |
| [`MODELS.md`](MODELS.md) | everyone, AI workers | the model catalogue: vendors and models declared once in share/models.yaml with provenance and no fabricated facts, and the explainable routing that names why every candidate was chosen or excluded |
| [`DESIGN_SYSTEM.md`](DESIGN_SYSTEM.md) | everyone, contributors | the design system: the one declaration every surface is rendered from, the pipeline from `share/design/tokens.yaml` to the site, the Cockpit and the executable's own pages, how to add a role, a status word or a type step without touching a consumer, how drift is caught, and how to read a page that looks wrong |
| [`UI.md`](UI.md) | maintainers, contributors | UI conformance: the page set and the width set discovered rather than listed, the responsive, semantic, component and WCAG invariants a browser checks over every page, what the build normalises in markup it did not write, and where the report is |
| [`GITHUB_PAGES_ARCHITECTURE.md`](GITHUB_PAGES_ARCHITECTURE.md) | maintainers | how the website is derived from the repository and checked for drift |
| [`QUALITY.md`](QUALITY.md) | contributors, AI workers | what the Rust crate's exported surface is held to: documentation that says more than the signature, an executable example on everything that carries behaviour, a module boundary something exercises, and every command of the command line accounted for against the capability registry — the codes, the ratchet, and the golden path for adding a command |
| [`CI.md`](CI.md) | contributors, AI workers | how a change is validated: the validation workflow over repository-owned gates, the planner and its model of what can affect what, the gates and how to run each locally, the caches and artifacts, the executable as a build output, the parallel suite and probe, the platform policy, and where the measurements live |
| [`GITHUB_PAGES_PERFORMANCE.md`](GITHUB_PAGES_PERFORMANCE.md) | maintainers, contributors | how a push becomes public: the publication path separated from the gates that decide merging, controlled against external latency, the derived trigger, the input fingerprint that replaces a generation, the caches and what each is worth, the budgets and where they live, how publication itself is measured, and the bottlenecks that remain |
| [`PAGES_STATUS.md`](PAGES_STATUS.md) | maintainers, contributors | the current numbers behind that document: the controlled budgets, the paths that start a publication, and the last measured build-and-check path per platform. Generated from `.ai/repo/ci/pages.yaml` and `.ai/repo/benchmarks/pages/`; never edited |
| [`SITE_REVIEW.md`](SITE_REVIEW.md) | maintainers | route audit, ownership of site facts, validation performed |
| [`../AGENTS.md`](../AGENTS.md) | AI workers and contributors | the agent bootstrap for this repository, generated from `.ai/repo/policy.yaml`; the contract itself is under `.ai/` |
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) · [`../SECURITY.md`](../SECURITY.md) | contributors | process; security commitments and stated limits |

Reading order for a new contributor: root `README.md`, then `DESIGN.md`, then `CLI.md`
and `SCHEMAS.md` if implementing, then the pattern ledger in `EXTRACTION_REPORT.md` for
any decision that seems arbitrary. `CLAIMS.yaml` is the shortest honest answer to "what
does this actually do today?".

## Reference files

In the repository only. The website presents the same content through its own routes, so
these carry no site route of their own.

| File | Audience | Purpose |
|---|---|---|
| [`CLAIMS.yaml`](CLAIMS.yaml) | everyone | the canonical claims matrix: every capability with its status, source, implementation, and test. Hand-edited. Rendered on the website's guarantees page |
| [`RESPONSIBILITIES.yaml`](RESPONSIBILITIES.yaml) | implementers | the responsibilities as data, one entry each: title, README key, owning command, files, implementation, and the CLI and schema anchors each one documents. Hand-edited. Read by `scripts/generate-site-data`, which builds each responsibility page from it and refuses to publish when an entry names a path, an anchor, a command or a README row that does not exist |
| [`HARDCODING_LEDGER.yaml`](HARDCODING_LEDGER.yaml) | maintainers | every place a fact is written down twice: authority, copies, risk, and the reproduce command for each. Hand-edited, read alongside `DYNAMICITY.md` |
| [`SITE_CLAIMS.md`](SITE_CLAIMS.md) | everyone | the same matrix as a Markdown table, generated from `CLAIMS.yaml` by `scripts/generate-site-data`. Never edited |
| [`PLAN_STATUS.md`](PLAN_STATUS.md) | everyone | where the plan stands right now — milestones, issues, waves and what is ready — generated from `.ai/repo/project/` by `scripts/generate-site-data`. Never edited; the semantics are in `PLANNING.md` |
| [`GITHUB_PAGES_ARCHITECTURE.md`](GITHUB_PAGES_ARCHITECTURE.md) | site contributors | how the website is projected from this repository, which files are generated, and how the sync guarantee is enforced |
| [`generated/artifacts.md`](generated/artifacts.md) · [`generated/openapi.json`](generated/openapi.json) · [`generated/registry.json`](generated/registry.json) · [`generated/capabilities.md`](generated/capabilities.md) · [`generated/cli.md`](generated/cli.md) · [`generated/benchmarks.md`](generated/benchmarks.md) | integrators | the generated tree of the Rust executable, written from its own registry by `majordomus generate` and checked by `generate --check`. `generated/artifacts.md` is the index of the whole set and is itself generated — read it rather than this row for what exists: every document is written in every encoding this repository commits it in (JSON for a program, YAML beside it, Markdown for a reader), from one value, each declaring its schema and its source. Never edited. `just derive` regenerates every derived file of the repository in order and `just derive-check` names every stale one (`GITHUB_PAGES_ARCHITECTURE.md`) |

Rule for these documents: a sentence describing a capability is either backed by a test
in `test/cases/` or phrased as a target. When implementation and document disagree, the
document changes in the same commit. Every file in `docs/` appears in one of the two
tables above; `scripts/generate-site-data` fails when one does not.
