+++
title = "Evolve the repository's know-how"
description = "Turn what sessions observed into reviewed knowledge and into the rule, skill, document or test each lesson belongs in, then deliver the change and tell every peer what moved."
weight = 3
[extra]
id = "evolve"
status = "active"
version = 1
source = ".ai/repo/skills/evolve/SKILL.md"
+++
{% raw %}

## Purpose

A repository learns in one direction: something is met, it is recorded, somebody decides
whether it is true, and what is true changes how the next worker works. Every organ of
that already exists here — observations, candidates derived at the end of an episode,
promotion and rejection, rules, skills, cases, the derived surfaces. This skill is the
order to use them in, so that a lesson does not stop as a note.

It is not a second store, a scoring system or an autonomous rewrite. Nothing is promoted
without evidence and a decision, nothing is written anywhere but where the kind already
lives, and nothing reaches a surface except by being derived from the file that changed.

## When to use

When a session ends with something learned, when the briefing names candidates awaiting
review, after an incident, and whenever the operator asks for it by name. An episode's end
already derives its candidates (`session.knowledge_on_end`); this is what is done with them.

## Procedure

### 1. Gather what there is to decide

```bash
majordomus knowledge candidates
majordomus knowledge check
```

Then record what this session met and has not written down yet, one observation each, in
the ledger and nowhere else:

```bash
majordomus knowledge observe --kind defect --subject rule:project.every-wait-is-bounded \
  --evidence test:test/cases/122_liveness_gate.sh "the scan read no build, so a silent build wait was not a finding"
majordomus knowledge derive --dry-run
```

`derive` writes this episode's candidates; the dry run shows them first. Refused here: an
observation without a subject, and a note kept in a file of your own instead of the ledger.

Three things the command refuses, each met by the first worker who followed this step:
- An observation belongs to an episode. In a worktree where none is open, open one first
  with `majordomus session start`.
- `--kind` is one of `friction`, `workaround`, `defect`, `drift`, `repetition`. A lesson is
  what a candidate becomes when it is promoted, not a kind of observation.
- `--subject` is a repository-relative path, or `capability:`, `rule:`, `command:` or
  `gate:` followed by an identifier. A test is named by its path; `test:` is a prefix of
  `--evidence` and not of a subject.

### 2. Decide each candidate

A candidate is a claim somebody has to stand behind. Read its record and what it names,
then one of two things, never a third:

```bash
majordomus knowledge promote <id> --class lesson < evidence.md
majordomus knowledge reject <id> --reason "<why, and what replaces it>"
```

Promotion requires the evidence on standard input: the command, test, commit or measurement
that makes the statement true, not a restatement of it. A candidate that duplicates a
curated note or a rule is rejected with the identity of what already says it. A candidate
you cannot judge stays a candidate; say so in the report instead of guessing.

### 3. Put each lesson where it changes behaviour

A promoted note that nothing reads changes nothing. For each one, ask what would have
prevented the thing it describes, and make that change in the place the kind lives:

<div class="overflow-x-auto" tabindex="0">

| the lesson is about | it becomes | where |
|---|---|---|
| something that must never happen again | a rule with a validator or a test, or a new shape in an existing gate | `.ai/repo/rules/`, `scripts/ci/`, `test/cases/` |
| how to do a kind of work | a step in a skill or a workflow | `.ai/repo/skills/`, `.ai/repo/workflows/` |
| what a thing is or why it is so | a paragraph in the document that already explains it, or a decision | `docs/`, `majordomus decision` |
| a choice between alternatives | an architecture decision record | `.ai/repo/adrs/` |
| a fact only this repository needs to remember | the curated note itself | `.ai/repo/knowledge/curated/` |

</div>


Follow `majordomus://skill/implement` for the change. Refused here: a new directory,
registry or database for lessons; a rule with no way to be decided; a number, a list or a
link typed into a page that a generator owns.

### 4. Prove it

```bash
majordomus knowledge check
majordomus skills check
majordomus check
scripts/derive
```

Run the gate of everything the change touched before the derive, so that one derive is
enough. A lesson about a gate is proven by a case that breaks the thing and watches the
gate refuse it.

### 5. Deliver, and let the surfaces follow

Commit, hand the change to the landing queue as one pull request, and read the pages run
after it merges. The website, the product UI's published copy, the generated skill pages,
the MCP resources and every provider's instruction file are projections of the files that
changed; none of them is edited, and one that did not move is a finding about its
generator. Where a chapter of the handbook says what changed, change it in the same pull
request.

### 6. Tell everyone who works here

A lesson one session learned is worth what the others know of it. Announce on the board
what changed and where, and publish a handover that names the rule, skill or document and
the commit, so a session that starts tomorrow reads it in its briefing:

```bash
majordomus handover
```

and, over the shared server, the `peers.announce` and `mesh.handover.publish` capabilities
(`majordomus_announce`, `majordomus_mesh_handover_publish`). Report under
`majordomus://skill/report-verification-state`: what was promoted, what was rejected and
why, what changed, what was proven and what was not.

## Output

- Every candidate that was reviewed is promoted with evidence or rejected with a reason;
  the ones left undecided are named with why.
- Each promoted lesson that asked for a change has one, in the place its kind lives, with
  the test or validator that decides it.
- `majordomus knowledge check`, `majordomus skills check` and `majordomus check` pass, and
  the derived files are current.
- The change is delivered or its pull request is named, the pages run after it was read,
  and the board and a handover say what moved.
{% endraw %}
