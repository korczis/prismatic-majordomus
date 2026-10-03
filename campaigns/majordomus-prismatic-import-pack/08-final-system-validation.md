# Prompt 08 — Final System Validation and Architectural Proof

This is the final adversarial review.

Assume previous prompts may have produced plausible-looking but incomplete work.

Try to break it.

## 1. Re-read repository doctrine

Re-read `AGENTS.md`, rules/doctrines and changed architecture.

Confirm the implementation itself complies with the principles it claims to enforce.

## 2. Compare donor and target again

Re-scan `~/dev/prismatic-platform` and the Prompt 01 migration matrix.

For every approved item, verify its final disposition:

- imported;
- adapted;
- reimplemented;
- merged.

For every rejected/reference-only item, verify it did not sneak in accidentally.

## 3. Zero-registration proof

Using canonical test fixtures or a temporary entity:

- add a skill;
- add a doctrine if feasible;

prove that applicable target surfaces discover them automatically.

No edits to independent consumer inventories are allowed.

## 4. Fresh-clone mindset

Check for reliance on:

- donor checkout at runtime;
- uncommitted generated files;
- local caches;
- developer-specific paths;
- globally installed ad-hoc scripts;
- hidden environment state.

The target must be self-contained according to its documented prerequisites.

## 5. Run canonical validation

Use actual project commands to run all relevant:

- formatters;
- builds;
- unit tests;
- integration tests;
- schema checks;
- docs checks;
- generated drift checks;
- API/OpenAPI checks;
- MCP tests;
- Cockpit tests;
- CLI tests;
- repository gates.

Do not invent replacement commands if repository tooling already defines them.

## 6. Inspect diff and architecture

Search for:

- duplicated lists;
- parallel registries;
- hardcoded skill/doctrine names in consumers;
- dead compatibility code;
- new shell domain logic;
- unexplained generated blobs;
- stale docs;
- TODOs deferring required migration.

Fix what you find.

## 7. Final acceptance criteria

The work is complete only if:

- selected Prismatic concepts are imported based on evidence;
- Majordomus remains canonical owner;
- all imported entities are schema-valid and typed/discoverable where applicable;
- provenance exists;
- rules/doctrines are actually enforced;
- legacy duplicates are removed;
- relevant CLI/API/OpenAPI/MCP/Cockpit/docs surfaces derive from canonical data;
- a new entity requires no consumer-by-consumer registration;
- tests prove this;
- CI/gates enforce it;
- docs explain extension and troubleshooting;
- runtime does not require `~/dev/prismatic-platform`;
- no secrets/machine-local state were imported.

## Final report

Produce:

### Source audit outcome
What was actually selected from Prismatic Platform.

### Rejected material
What was deliberately not imported and why.

### Canonical architecture
Where skills/doctrines/rules/provenance now live and how discovery works.

### Migration
What legacy Majordomus state was consolidated or deleted.

### Enforcement
Which gates/rules/tests prevent regression.

### Surface integration
Exact relevant CLI/API/OpenAPI/MCP/Cockpit/docs integration.

### Evidence
Commands run and results.

### Zero-registration proof
Show the representative propagation test.

### Remaining debt
Only genuine remaining limitations with concrete reasons.

Do not declare victory because files exist. Prove the system behaves as designed.
