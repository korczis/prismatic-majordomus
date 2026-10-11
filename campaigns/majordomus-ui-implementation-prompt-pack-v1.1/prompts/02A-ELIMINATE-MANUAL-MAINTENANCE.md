# 02A-ELIMINATE-MANUAL-MAINTENANCE: Canonical inventory migration and enforcement

## Mission
Perform a REPOSITORY-WIDE retrospective elimination of manually synchronized, derivable inventories and consumer-specific duplicate domain metadata, before building further UI surfaces. This is an implementation milestone, not a code-style review. Execute in the *current checkout*, identify real files and working contracts; never pretend listed examples exist. Read and obey `contracts/GLOBAL_CONTRACT.md` and repository's governing instructions.

**Invariant:** One authoritative declaration of each domain fact. All downstream lists, counts, UI navigation, commands, schemas and docs that can be derived MUST be projections. Never ban legitimate human-authored primary domain declarations, explanatory documentation, intentional UX copy, security policy, or explicit ordering semantics that truly belong to the canonical model.

## 1. Evidence-first repository inventory
Search with code-aware methods for hardcoded arrays, navigation lists, command catalogs, provider tables, endpoint directories, feature switches, mock data, generated-but-committed copies, repeated enums/strings, frontend maps, Markdown indices, shell pipelines, manually counted totals and redundant configuration across:
- CLI clap/command metadata, command help/completions, `justfile`, `.just/**`, installer, `.envrc`, environment banner;
- `.ai/**`, `.majordomus/**`, rules, doctrines, skills, schemas, ADRs, knowledge and capabilities;
- Rust runtime, API routers, JSON Schema, OpenAPI/Swagger, MCP tools/resources/prompts;
- Cockpit and new UI navigation, routes, menu actions, cards, health/status, graph nodes/edges, dashboard KPIs;
- mesh peers/capabilities/tasks, intents, issues/milestones, sessions, evidence, test coverage, docs, releases and deployment metadata.
Produce a tracked evidence ledger using repo's existing typed evidence mechanism if available; otherwise a temporary auditable working document with rows:
`fact/entity | authoritative owner | duplicate consumer/file:line | how derived today | migration strategy | consumers | tests | severity | disposition`.
Classify every candidate: (A) canonical primary declaration KEEP; (B) duplicated derived material REMOVE/GENERATE; (C) unavoidable protocol mapping GENERATE + drift-test; (D) purposeful presentation copy KEEP with justification; (E) missing canonical source MODEL FIRST. Do not conflate manually maintained *source truth* with redundancy.

## 2. Typed canonical capability/operation registry
Find/reuse existing typed registry and capability ownership; do not invent a new registry if one exists. Canonical descriptors must have stable IDs, domains, typed input/output schemas, authorization/read-write classification, availability/reason, provenance, lifecycle and ordered user-facing presentation metadata where truly needed. API/OpenAPI, MCP and UI actions derive from one authoritative capability/operation definition or a validated adapter for genuine protocol differences. No frontend reimplementation of permission, lifecycle or business logic. Reject arbitrary shell execution. Avoid allowing auto-discovered operations to become executable without policy checks.

## 3. Navigation as a projection, not a second product catalog
Derive feature navigation and permitted actions from stable capability descriptors and live scoped repo state. Explicitly distinguish:
- capability exists / runtime available / authorized / currently enabled;
- empty data / unsupported integration / disconnected peer / error;
- visible UI entry versus executable action.
UI may retain ergonomic grouping and curated explanatory copy ONLY as canonical presentation metadata or deliberately authored templates. It may use presentation-only ephemeral ordering/filtering state. It MUST NOT maintain lists of supported domain entities independently. Preserve stable URL deep links and redirects on migration. On browser refresh/back/forward/mobile drawer, keep typed context and authorization semantics consistent.

## 4. Deterministic discovery and semantic ordering
Discover from canonical registries, normalized filesystem and runtime adapters. Canonicalize collection order once using stable normalized group/key/ID; define total deterministic fallback and identity tie-breaker. Never depend on `HashMap`/filesystem/concurrency iteration order. Natural numeric ordering where appropriate. Permit explicit user sorts without changing canonical default. Require identical ordered IDs for CLI JSON/API/MCP/UI models/generated documentation when domain semantics require the same order. If an endpoint contract treats arrays as sets, don't introduce gratuitous breaking ordering promises.

## 5. Eliminate legacy duplicates, not merely route around them
Migrate all in-scope consumers onto common typed projection/query. Delete now-unused arrays, ad-hoc parser logic, duplicate front-end enums, handwritten API inventories, redundant docs indexes, stale generated copies, and hardcoded dashboard metadata. If a consumer must be generated, wire generator into canonical build/check; generated files carry provenance and drift check. For injected consumer repositories, discover their own canonical state instead of displaying Majordomus self-repo data. Do not rewrite legitimate documentation prose.

## 6. Just, env, installer and command catalog
Where `just --dump --dump-format json` (or actually supported canonical CLI output) exists, introspect the parser result rather than regexp scraping. Discover module namespaces, aliases, recipe descriptions and platform-specific options. Do not regenerate `justfile` during every `cd`. Keep `.envrc` thin: no repository-domain business logic or network/build on entry. Environment snapshot, command suggestions and banner all share a typed source. Shell completion is generated from the CLI authority, not a second handwritten vocabulary. Respect install portability and absence of optional `just`.

## 7. Schema, API, MCP, docs and UI propagation
Apply producer-to-consumer lineage:
`canonical declarations and observations -> typed validated registry -> canonical ordered projections -> CLI / REST / OpenAPI / MCP / UI / docs / banner / completion`.
Generate schema from owner types where supported, and prove CI detects schema drift. API compatibility: version wire changes; use adapters only when required, with contract tests. The UI must not contain copied counts, sample peers, synthetic statuses, fictional progress or manually curated entities. Dynamic counts from actual sources, with a visible source/provenance and fresh/stale/unavailable distinction.

## 8. Automated anti-duplication enforcement
Integrate into existing `check`/governance gate rather than an ornamental linter. Implement a scoped structural validator using AST/schema/registry comparisons where possible, with allowlisted exceptions documented alongside canonical ownership. Avoid brittle blanket grep rules that mistake explanatory prose, fixtures or true primary declarations for duplicates. Detect:
- distinct manually curated copies of same canonical domain catalog;
- missing/unresolved canonical IDs, schema mismatch, duplicate IDs, deprecated registration points;
- generated artifacts out of sync, stale frontend constants, duplicate ordering inventories;
- new entity absent from eligible derived consumers;
- unauthorized auto-discovered executable operation;
- forbidden hardcoded counts/statuses in production.
Emit actionable diagnostics with file/line, owner, suggested migration and severity. Check must run locally + CI + relevant release gates; no opt-out that trivially bypasses it.

## 9. Mandatory proof: zero-registration extensibility
Create controlled temporary fixture entity in an appropriate canonical source for EACH actually supported representative kind (e.g. rule, doctrine, skill, provider, command, action, issue, peer capability). Assert that it propagates to every eligible output without touching the consumer implementation. Verify discovery after removing the entity too. Ensure entity definitions that require deliberate authorization do *not* auto-grant execution privileges. Test against (1) Majordomus self-repo, (2) injected project, (3) no optional integrations. Use fixtures rather than making dirty synthetic production records.

## 10. Regression and contract test matrix
- shuffled enumeration/insertion/concurrent completion yields byte-stable ordered IDs;
- CLI structured JSON, REST, MCP, UI projection, documentation inventory parity;
- frontend navigation automatically reflects newly available/removed capabilities;
- unknown/unauthorized actions fail closed and are never falsely offered;
- env/banner/command completion discover no manually synchronized lists;
- schemas/OpenAPI regeneration fails on unsynchronized wire artifacts;
- mobile navigation and deep-links survive migration;
- empty/disconnected/stale/unavailable states truthful;
- generation and discovery operate offline where local-first promised;
- performance budgets for `cd`/launcher do not regress;
- intentional primary declarations and authored explanatory text are not mistakenly flagged.
Add property-based or permutation tests where sensible, snapshot tests as render checks (not sources of truth), and cross-process tests for hash-order nondeterminism.

## 11. Migration execution plan
1. Establish inventory and classify cases A-E with evidence.
2. Fix highest leverage canonical owners and typed metadata.
3. Convert vertical consumer slices, one domain at a time; keep gates passing.
4. Delete obsolete paths and update cross-surface contract tests.
5. Add structural validator and CI gate, verify it catches a deliberately injected violation.
6. Run full repository quality gates, feature E2E in self + injected repo.
7. Update appropriate docs, ADR, schemas and generated projections from source.
8. Report precise before/after duplicated inventory counts, removed lines/files (with grounded measurement), runtime/capability safety checks, and reproducible zero-registration proof.

## Hard acceptance (all required)
- No manually synchronized duplicate catalog remains in the audited scope, except explicit justified exceptions.
- New canonical entity appears automatically in relevant readers without changes in UI/CLI/API/MCP/docs inventories.
- Canonical semantic ordering is stable, inferred/typed, and reusable; explicit user sort remains functional.
- No invalid capability becomes executable merely because it is discovered.
- `majordomus ui` still works in self and injected repositories.
- Structural anti-drift gate fails correctly when a duplicate inventory is reintroduced.
- Legacy duplicates are actually deleted, with compatibility and documentation managed.
- Exact test commands, exit codes and evidence are recorded. Blocked cases stay RED/blocked, not claimed done.

## Anti-goals
No new mega-registry containing manually copied lists; no consumer-specific `.sort()` fix; no blanket auto-generation of authorial prose; no duplicated JSON catalogs; no fake test data in production; no new shadow database; no AST checker that cannot identify the canonical owner; no arbitrary action executability; no broad unrelated rearchitecture justified as “deduplication”.
