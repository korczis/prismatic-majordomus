# Documentation index

| Document | Audience | Purpose |
|---|---|---|
| [`DESIGN.md`](DESIGN.md) | humans and AI workers | the v0.1 specification: problem, thesis, models, boundaries, what is intentionally absent |
| [`CLI.md`](CLI.md) | implementers, AI workers | every command: behaviour, reads, writes, exit-code contract, target output |
| [`SCHEMAS.md`](SCHEMAS.md) | implementers, AI workers | every file: schema, a concrete example, which command reads and writes it |
| [`EXTRACTION_REPORT.md`](EXTRACTION_REPORT.md) | humans | how the design was derived: root cause, pattern ledger, rejected patterns, risks, plan |
| [`../AGENTS.md`](../AGENTS.md) | AI workers and contributors | the operating contract for changing this repository |
| [`../CONTRIBUTING.md`](../CONTRIBUTING.md) · [`../SECURITY.md`](../SECURITY.md) | contributors | process; security commitments and stated limits |

Reading order for a new contributor: root `README.md`, then `DESIGN.md`, then `CLI.md`
and `SCHEMAS.md` if implementing, then the pattern ledger in `EXTRACTION_REPORT.md` for
any decision that seems arbitrary.

Rule for these documents: a sentence describing a capability is either backed by a test
in `test/cases/` or phrased as a target. When implementation and document disagree, the
document changes in the same commit.
