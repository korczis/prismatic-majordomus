# Canonical ownership and derivation

How this repository decides where a fact lives, what may be written down twice, and what
must be derived. The companion file is [`HARDCODING_LEDGER.yaml`](HARDCODING_LEDGER.yaml),
which records every place the rules below are currently broken, with a reproduce command
for each.

Statements here describing behaviour that is not yet implemented are phrased as targets.

## The rule

> One fact has one canonical owner. Every other place that fact appears is derived from
> the owner, or is a check that the two agree.

The failure this prevents is not untidiness. It is the failure the tool exists to catch:
a declared rule that nothing enforces, a documented command that no longer exists, a
count that was true once. Every such defect begins as one fact written in two places and
one of them changing.

## What counts as hardcoding

Not every constant is a defect, and a sweep that treats them alike produces unreadable
indirection while missing the real drift.

**A contract stays literal.** These are decisions, not discoverable facts, and writing
them down is how they become a contract:

| Kind | Example here |
|---|---|
| Protocol constants | the exit codes in `lib/common.sh` |
| Schema versions | `version: 1` in every canonical file |
| Standard filenames | `.majordomus/policy.yaml`, `CLAUDE.md` |
| Bootstrap defaults | the default profile name |
| A support decision | the operating systems in the CI matrix |
| A schema itself | `share/allow/*.txt`, which define the permitted keys |
| Editorial judgement | the audience and purpose columns in `docs/README.md` |

**A mirror is a defect.** A list is a mirror when something else already knows its
contents:

| Mirror of | Where the truth is |
|---|---|
| filesystem contents | the directory |
| CLI commands | the dispatch table |
| tests | `test/cases/` and the metadata in each case |
| routes | the generated content collections |
| doctrines | `share/doctrines.yaml` |
| providers | the declared projections |
| claims | `docs/CLAIMS.yaml` |
| counts of any of the above | the thing being counted |

The distinguishing question is not "could this be data?" but "does something else
already know this, and will the two disagree?" A curated list whose completeness is
enforced against the directory it describes is not a mirror — `docs/README.md` is the
model: the rows are editorial, and `scripts/generate-site-data` refuses to build when a
file in `docs/` appears in no row.

## Canonical owners

| Entity | Canonical owner | Discovery | Derived surfaces |
|---|---|---|---|
| Policy | `.majordomus/policy.yaml` | schema in `share/allow/policy.txt` | provider projections |
| Profile | `.majordomus/profiles/*.yaml` | directory glob | context, finish requirements, site |
| Doctrine | `share/doctrines.yaml` | registry walk | `check`, `finish`, `doctor`, `watch`, site |
| Validator | `mj_validate_*` in `lib/` | source scan, reconciled against the registry | doctrine dispatch |
| Claim | `docs/CLAIMS.yaml` | registry walk | guarantees pages, `docs/SITE_CLAIMS.md` |
| Responsibility | `docs/RESPONSIBILITIES.yaml` | registry walk | site, doctrine registry |
| Document | `docs/*.md` + the index tables | directory glob, index enforced | site routes |
| Command | *target:* `share/commands.yaml` | validated against the dispatch table | usage, docs, pages, demos, coverage |
| Event type | *target:* `share/events.yaml` | registry walk | ledger validation, `history`, context |
| Projection | `.majordomus/policy.yaml` `projections[]` | policy walk | `update`, `doctor`, `watch` |
| Context provider | *target:* one table in `lib/context.sh` | registry walk | assembly, budget dropping, JSON |

The rows marked *target* are the entities that have no owner today. Each is a row in the
ledger with the evidence for why it needs one.

## Derivation, not duplication

A derived surface is regenerated from its owner and is never authority. The three
generated surfaces here are provider instruction files (`majordomus update`), the site's
data and content (`scripts/generate-site-data`), and `docs/SITE_CLAIMS.md`. Each is
fingerprinted or diffed against a fresh generation, so a hand edit is a reported failure
rather than a silent divergence.

Two properties every generator must have:

- **Transactional.** Render to a temporary location, validate the whole result, then
  replace. A generator that truncates its output and then discovers an error has
  destroyed the last good state. `majordomus update` does this; the site generator
  renders to a temporary directory and compares.
- **Complete freshness.** The check that a generated artifact is current must iterate
  what the generator produced, not a list someone wrote next to it. The ledger's
  `generated-json-inventory` finding is this property missing.

## Events

A durable state change is an event. The ledger is append-only and already the canonical
record; the target is a registered vocabulary over it, not a second log.

Nine event names are written today and none is registered, so a typo produces a durable
record that every reader ignores and `history --event` cannot distinguish an unknown name
from one that has not occurred. The target is `share/events.yaml` declaring each type with
its required payload fields, `mj_ledger_append` refusing an unregistered name, and
`history` refusing to filter on one.

Two constraints on that work:

- **Existing names are kept.** Renaming breaks every ledger that exists, including this
  repository's own. The vocabulary is registered as written.
- **Ledger order is load-bearing.** `mj_record_rank` uses ledger position to break
  resolution ties between records created in the same second. Nothing may reorder,
  rewrite, or rotate lines without accounting for it.

The one absent event worth adding is a refused `finish`: today an accepted completion is
recorded and a refusal leaves no trace, so a task refused four times looks exactly like
one that passed first try.

## Self-governance

Majordomus supervises this repository with the same registries it ships. There is no
self-specific inventory and no test-only code path: the doctrines that run here are the
doctrines in `share/doctrines.yaml`, and the wiring `doctor` reconciles is the wiring
declared in `.majordomus/policy.yaml`.

The property this is meant to produce, stated as a target: adding a command, doctrine,
provider, event type, projection or claim requires defining the entity, implementing the
behaviour, and adding the tests — and every other surface follows without a maintainer
remembering a secondary registration step. Where that is not yet true, the ledger says
so.

## How to add one of each

Short by design. If a list below grows, the architecture has regressed.

- **A doctrine** — add the entry to `share/doctrines.yaml` and write its
  `mj_validate_<validator>` function. `doctor` reconciles the two and fails if either is
  missing.
- **A claim** — add the entry to `docs/CLAIMS.yaml` and write `docs/claims/<id>.md`. The
  site generator refuses to build without the detail page, and a guaranteed claim with no
  test path is an error.
- **A document** — write `docs/<NAME>.md` and add its row to one of the two tables in
  `docs/README.md`. The route, the page and the index entry follow.
- **A profile** — add `.majordomus/profiles/<name>.yaml`. It is discovered by glob and
  validated by `doctor`.

Adding a command, an event type, a provider or a projection is not yet this short. Those
are the ledger's priority 1 and 2 rows.
