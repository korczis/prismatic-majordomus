---
name: workspace-consolidate
description: turn one domain of a workspace's extracted items into a deduplicated proposal of records verified against the trunk and the peer board
---

# Turn one domain of a workspace's extracted items into a deduplicated proposal of records verified against the trunk and the peer board

This is the brief of the consolidation step of majordomus://skill/promote-a-workspace. The
conversations of one declared workspace have been extracted, each item carrying its source
`<conversation>#<n>`. For one domain, turn those items into a deduplicated, verified
proposal of repository records — intents, milestones, issues whose `depends_on` edges
derive the waves, decision records and knowledge notes — that a writer puts into
`.ai/repo/` of {{REPOSITORY}}. Every proposed record lists the sources it came from.

## Inputs

- `<domain file>`: JSON lines, one item each (kind, source, conversation, created, themes,
  existing, item). Items were routed here by the catalogue's own vocabulary; one that
  belongs elsewhere goes under `misrouted` with the domain it belongs to.
- `<item ledger>`: every item of every domain, for finding related ones.
- The addressed conversations in `md/` of the run's working set (the store
  `.ai/local/workspaces/<workspace>/` once a command owns it): open one when an item is
  ambiguous or the owner's exact words matter.
- `<board snapshot>`: what the other sessions are doing now, as `peers.list` answered with
  `complete` true. Reference that work; never duplicate it.
- The repository at {{HEAD}}, read-only: `majordomus plan show <id>`, `majordomus intent
  show <id>`, `majordomus adr list`, and searching records and code are all allowed.

## How a record is proposed here

- **Verified against the trunk first.** The extractor's `existing` is a guess. Search
  records and code for every candidate. What the trunk already does goes to
  `already_recorded` with its identities; what it does in part becomes an issue that states
  only the remaining delta and cites what exists.
- **One record, all its sources.** The owner repeats a demand across weeks. A later
  conversation supersedes an earlier one, and an idea the repository later decided against
  (a decision record, a rule) goes to `superseded_or_rejected` with what replaced it.
- **One worker, one issue, one pull request.** An issue is an outcome one worker can finish
  and prove in one pull request. A large theme is a milestone with several issues and
  dependency edges; wave numbers are derived, never written.
- **Opinions are not facts.** What may be knowledge is `docs/KNOWLEDGE.md`'s, including
  its line on an advisor's numbers. A dated observation may be cited as dated inside a
  `why`. An unverified vendor claim is an open question.
- **Accepted is the owner's act.** A decision is accepted only with the owner's message that
  ordered, accepted or acted on it, and you cite it. Decision records are proposed
  regardless, and the whole proposal is read and accepted by the operator before anything
  is written.
- **No new nouns.** Extend the capability or milestone that already owns the subject and
  name it (`project.no-new-nouns`, `project.derived-once`).
- **The release posture holds.** New scope waits for the release the briefing names unless
  it unblocks it: mark each issue `release_blocker: true|false`.
- **The writer's constraints, applied now.** No commit identifier in any prose field; no
  generated directory (`docs/generated`, `site/content`, `site/data`) in an issue's scope; a
  criterion whose test does not exist yet says `evidence: command`, and the issue that
  writes the test is the one that turns it into `evidence: test`.
- **Style.** English; titles are present-tense assertions of the outcome, not tasks. Read
  three or four recent issue and milestone records and match their voice and fields.
- **Privacy.** No credentials, tokens, the owner's own personal data (contact details,
  accounts, location, health, finances, family, employer and clients) or any third party's
  personal data, and nothing from an off-topic conversation: write `[redacted]`. Never quote
  a conversation in a record: state the present-tense assertion in your own words and cite
  `<conversation>#<n>`; quotes stay in the working set.

## Output

Write one file, `<output file>`, as JSON. Write a first version early and rewrite it as you
go, so a stopped run keeps its work.

```json
{
  "domain": "<domain>",
  "themes": [{"id": "kebab", "title": "...", "summary": "...", "sources": ["conv#n"]}],
  "intents": [{"action": "new|amend", "id": "kebab", "title": "...", "statement": "...",
    "invariants": ["..."], "non_goals": ["..."],
    "criteria": [{"id": "kebab", "criterion": "observable",
                  "evidence": "test|claim|command|deployment", "ref_hint": "test path"}],
    "milestones": ["id"], "sources": ["conv#n"], "owner_acceptance": "conv#n or null"}],
  "milestones": [{"action": "new|extend", "id": "kebab", "title": "...", "priority": "p0-p3",
    "problem": "...", "outcome": "...", "current_state": "verified on the trunk",
    "desired_state": "...", "scope": ["paths"], "non_scope": ["..."],
    "acceptance_criteria": ["..."], "depends_on": ["id"], "risks": ["..."],
    "intent": "id or null", "sources": ["conv#n"]}],
  "issues": [{"key": "<domain>-NN", "milestone": "id", "title": "...", "slug": "kebab",
    "priority": "p0-p3", "profile": "implementation|deep-work|debugging|verification",
    "release_blocker": false, "objective": "...", "why": "with dated evidence",
    "current_state": "verified on the trunk, with paths", "desired_state": "...",
    "scope": ["paths"], "non_scope": ["..."], "acceptance_criteria": ["observable"],
    "validation": ["commands"], "evidence_required": ["case or test to be written"],
    "depends_on": ["keys or issue ids"], "serves": ["intent#criterion"],
    "overlaps": ["issue ids or board claims"], "sources": ["conv#n"]}],
  "adrs": [{"title": "the decision as an assertion", "context": "...", "decision": "...",
    "alternatives_rejected": ["..."], "consequences": "...", "related": ["..."],
    "owner_acceptance": "conv#n or null", "sources": ["conv#n"]}],
  "knowledge": [{"id": "kebab", "class": "fact|convention|constraint|lesson",
    "title": "assertion", "description": "when it applies",
    "epistemics": "observed|inferred|decided", "body": "3-8 sentences",
    "related": ["..."], "sources": ["conv#n"]}],
  "open_questions": [{"question": "...", "why_it_matters": "...",
    "who_decides": "owner|peers|experiment", "sources": ["conv#n"]}],
  "superseded_or_rejected": [{"statement": "...", "by": "ref", "sources": ["conv#n"]}],
  "already_recorded": [{"statement": "...", "refs": ["..."], "sources": ["conv#n"]}],
  "misrouted": [{"source": "conv#n", "belongs_to": "domain"}],
  "coverage": {"items_in": 0, "items_used": 0, "items_misrouted": 0,
               "items_dropped_as_noise": 0}
}
```

Every item of the domain file ends in exactly one place — a record's `sources`,
`already_recorded`, `superseded_or_rejected`, `open_questions` or `misrouted` — or is
counted as noise, and `coverage` adds up: `items_in` equals `items_used` (every item in a
record's `sources`, `already_recorded`, `superseded_or_rejected` or `open_questions`) plus
`items_misrouted` plus `items_dropped_as_noise`. Read-only everywhere except `<output file>`.

Answer at the end with the count of each section, the five most valuable new records, and
any conflict with work on the board or with an accepted decision.
