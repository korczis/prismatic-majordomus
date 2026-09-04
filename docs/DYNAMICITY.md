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
| Command | `share/commands.yaml` | validated against the dispatch table | docs, pages, demos, coverage |
| Event type | `share/events.yaml` | registry walk | ledger validation, `history`, `docs/SCHEMAS.md` |
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

`share/events.yaml` declares every name the ledger accepts, the command that writes it,
and the payload keys that name requires. `mj_ledger_append` refuses an unregistered name
and one missing a required field; `history --event` refuses to filter on a name that is
not declared, rather than answering with the empty result a declared-but-absent name would
give; `history --validate` reports a stored line whose name no reader recognises. Before
that, a mistyped name produced a durable record that every reader silently ignored, and
`docs/SCHEMAS.md` documented a `bootstrap` event that nothing had ever written.

Two constraints shaped the work:

- **Existing names are kept.** Renaming breaks every ledger that exists, including this
  repository's own, and a reader that silently omits everything before a rename is worse
  than an inconsistent name. The vocabulary is registered as written.
- **Ledger order is load-bearing.** `mj_record_rank` uses ledger position to break
  resolution ties between records created in the same second. Nothing may reorder,
  rewrite, or rotate lines without accounting for it.

Rendering is the one place a per-event branch is still allowed to exist, because how an
event reads to a person is presentation rather than vocabulary. There is exactly one such
branch, in `lib/history.sh`, and it is reconciled against the registry rather than removed:
an event with no rendering, and a rendering for an event that is not declared, both fail.

The one absent event still worth adding is a refused `finish`: an accepted completion is
recorded and a refusal leaves no trace, so a task refused four times looks exactly like one
that passed first try. `test/cases/32_refusal_lifecycle.sh` performs four refusals and
carries the assertion that will hold when the event exists.

## Demonstrations are projections, not descriptions

A command's page carries an interactive demonstration. The risk in that is obvious: a page
that describes behaviour becomes a second implementation, and the second one is never the
one that ships.

So the demonstration is not written. `test/fixtures/commands/<command>.json` holds a set of
scenarios — a situation, the command run, the expected exit code, the lines the output must
contain, and an explanation. `test/cases/34_command_fixtures.sh` executes every one of them
against the real binary in a fresh repository, and the site generator inlines the same file
into the page. What the page shows is what a test asserted.

Two consequences worth stating:

- **No shell lives in the fixture.** Preparation is a real script under
  `test/fixtures/commands/setup/`, composed by sourcing, which the page displays verbatim
  and the case sources. Nothing executes a string that came from data, which keeps the rule
  against `eval` intact and makes the demonstration honest at the same time.
- **The generator refuses rather than degrades.** A public command with no fixture, no
  demonstration id, no behavioural case or no negative case stops the build. There is no
  "tests coming soon" state for a command the project claims to support.

Every refusal is raised before a byte of output is written. An `exit` inside a
`{ ... } | jq . > file` group kills only the group, while the redirect has already truncated
the target — so a refusal written there reports the wrong exit code and leaves a ruined
file behind. The validation runs first; the emitting pipeline cannot fail on policy.

The same fixtures serve a second surface. A responsibility's page shows the command that
acts on it refusing and accepting, drawn from that command's fixture — so the demonstration
on `/supervises/finish/` and the one on `/commands/finish/` are the same scenarios, executed
by the same case, and cannot disagree.

The arrangement earns its keep. Writing the fixtures found a defect nothing else had:
`rev-parse HEAD` in a repository with no commits prints the literal string `HEAD` on stdout
and *then* fails, so the `|| printf 'NONE'` fallback appended to it and produced an identity
field with an embedded newline — a permanently corrupt line in an append-only ledger, for
anyone who ran `init` and `update` before their first commit.

## A link check must walk both directions

A rule that walks from a page to the things it links can only find links that are *wrong*.
It cannot find links that are *missing*, because a missing link is invisible from that side
by construction — there is nothing on the page to check.

The defect that made this concrete: six claims in `docs/CLAIMS.yaml` declared
`responsibility: plan`, the `/supervises/plan/` page linked none of them, and every check
looked outward from the page. Walking from the claim's side — every claim that names a
responsibility must appear on that responsibility's page — finds it immediately.

So a relation is checked in both directions or it is not checked:

- every id a page names must exist, and
- every record that names the page must be linked from it.

The second is the one nobody writes, and it is the one that catches the defect.

There are two such rules today, `why` and `supervises`, each knowing one join. That is
tolerable. A third is the point at which the relation itself should become data — the two
collections, the field holding the ids, and the route shape — and one rule should walk it.
Writing a third hand-rolled version would be the same mistake as the map this section's
defect came from.

## A mutation must prove it mutated

A test that breaks something and then checks for the failure is only as good as the break.
`sed` is silent when its pattern matches nothing: it exits 0 having changed nothing, and the
case goes on to assert that a no-op produced no failure, which is trivially true. The case
stays green and proves nothing, and it does so from the moment the code it was mutating
changed shape.

So every probe asserts it took effect before asserting what it caused:

```sh
sed 's/<pattern>/<replacement>/' "$SRC" > "$PROBE"
grep -q '<the replacement>' "$PROBE" || { echo "    the probe did not take"; exit 1; }
```

Two refinements, both learned the hard way in this repository:

- **An absence needs the inverted assertion.** `sed '/x/d'` leaves no replacement to find,
  so the guard is that the thing is gone rather than that something new is there.
- **Match by shape, not by contents.** A probe that names today's command list stops
  mutating the day someone adds a command. That is the same defect as a validator that
  silently stops running, and `35_future_command` had it within an hour of being written —
  its guard is what caught it.

A mutation that silently stops mutating and a rule that silently stops being enforced are
the same failure, and this repository exists because of the second one.

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
