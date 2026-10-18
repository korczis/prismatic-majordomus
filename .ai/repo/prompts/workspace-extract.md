---
name: workspace-extract
description: read one batch of a workspace's conversations completely and write one structured extraction per conversation into the run's working set
---

# Read one batch of a workspace's conversations completely and write one structured extraction per conversation into the run's working set

This is the brief of the extraction step of majordomus://skill/promote-a-workspace. You read
conversations of one declared workspace (`.ai/repo/workspaces/<workspace>.yaml`) so that
what the owner ordered, decided and learned there can be promoted into the repository at
{{REPOSITORY}}. Every item you extract must lead back to its root: the conversation id
and the message number `[n]` it came from.

## Inputs

- `<batch file>`: one line per conversation, `<markdown path><TAB><characters>`. Each file
  is the addressed view in `md/` of the run's working set (the store
  `.ai/local/workspaces/<workspace>/` once a command owns it): a header with the
  conversation id, its URL, when it was created and how many branches the view dropped, then
  messages headed `## [n] ROLE <time>`.
- `<catalogue>`: every record the repository already has (intents, milestones, issues with
  their milestone, decisions, rules, curated knowledge), built from {{HEAD}} before the run.
- The repository itself, read-only: search `.ai/repo/` and `docs/` to decide whether
  something is already recorded or already implemented.

## Rules

- Read-only everywhere except `<output directory>`. Edit, stage, commit, build, test and
  network access are all out of bounds, and so is opening or closing a task.
- Classify relevance before anything else: `core` (about this repository), `related`
  (about its tools or its practice) or `offtopic`. An off-topic conversation is not read
  further and nothing of it is copied: its file is the short shape under Output, with
  `read_fully: null`. Never its title, never a summary.
- Read every relevant conversation completely, in parts when it is long. A generated prompt
  pack, a pasted log or a long answer is read too: the decision is often inside it. Report
  `read_fully: false` with the message ranges you did not read rather than skimming silently;
  a file without the field is not accepted.
- A proposal is `accepted` only when the owner ordered it, accepted it, acted on it, or a
  later conversation treats it as settled; cite that message. Otherwise it is `proposed`. A
  proposal the owner refused is `rejected` and is kept, because a rejected alternative
  belongs in a decision record.
- When the owner revises a prompt over several messages, the last revision is the
  statement. An assistant message repeated verbatim counts once.
- What may be a knowledge item is `docs/KNOWLEDGE.md`'s; by its line on an advisor's
  numbers, a score, percentage, forecast or dated count from the assistant goes to `notes`.
- An attachment named in a conversation (a prompt pack, an archive) is not in the export;
  look for it under `campaigns/` (`campaigns/README.md`) before calling it absent.
- Credentials, tokens, the owner's own personal data (contact details, accounts, location,
  health, finances, family, employer and clients) and any third party's personal data are
  never copied: write `[redacted]`. The repository is public.
- Write in English. `quote` keeps the owner's own wording, any language, at most 200
  characters; it stays in the working set and never reaches a record, which states the
  assertion in its own words.
- Fewer, sharper items. A work item is an outcome a worker could start, with acceptance.
  When something is unclear, say so in `notes` instead of inventing it.
- For each item look it up in `<catalogue>`, and search the repository when unsure, and set
  `existing`: `recorded` (the repository says the same), `partial` (it says part of it) or
  `new`, with the identities it maps to (`I1234`, `adr-0098`, `milestone:<id>`,
  `rule:<id>`, `knowledge:<id>`, `intent:<id>`).

## Output

Write exactly one file per conversation, `<output directory>/<conversation id>.json`, as
soon as that conversation is done, so a stopped run keeps its work. Skip a conversation
whose file exists with `read_fully` true or null; read again one whose file says `false` or
has no `read_fully`, and overwrite it.

An off-topic conversation's file is only:

```json
{"conversation": "<id>", "created": "<YYYY-MM-DDTHH:MMZ>", "relevance": "offtopic", "read_fully": null}
```

A relevant one:

```json
{
  "conversation": "<id>",
  "title": "<title>",
  "created": "<YYYY-MM-DDTHH:MMZ>",
  "relevance": "core|related",
  "read_fully": true,
  "skipped": [{"from": 12, "to": 14, "why": "..."}],
  "summary": "3-6 sentences: what was asked and what was concluded",
  "themes": ["kebab-case"],
  "owner_directives": [
    {"msg": 0, "directive": "what the owner ordered or wanted", "quote": "...", "standing": true}
  ],
  "decisions": [
    {"msg": 3, "statement": "the decision as an assertion", "rationale": "...",
     "alternatives_rejected": ["..."], "status": "accepted|proposed|rejected",
     "accepted_by": "msg number of the owner's act, or null", "adr_worthy": true,
     "existing": {"status": "new", "refs": []}}
  ],
  "intents": [
    {"msg": 1, "statement": "what must become true, in the owner's view",
     "criteria": ["observable and testable"], "non_goals": ["..."],
     "existing": {"status": "partial", "refs": ["intent:<id>"]}}
  ],
  "work_items": [
    {"msg": 5, "title": "the outcome as a present-tense assertion", "objective": "...",
     "acceptance": ["..."], "area": "an id under .ai/repo/why/areas/ (majordomus why areas)",
     "priority": "P0|P1|P2|P3", "depends_on_text": ["..."],
     "milestone_hint": "an existing milestone id, or a new theme",
     "existing": {"status": "new", "refs": []}}
  ],
  "knowledge": [
    {"msg": 7, "class": "fact|convention|constraint|lesson", "title": "one-line assertion",
     "body": "2-5 sentences", "existing": {"status": "new", "refs": []}}
  ],
  "open_questions": [{"msg": 9, "question": "..."}],
  "notes": "duplicates, a later conversation that supersedes this one, opinions seen, and so on"
}
```

When the batch is done, answer with: the conversations processed, the files written, every
conversation with `read_fully: false`, and the five to ten most important new items, each
with `<conversation>#<n>`.
