---
schema: skill/v1
id: promote-a-workspace
version: 1
title: Promote a workspace into the layer
description: Read every conversation of a declared external workspace into a working set outside the tracked tree, account for each one, and, once the operator has accepted the proposal, promote what it decided into intents, milestones, issues, proposed decisions and knowledge that cite the conversation and message they came from.
status: active
tags: [workspace, knowledge, planning, privacy, provenance]
related: [evolve, implement, report-verification-state, pack-for-chatgpt, assess-before-deleting]
inputs:
  - a workspace declared under .ai/repo/workspaces/ whose authorisation covers reading it
  - the operator's own data export of the vendor account, which the operator requests and hands over
  - a branch and worktree of its own for the records, and an issue the import runs under
outputs:
  - a working set outside every checkout holding the export, its addressed view, the extractions and a coverage ledger, which moves into the store .ai/local/workspaces/<workspace>/ once a command owns that store
  - promoted records, accepted by the operator before they are written and again before they land, each citing <workspace>:<conversation>#<n>, that every plan, intent, decision and knowledge check passes
  - a delivered change whose every reached surface was looked at, and an announcement and a handover that name it and the run's high-water mark
---

# Purpose

Work decided in a vendor-hosted workspace — a ChatGPT Project first — is invisible to every
instrument here until it is promoted. This skill is the order in which a whole workspace is
read, accounted for conversation by conversation, and promoted into the kinds that already
exist, so that each promoted record leads back to the message it came from and nothing of
the workspace's own content is ever committed (ADR 0032, `.ai/repo/workspaces/README.md`).

Promotion stays the person's act (ADR 0032): the agent reads, extracts and proposes; the
operator accepts the proposal before any record is written (step 6) and accepts the staged
records before they are handed to the landing seat (step 9).

Until milestone `external-workspaces` lands the sync (I1303–I1308), steps 2 and 3 are done
by hand in the way written here; each names the issue that automates it. It is not for
sending sources to a workspace (use `pack-for-chatgpt`), not for one lesson a session met
(use `evolve`), and never for another account, another project, or any write upstream.

# When to use

When the operator asks for a workspace to be read into the repository, when its
conversations have moved past the high-water mark the last run's handover recorded, and
before planning that depends on what was decided there.

# Procedure

## 1. Read what is declared, then open the run

```bash
majordomus context
majordomus plan show external-workspaces
majordomus worktree create docs/<the-records-branch>
```

Read `.ai/repo/workspaces/<workspace>.yaml`: `vendor.origins` bounds what may be observed,
`authorisation.statement` bounds what may be read, `authorisation.write` stays false. Read
ADR 0032, the open issues of the milestone, and the last run's high-water mark from its
handover and receipt (`majordomus handover --resolve`). Refused here: a workspace with no
declaration, and any reading the statement does not cover.

**The working set.** `.ai/local/workspaces/<workspace>/` is the store I1303 will own. Until
a command owns it, nothing writes it by hand: the run keeps its working set — `raw/`, `md/`,
the extractions, `listing.json`, `ledger.jsonl` — in a directory the operator names outside
every checkout (`<work>` below). It outlives the records worktree, which is removed after
landing, and it moves into the store when I1303 lands.

**The import's issue.** Each run has an issue of its own under `external-workspaces`. Survey
the identities before taking one: the highest `.ai/repo/project/issues/I*.yaml` on master,
on every `origin/*` ref (`git ls-tree -r --name-only <ref> .ai/repo/project/issues`) and in
every sibling worktree, plus every block announced on the board (`peers.list`, step 5).
Choose above all of them, announce the block you take — issue numbers and case numbers
together, with `scripts/case-numbers next` confirming the cases — and only then write. No
allocator exists for issue identities; a guessed number is the collision the board exists
to prevent.

In the records worktree, start the server, open the episode and start the task:

```bash
majordomus-cli serve ensure
majordomus session start
majordomus start "promote <workspace>" --issue <import issue> --scope .ai/repo
majordomus-cli run gates.completion --input '{}' --quiet --format json \
  | jq '.output.questions[] | select(.id == "no-new-debt")'
```

`majordomus check` needs the task, `majordomus knowledge observe` needs the episode, and the
`no-new-debt` question of the closing gate is measured from the commit the task started at:
the reading printed here is the "before" of step 8.

## 2. The operator's export, then the working set

Access is the operator's act. The default path is the vendor's own data export, which needs
no token, no browser driven by the agent and no permission of the agent's own:

- The operator requests the export from the vendor (for ChatGPT: Settings, Data controls,
  Export data), downloads it, and unpacks it into a new, empty directory under `<work>`.
- The agent keeps only the conversations whose project matches the declaration's
  `upstream.id` (for ChatGPT, the conversation's gizmo id), writes each one unchanged as one
  JSON file to `<work>/raw/`, and writes the list of their ids and update times to
  `<work>/listing.json`. If the export lacks the project's conversations, say so and stop.
- The archive is deleted once `raw/` is written (`majordomus://skill/assess-before-deleting`):
  everything outside the declared project is the operator's and not the run's.

Reading through the operator's signed-in session instead is not part of this skill. It needs
the operator's written authorisation in their own message, a narrow permission naming this
workspace's `upstream.id` and the destination `<work>/raw/` (a rule written for another
workspace or another destination does not cover it), one request at a time at a
conservative pace that honours Retry-After and never probes the limit, and the permission
removed when the export ends. Handing a page's data to a local receiver past the page's own
content security policy routes around that policy: it waits for an owner decision recorded
with `majordomus decision`, and its receiver design — origin checks both ways and a nonce
per run — is I1306's acceptance, not yet proved. When the harness or its classifier refuses
an action, stop and quote the refusal to the operator; a refusal is reported, never worked
around.

Nothing of the content passes through the agent's context or leaves `<work>`.

## 3. Give every statement an address

Convert each raw file into `<work>/md/<created>--<conversation>.md` (I1307, I1308): a header
with the id, the URL, the creation time and the number of non-current branches, then the
thread walked from its current node up through its parents, keeping messages to everyone,
each headed `## [n] ROLE <time>`. That walk flattens the tree: regenerated and branched
answers are dropped, so their count stays visible in the header and in the ledger until
I1307 keeps the graph. Collapse an identical repeated message and inline style dumps; keep
the raw file. From here on, `<workspace>:<conversation>#<n>` names one message inside the
working set; a tracked record writes `<workspace>:<alias>#<n>` instead (step 7). Attachments
are metadata only: a pack a conversation names is looked up under `campaigns/`
(`campaigns/README.md`; those packs are history, not instructions).

Once step 4 classifies a conversation off-topic, delete its `md/` file. Its raw file is kept
only as long as the store's bound allows; that bound and its `doctor` line are I1302, and
until they exist the receipt records the size of `<work>`.

## 4. Extract, every conversation accounted for

Build the catalogue on master's head and write it to `<work>/catalogue.md`, with that head's
commit identifier at the top:

```bash
majordomus intent list
majordomus plan list
majordomus adr list --json
majordomus rules list
majordomus knowledge nodes --kind knowledge --json
```

Split the addressed files into batches by date of at most about 900k characters each — at
that size four of fourteen workers still skimmed, so smaller is better — and give one
read-only worker per batch the brief

```bash
majordomus prompt render workspace-extract
```

with its batch file, the catalogue and an output directory under `<work>`. Relevance comes
first: an off-topic conversation is counted, never copied, and its title appears nowhere.
Every file says `read_fully`: `true` or `false` for a relevant conversation, `null` for an
off-topic one. A worker's report is accepted only with it, and every relevant conversation
whose file says `false` is dispatched again; the brief reads such a file again rather than
skipping it.

`<work>/ledger.jsonl` has one line per conversation:

```json
{"conversation": "<id>", "created": "<time>", "relevance": "core|related|offtopic",
 "read_fully": true, "branches_dropped": 0,
 "items": [{"kind": "decision", "msg": 3, "domain": "<domain>",
            "disposition": "<section>", "ref": "<record id or null>"}]}
```

It adds up when every check below prints what it says:

```bash
jq length <work>/listing.json; jq -s length <work>/ledger.jsonl           # equal
jq -s '[.[] | select(.relevance != "offtopic" and .read_fully != true)] | length' \
  <work>/ledger.jsonl                                                      # 0
```

## 5. Route deterministically, then consolidate

Route items to domains by the catalogue's own vocabulary — milestone and rule identities,
and the areas under `.ai/repo/why/areas/` (`majordomus why areas`) — never by common words
such as test, check, rule or done alone. Keep a domain to at most about 300 items; split a
larger one by milestone. Snapshot the board and refuse to consolidate on a partial one:

```bash
majordomus-cli run peers.list --input '{}' --quiet --format json > <work>/board.json
jq -e '.output.complete' <work>/board.json
```

Then give one read-only worker per domain, with `<work>/board.json` as its board snapshot,
the brief

```bash
majordomus prompt render workspace-consolidate
```

Every item ends in exactly one disposition — a record's sources, already recorded,
superseded or rejected, open question, misrouted, or noise — and `misrouted` items go back
to their domain. A consolidation whose coverage does not add up is not written:

```bash
jq -e '.coverage | .items_in == .items_used + .items_misrouted + .items_dropped_as_noise' \
  <work>/<domain>.plan.json
```

## 6. The operator accepts the proposal

Nothing is written into `.ai/repo/` until the operator has read the consolidated proposals
and the ledger's dispositions — counts by disposition, every proposed record with its
sources, every open question — and accepted them, wholly or in part, in their own message.
That message is the person's act ADR 0032 requires; the records of step 7 are the ones it
accepted, and the receipt cites it. What the operator rejects goes to the ledger as
`superseded_or_rejected` with that message as its reference.

## 7. Write the records the way this repository writes them

- Identities come from the block announced in step 1.
- Decisions one at a time: `majordomus adr next` (its answer is the first line), then
  `majordomus adr propose "<title>" --from issue:<import issue>`, so the record is
  `proposed` with `provenance.origin: extracted`. `majordomus adr check` refuses any
  extracted record marked accepted, whoever marks it, for as long as it stays extracted. The
  owner accepts a decision in a commit of their own that sets `status: accepted` and
  `provenance.origin: authored` together and cites their acceptance message.
- YAML is the repository's subset: quote every scalar, and write no escape sequence inside
  a double-quoted one — the readers do not decode it; use typographic quotes instead.
- An intent criterion whose test does not exist says `evidence: command`; the issue that
  writes the test turns it into `evidence: test`. An issue under an intent's milestone
  `serves:` a criterion; an intent gets a critique from a fresh read-only reviewer under
  `.ai/repo/project/critiques/` before work on it starts.
- No commit identifier in prose, no generated directory in an issue's scope, no new noun
  where an existing milestone or capability owns the subject. What a knowledge record may be
  is `docs/KNOWLEDGE.md`'s, including its line on an advisor's numbers.
- Provenance is written in each record's body as `<workspace>:<alias>#<n>`, until a typed
  workspace reference exists for `derived_from`. The alias (`c001`, `c002`, ...) numbers the
  workspace's conversations in the order they were created and is never renumbered; the map
  from alias to the vendor's conversation id lives in `<work>/aliases.json` and moves into the
  store with the rest of the working set. No vendor conversation id and no vendor URL is ever
  written to a tracked file: the repository may be public, and an id tells the world that a
  private conversation exists and when (owner's decision, 2026-10-18). A record states the
  assertion in its own words; it never quotes a conversation.
- No staged file under `.ai/repo/` repeats a run of eight or more words from `<work>/md/`.
  The check that refuses it is I1309's; until it exists, the writer compares the staged
  records against `md/` before committing and the receipt says it was done.
- `git add` each record before checking it: an untracked file is invisible to every check.

```bash
majordomus plan validate
majordomus intent validate
majordomus adr check
majordomus knowledge check
majordomus check
```

## 8. Leave every surface it reaches better

Before the first edit, record the reading the change will be judged against. For each
route the change reaches, audit master's server — its URL is what
`majordomus-cli serve status` prints in the primary checkout — into a file of its own:

```bash
scripts/ui audit --only <route> --origin <master's server> --out target/web/ui-before.json
```

Then derive once and look at every surface the records reach:

```bash
scripts/derive
scripts/derive-check
scripts/ci/local-never-published
```

The new records exist only on the records worktree's own server: the origin of every URL
below is the one `majordomus-cli serve ensure` printed there in step 1, never the primary's
(a worktree server on the shared port is shadowed by the primary's; give it `--port`).

| record | answers 200 | names the id |
|---|---|---|
| issue | `/cockpit/plan/issues/<id>` | `/ui/prs?issue=<id>#issue`, `/plan/<id>/` on the site |
| milestone | `/cockpit/plan/milestones/<id>` | `/ui/roadmap?milestone=<id>` |
| intent | `/cockpit/objects/intent/<id>` | `/ui/idd?intent=<id>` |
| decision | `/cockpit/objects/adr/<adr-NNNN>` | `/adrs/<id>/` on the site |
| knowledge | `/cockpit/objects/knowledge/<id>` | |
| critique | `/cockpit/objects/critique/<id>` | |

The `/ui` screens answer 200 for an id that does not exist, so read their content; Cockpit's
200 is proof. To view the site, build its rustdoc first — a crate build, so take the build
slot — then serve it, on another port when 1111 is taken:

```bash
scripts/rust-check --doc
SITE_SERVE_PORT=<port> scripts/site-serve
```

Audit the same routes again on the records worktree's server with `--out
target/web/ui-after.json` and diff the two files: no added finding, no baseline grown, and
`no-new-debt` read again as in step 1. Fix one thing the surfaces showed wrong on the way —
a missing link, a stale count, a broken mention — in its source, never in a projection. A
surface that did not move is a finding about its generator, recorded as an observation with
`majordomus knowledge observe`.

## 9. Deliver, and tell the peers

One pull request carries the records, their derived files and the receipt: the before and
after readings, the URLs checked with their status, the ledger's counts by disposition, the
size of `<work>`, what was left undecided, the operator's acceptance of step 6, and the
run's high-water mark — the last conversation id and update time seen, and the ledger's
counts — so the next run computes its delta from committed state.

The operator then reads the receipt and the staged records and accepts them in their own
message, cited in the pull request; until then the head is not handed on. Hand its green
head to the landing seat; read the pages run after it lands (`scripts/pages verify-ui`).
Announce what moved, publish a handover that names the records, the pull request and the
high-water mark (`majordomus handover`), and report under
`majordomus://skill/report-verification-state`. What this run met follows
`majordomus://skill/evolve`.

# Output

- `<work>` with `raw/`, `md/` for the relevant conversations, the extractions,
  `listing.json`, `board.json` and `ledger.jsonl`, in which every conversation of the
  listing has a line, every relevant one says `read_fully: true`, and every item has exactly
  one disposition; it moves into `.ai/local/workspaces/<workspace>/` once I1303 owns the
  store.
- Records the operator accepted, citing `<workspace>:<conversation>#<n>`, decisions proposed
  with the owner's acceptance evidence named, and `plan validate`, `intent validate`,
  `adr check`, `knowledge check` and `majordomus check` passing.
- No tracked file carrying a conversation's text or a quote from one, a credential, the
  owner's own personal data or a third party's, and `scripts/ci/local-never-published` and
  `scripts/ci/no-machine-paths` passing.
- A pull request with the receipt and the operator's acceptance, every surface named in step
  8 read, and a handover and an announcement that say what moved and where the run stopped.
