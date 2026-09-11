---
schema: context/v1
id: ai.repo.sessions
kind: context
title: Session records
description: One immutable record per closed execution episode, derived from git and the ledger and shared with every surface.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [lib/session.sh, share/schemas/majordomus/session-record/session-record.v1.schema.json]
---

# Session records

A record here is one closed execution episode: when it opened and closed, the branch it ran
on, the commit it started from and the one it ended at, the commits between them, the paths
the working tree held changed, what it produced, and how it ended. `majordomus session
close` writes it; nothing else does, and nothing edits one afterwards.

Everything in the front matter is **derived**. The times come from the clock, the heads and
the commits from git, and every list — tasks, issues, milestones, checkpoints, handovers,
decisions, questions, evidence — from the ledger's own events for that episode.

The body is **composed, and never empty**. An authored summary on standard input at close is
still taken where one is given, but it was for a long time the body's only producer, and the
lifecycle closes an episode with `< /dev/null` — so on the automatic path, which is every
path, the body was empty by construction. A record naming ninety-four commits and saying
nothing about any of them is a receipt, not a record. Where no summary is authored, the body
is composed from this episode's own checkpoints, decisions and questions and from what git
and the ledger prove, and where the episode recorded nothing of its own the section **says
so by name** and names the command that would have recorded something. Nothing here invents
a summary: an acknowledged gap is a fact a reader can act on, and a fabricated narrative in
an append-only record is one nobody can tell from a real one afterwards. The sections are
declared in `policy.yaml` under `session.record_sections`, and one named there with no
writer is a configuration error reported by name.

## What a record may not contain

No conversation. `project.never-store-transcripts` holds here as everywhere: a record states
what is true about the repository, not what was said to arrive at it. The schema has no
field for a transcript, a prompt or a turn, and an unknown key is refused rather than
carried. The prompts a person wrote have their own store, and it is checkout-local by
design.

No fact about a machine. A shared record names the repository by its remote and the working
copy by `worktree_id`, a hash that identifies it without disclosing where it lives. The
absolute path stays in the ledger, which is local. ADR 0014 records that decision.

## The contract

The front matter satisfies `share/schemas/majordomus/session-record/session-record.v1.schema.json`; the allow-list under
`share/allow/session-record.txt` is generated from it, and an unknown key is an error.
`schema: session/v1` identifies the format, and a version this executable does not read is
refused rather than guessed at.

The file name is a convenience — the timestamp, the id, the branch, the head and a digest —
and the identity is `session_id` in the front matter, which survives a rename.

## What is derived from this directory

The source class `session` in `../knowledge/sources.yaml` discovers `*.md` here; that one
declaration is the whole registration. From it follow the index, the MCP resource
`majordomus://session/<id>`, the object routes of the API and their OpenAPI description, the
knowledge graph's `session` nodes, the site's registry pages and the cockpit. None of them
holds a list of sessions, and none needs editing when an episode closes.

```bash
majordomus session list              # closed episodes, newest first
majordomus session show <id>         # one record, whole
majordomus session latest --path     # the newest that resolves in this worktree
```

## The other half of an episode

A record here says what the episode **produced**. What it was **given** is a different
object, and a local one: `session start` freezes the context the builder resolved into
`.ai/local/session-contexts/<stamp>--<session-id>.md`, and `session close` appends the
outcome and the path of the record it wrote here. The two share a `session_id` and answer
opposite questions.

That file is the **opening snapshot** and nothing more. It used to carry a `## Notes`
heading for the worker to hand-edit, and across the 23 episodes this repository's own
checkout had opened by 2026-09-11, 23 carried the template and 0 carried a note: a heading
with no producer is a database nobody writes to, and a lifecycle that depends on a model
remembering to edit Markdown is not a lifecycle. The heading is gone. What replaced it is
not a fourth noun for a "note" — `decision`, `question` and `checkpoint` already exist,
already emit ledger events, and `mj_ledger_append` already stamps each line with the open
episode's id (`project.no-new-nouns`). What they lacked was a reader.

`majordomus session context` is that reader. It no longer prints a path; it **composes the
episode's working context on every read** — its identity, git now against git at the open,
and every checkpoint, decision and question the episode recorded, taken from the ledger
lines carrying its session id. Nothing caches it, so nothing can serve a stale one, and
nothing has to remember to refresh it. `--path` prints the opening snapshot instead, which
is what this command printed before.

Printing the composed context on an explicit request is within the layer's contract and not
a breach of it: `.ai/README.md` forbids loading `local/` *implicitly* and names the context
builder, "when a worker asks it", as one of the two routes by which local state legitimately
reaches a model. What is composed is bounded to repository facts and the worker's own
records — never the prompt archive, never a transcript.

That half stays local for two reasons that are not the same one. It names one machine — the
worktree, the checkpoint that happened to be newest there — and it is a snapshot of a
projection, so re-resolving it later produces a different document and no surface can
reproduce it. Publishing it would publish something nothing can check (ADR 0017). Its
contract is `majordomus.session-context/v1`.

Neither half is a transcript. This one carries what git and the ledger can prove; that one
carries the builder's output and a summary of the work. A key naming a conversation is
refused in both, which is how `project.never-store-transcripts` is kept mechanically.

## Retention

None. A record is history, it is small, and it is tracked: git keeps it, and every surface
can query it. The checkout-local half — the open session, the working contexts, the ledger —
keeps its own caps in the policy where it has them, because those grow without bound and
nothing reads them after the fact. The working contexts have no cap for the same reason the
prompt archive has none: nothing else can reconstruct one.
