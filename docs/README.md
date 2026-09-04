# Documentation index

## Documents

Each of these is rendered on the website as well, from this same Markdown.

| Document | Audience | Purpose |
|---|---|---|
| [`DESIGN.md`](DESIGN.md) | humans and AI workers | the v0.1 specification: problem, thesis, models, boundaries, what is intentionally absent |
| [`CLI.md`](CLI.md) | implementers, AI workers | every command: behaviour, reads, writes, exit-code contract, target output |
| [`SCHEMAS.md`](SCHEMAS.md) | implementers, AI workers | every file: schema, a concrete example, which command reads and writes it |
| [`CONCEPTS.md`](CONCEPTS.md) | everyone | the vocabulary, and the two outcomes people confuse |
| [`DOCTRINE.md`](DOCTRINE.md) | everyone | what rules are enforced, what enforces each one, how the wiring is verified, and what was deliberately left out |
| [`CONTINUITY.md`](CONTINUITY.md) | everyone | how work survives the session doing it: the durable records, why transcripts are not state, how context is selected and records resolved |
| [`PLANNING.md`](PLANNING.md) | everyone | milestones as executable outcome specifications, issues as execution contracts, the dependency graph, derived status, execution waves, evidence, and the projections |
| [`ROADMAP.md`](ROADMAP.md) | everyone | the graph between milestones: identity against version, the gate that makes a dependency real before the next step starts, derived ordering, claim linkage, and how a milestone is added |
| [`DOGFOODING.md`](DOGFOODING.md) | contributors, AI workers | the one rule: Majordomus cannot recommend a development discipline it does not use itself — and what following it costs |
| [`ADOPTION.md`](ADOPTION.md) | teams | day one, week one, several workers, removal |
| [`ECONOMICS.md`](ECONOMICS.md) | leads | the claim it refuses to make, what v0.1 controls without measuring, where the cost actually is, what the ledger alone can measure, and what honest measurement would take |
| [`EXTRACTION_REPORT.md`](EXTRACTION_REPORT.md) | humans | how the design was derived: root cause, pattern ledger, rejected patterns, risks, plan |
| [`GITHUB_PAGES_ARCHITECTURE.md`](GITHUB_PAGES_ARCHITECTURE.md) | maintainers | how the website is derived from the repository and checked for drift |
| [`SITE_REVIEW.md`](SITE_REVIEW.md) | maintainers | route audit, ownership of site facts, validation performed |
| [`../AGENTS.md`](../AGENTS.md) | AI workers and contributors | the operating contract for changing this repository, generated from `.majordomus/` |
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
| [`RESPONSIBILITIES.yaml`](RESPONSIBILITIES.yaml) | implementers | the responsibilities as data, one entry each: title, README key, owning command, files, implementation, and the CLI and schema anchors each one documents. Hand-edited. Read by the site generator and by the doctrine registry |
| [`SITE_CLAIMS.md`](SITE_CLAIMS.md) | everyone | the same matrix as a Markdown table, generated from `CLAIMS.yaml` by `scripts/generate-site-data`. Never edited |
| [`PLAN_STATUS.md`](PLAN_STATUS.md) | everyone | where the plan stands right now — milestones, issues, waves and what is ready — generated from `.majordomus/project/` by `scripts/generate-site-data`. Never edited; the semantics are in `PLANNING.md` |
| [`GITHUB_PAGES_ARCHITECTURE.md`](GITHUB_PAGES_ARCHITECTURE.md) | site contributors | how the website is projected from this repository, which files are generated, and how the sync guarantee is enforced |

Rule for these documents: a sentence describing a capability is either backed by a test
in `test/cases/` or phrased as a target. When implementation and document disagree, the
document changes in the same commit. Every file in `docs/` appears in one of the two
tables above; `scripts/generate-site-data` fails when one does not.
