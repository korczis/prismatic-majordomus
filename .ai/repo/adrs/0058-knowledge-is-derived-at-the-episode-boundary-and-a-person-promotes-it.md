---
schema: adr/v1
id: adr-0058
kind: adr
title: Knowledge is derived at the episode boundary, from the ledger and git, and a person promotes it
status: proposed
date: 2026-09-12
tags:
  - knowledge
  - sessions
  - continuity
  - governance
provenance:
  origin: authored
  derived_from:
    - decision:adr-0010
    - decision:adr-0014
    - decision:adr-0017
    - decision:adr-0052
related:
  - rule:majordomus.knowledge-observed
  - rule:majordomus.knowledge-integrity
  - rule:majordomus.candidates-reviewed
  - rule:majordomus.lifecycle-observed
  - rule:project.never-store-transcripts
  - rule:project.no-new-nouns
  - rule:project.no-network-no-eval
  - rule:project.derived-files-regenerated
  - rule:project.accumulation-is-measured
  - claim:knowledge-derived-at-the-boundary
  - claim:knowledge-derivation-is-idempotent
  - claim:knowledge-failure-is-recorded
  - claim:knowledge-integrity
  - claim:knowledge-writer-observed
  - claim:knowledge-promotion-is-an-act
  - claim:briefing-names-candidates
  - claim:candidates-reviewed-not-accumulated
  - claim:knowledge-served-on-every-surface
  - file:share/standard/majordomus/rules/knowledge-observed.v1.md
  - file:share/standard/majordomus/rules/knowledge-integrity.v1.md
  - file:share/standard/majordomus/rules/candidates-reviewed.v1.md
  - file:share/events.yaml
  - file:.ai/repo/policy.yaml
  - file:.ai/repo/knowledge/sources.yaml
  - file:.ai/repo/knowledge/candidates/README.md
  - file:share/skeleton/ai/repo/knowledge/candidates/README.md
  - file:share/schemas/majordomus/knowledge/knowledge.v1.proto
  - file:lib/knowledge.sh
  - file:lib/capture.sh
  - file:lib/session.sh
  - file:lib/init.sh
  - file:apps/majordomus-cli/src/capability/builtin/knowledge_base.rs
  - file:docs/KNOWLEDGE.md
  - test:test/cases/278_knowledge_is_derived_at_the_boundary.sh
  - test:test/cases/279_knowledge_derivation_is_idempotent.sh
  - test:test/cases/280_knowledge_switch_and_failure_leave_evidence.sh
  - test:test/cases/281_knowledge_check_refuses_malformed_records.sh
  - test:test/cases/282_health_sees_a_stopped_knowledge_writer.sh
  - test:test/cases/283_knowledge_promotion_is_an_act.sh
  - test:test/cases/284_briefing_names_the_candidates.sh
  - test:test/cases/285_candidates_are_reviewed_not_accumulated.sh
  - test:apps/majordomus-cli/tests/knowledge_base.rs
---

# 58. Knowledge is derived at the episode boundary, from the ledger and git, and a person promotes it

## Context

An episode records decisions, resolves questions and finishes tasks, and every one of those
leaves a typed line in the ledger. When the episode ends, the checkpoint records what the
work looked like and the handover records what to do next (ADR 0052). Nothing records what
the repository now knows. A decision recorded under a task is visible in `majordomus
history`, on one machine, until the ledger's retention cap removes it; a resolved question is
an answer in a local store; a task that finished `blocked` leaves its reason in a note under
`.ai/local/state/completed/`. Each is evidence of something true about the repository, and
each is unreachable from the tracked tree, from every other checkout, and from every surface
the registry feeds.

The layer already has the object this is for. ADR 0010 fixed that curated knowledge is one
kind, `knowledge/v1`, with a class (`fact | convention | constraint | memory | lesson`), a
status (`candidate | verified | superseded`), an epistemic stance (`observed | inferred |
decided`) and provenance whose `derived_from` names ledger objects with typed references.
What is missing is a writer: `share/schemas/majordomus/knowledge/knowledge.v1.proto` says
`extracted` means the tool derived it, and no command does. The store holds one authored
note.

A previous attempt, `origin/feature/repository-knowledge`, took the opposite direction: a
scan of the checkout's manifests and prose into typed knowledge, a baseline of tolerated
knowledge debt, a `mode: observe|warn|protect|strict` ladder, and a `semantic` policy switch
that lets a provider read the text. Its substantive modules were untracked when the
checkpoint was forced and are lost; what survives is the wiring of a design, not the design.
This record says what is taken from it and what is not.

ADR 0014 supplies the argument about where records go: a record only one machine can read is
a record nobody reads. ADR 0017 supplies the moment the next worker is told: the start
briefing. ADR 0052 supplies the invariant this record extends: a store that can be read is
not a store that is being written, and a stopped writer must be reported by a check that
compares two facts, episodes closing and records advancing.

## Decision

**The knowledge base is the `knowledge` kind under `.ai/repo/knowledge/`, and nothing else.**
No new kind, no new store, no memory service, no agent, no registry. A record is one assertion
about the repository, in one line, with a class, a status, an epistemic stance and provenance
that resolves. A record is never a summary of a conversation and never quotes a prompt or a
turn; the transcript check that `project.never-store-transcripts` is kept by applies to its
title, its description and its body.

**A derived record is a candidate under `.ai/repo/knowledge/candidates/`, tracked, discovered
by a source class of the existing kind.** The deriver writes `status: candidate`,
`provenance.origin: extracted` and `derived_from` naming the ledger objects the record came
from. The directory is a sibling of `curated/`, declared as the class `candidates` in
`.ai/repo/knowledge/sources.yaml` with the pathspec `:(glob).ai/repo/knowledge/candidates/*.md`,
and carries its own `README.md` contract (ADR 0011). The contract is never missing: `init`
seeds the directory and its README from the skeleton, and a deriver that creates the
directory in a repository initialised before this decision seeds the README beside the first
candidate. It is tracked for ADR 0014's reason: a candidate under `.ai/local/state/` would be
readable by the machine that derived it and by no other, and the review it waits for would
happen nowhere. The file name is the record id, and the id is derived from the evidence: the
episode id followed by a digest of the source object, so that two worktrees never write one
name for different content and a re-run over the same evidence rewrites the same file byte
for byte.

**`verified` is written by an act, never by the deriver.** `majordomus knowledge promote <id>`
with evidence on standard input moves the record to `curated/`, sets `status: verified`,
keeps its `derived_from` and adds the promoting episode to it, and appends the evidence under
`# Evidence`; an empty stdin is refused, because an act without evidence is an assertion, and
stdin that carries a transcript marker is refused for the same reason a record may not. The
class is the person's to set: `--class <c>` at promotion is where a `convention` becomes a
`constraint` or a `lesson`, because whether a decision binds is a judgement the deriver does
not make. `majordomus knowledge reject <id> --reason "<why>"` rewrites the candidate in place
as `superseded`, naming `superseded_by` when `--by <id>` gives the record that replaces it and
recording the reason under `# Rejected` otherwise. A person editing the file is the other act.
The deriver skips an id that has been promoted or superseded, so neither act is undone by the
next episode boundary.

**What is derivable without a model is exactly this list, and the deriver implements exactly
this list.** The evidence is the ledger lines the episode stamped and the commits it made;
nothing reads a conversation, a handover or a prompt.

| evidence | record | class | epistemics | derived_from |
|---|---|---|---|---|
| `decision.recorded` | the decision text | `convention` | `decided` | `session:<episode>`, `decision:<task-id>`; the session alone when the line carries no task |
| `question.resolved` | the answer, stating the question it answers | `fact` | `observed` | `session:<episode>`, `task:<task-id>` |
| `task.finished` with outcome `blocked`, `failed` or `no_match` | the outcome and the first line of the task note's `# Reason` | `lesson` | `inferred` | `session:<episode>`, `task:<task-id>` |
| `task.finished` with outcome `completed` and a `verify` command | the verification that passed | `fact` | `observed` | `session:<episode>`, `task:<task-id>` |
| commits of the episode touching `.ai/repo/rules/` or `.ai/repo/adrs/` | no record; each record of the episode gains `commit:<sha>` in `derived_from` and `relates_to file:<path>` in `relations` | — | — | — |

Every derived decision is a `convention`. An earlier draft classified a decision as a
`constraint` when its wording carried a word such as `never` or `must`; that is a reading of
prose, which `docs/CLI.md` refuses for every kind, and it is exactly the judgement promotion
exists to record. A handover's Next Action is continuation state, not knowledge. An ADR, a
rule and a document are sources of their own; a record relates to them and never restates
them. Same ledger, same git, same bytes: the record's `date` is the day of the evidence line,
not the clock, and no random value enters the file. The free text of a ledger line is
unescaped once when it is read and escaped once when the record is composed, so the digest is
over the text a person wrote and a quotation mark inside a decision neither cuts the record
short nor changes its identity. A line whose text carries a transcript marker yields no
record; the deriver reports it as skipped rather than writing a record the integrity check
would refuse. No network, no model, no `eval`.

**The reference vocabulary gains `task:<id>`; `question:<id>` is rejected.** The proto's
`derived_from` pattern admits `decision`, `session`, `commit`, `issue`, `file` and `test`. A
decision has no identity of its own in the ledger — `decision.recorded` carries the task and
the text — so `decision:<task-id>` is the reference, as the proto's own comment and ADR 0010's
provenance already use it. A question has no identity either, and its task is what resolves
it, so the pattern gains `task` and nothing else. A decision recorded outside a task derives
from the session alone; `decision:none` and `task:none` are refused as references, because a
reference to nothing is not a reference. The proto changes; the schema and the allow-list are
regenerated, never edited.

**It runs when the episode closes and when the conversation is compacted, for the
checkpoint's reason.** Those are the moments at which what the episode knows stops being
reachable. The close path is one: `mj_session_close` in `lib/session.sh` runs the deriver
after the session record is published and before `session.closed` is appended, whenever
`session.knowledge_on_end` is not off, so every close — the provider's end adapter,
`majordomus session close`, any future caller — is followed by its `knowledge.derived` line
and never by a close without one. The end adapter reads the close's outcome and reports a
derivation failure through `mj_capture_session_failed`; it does not derive a second time. The
compaction adapter has no close to ride on, so it calls the deriver itself, switched by
`session.knowledge_on_compact` and independent of `checkpoint_on_compact`: a compaction that
skips its checkpoint still derives. It runs whether or not a task is active: the lifecycle is
the episode's. On demand, `majordomus knowledge derive [--episode <id>]` does the same for the
open episode or the one named. A second run over the same evidence writes nothing and says
so, on the command line and in the ledger.

**Evidence that it ran is a ledger line, registered before it is written.** `knowledge.derived`
carries the episode, the count written, the count unchanged, the paths written and, when a
line was refused, the count skipped; `knowledge.promoted` and `knowledge.rejected` carry the
id and what was done. A derivation that fails inside a provider hook is recorded as
`provider.event.failed` through `mj_capture_session_failed`, and the hook exits 0: the
provider is never blocked and a failure is never silent.

**The next worker is told.** The start briefing (ADR 0017) gains one bounded section, after
the open questions and before the handover: candidates awaiting review on this branch, the
count on one line and the ids beneath it, bounded the way the open-questions block is, inside
`session.briefing_budget_lines`. A candidate's branch is the branch of the episode it names in
`derived_from`, read from the tracked session record or, while that record is not yet
written, from the ledger. A candidate whose episode this checkout cannot resolve is named on
its own line as unattributed rather than hidden, because a record the briefing cannot place
is still a record somebody has to review. Absence is printed rather than omitted.

**Enforcement is three rules of the standard package, dispatched from the doctrine registry,
and not project rules.** The task asked for project rules. `lib/doctor.sh` refuses, in every
repository, a validator function that no rule of that repository's effective set declares,
and `majordomus init` seeds a repository with the vendored standard package only. A validator
declared by a rule under `.ai/repo/rules/project/` would therefore turn `doctor` red in every
disposable test repository and in every adopter, which is what ADR 0052 met and answered by
shipping `majordomus.lifecycle-observed` in `share/standard/majordomus/rules/`. The same
answer applies for the same reason: the deriver ships with the tool and runs in every
adopter's hooks, so the rule that it keeps writing is the vendor's, not one repository's.
The shipped manifest is written by `scripts/rules-package write`, the vendored copy under
`.ai/repo/rules/vendor/` is refreshed by `majordomus rules vendor update`, and neither is
edited by hand.

- `majordomus.knowledge-observed`, blocking, `mj_validate_knowledge_lifecycle`, dispatched from
  `doctor` and `watch`, category `knowledge`, exit 10. The finding: the newest `session.closed`
  line names an episode no `knowledge.derived` line names, and it is older than
  `session.freshness.stale_minutes`; or `session.knowledge_on_end` is on and the lifecycle
  source never calls the deriver, read from the source the way `mj_validate_doctrine_wiring`
  reads it. The judgement is gated the way ADR 0052 gated its own: a checkout whose ledger
  holds no `knowledge.derived` line at all is not judged and says so as a skip, because the
  day this decision lands every checkout has closed episodes and none has derived, and a
  finding that is red everywhere at once is a finding nobody reads. What the gate leaves
  unjudged is exactly one case, a checkout on which the deriver has never run, and the first
  derivation ends it. The finding's reproduce is read-only, `majordomus history` filtered on
  the two events; the remedy, `majordomus knowledge derive --episode <id>`, follows `fix:` in
  the message. The Rust reader, `knowledge_base.status`, makes the freshness half of the same
  judgement from the same ledger and the same policy numbers; the wiring half is the shell
  validator's alone, because only the shell tool can read its own source.
- `majordomus.knowledge-integrity`, blocking, `mj_validate_knowledge_integrity`, dispatched
  from `check` and `doctor`. Every record under `candidates/` and `curated/` is valid against
  `knowledge/v1` with no unknown key, ids are unique across both directories and equal the
  file name, `verified` carries provenance, every `derived_from` and `relations` target
  resolves, `superseded` names `superseded_by` or records its rejection, no record under
  `candidates/` claims `verified`, and no title, description or body carries a conversation.
  The validator reads both directories once and resolves every reference in one pass over
  the ledger and one batched query of git, so its cost grows with the store, not with the
  product of records and references. `majordomus knowledge check` is the same check as a
  command.
- `majordomus.candidates-reviewed`, advisory, `mj_validate_knowledge_accumulation`, dispatched
  from `doctor`. The policy declares `knowledge.candidates_max_files` and
  `knowledge.candidate_max_age_minutes` once; more candidates than the cap, a candidate whose
  review age exceeds the age, or a candidate the tree does not track, is a `WARN` naming
  them. Review age is the age of the queue entry, not of the evidence: it is measured from
  the oldest `knowledge.derived` line whose paths name the file, then from the commit that
  added the file, and only then from the record's `date`, and the finding says which source
  it used, because `date` is the evidence day and a candidate derived today from last
  month's decision has been waiting one day, not one month. An untracked candidate is named
  because a record git does not hold is discovered by nothing and reviewed by nobody. It is
  advisory because whether a candidate deserves review is a person's judgement; it is
  dispatched from `doctor` only, because under `watch` an advisory finding is drift and exits
  11, and a full review queue must not make `watch` fail. A policy that declares neither key
  is reported at the rule's own class, as `mj_validate_retention` reports its keys.

**The Rust reader is one module, declared once, and reads what the shell wrote.** The module
`knowledge_base` under `apps/majordomus-cli/src/capability/builtin/` declares
`knowledge_base.candidates`, `knowledge_base.record` and `knowledge_base.status`, and
`majordomus generate` projects them to HTTP, OpenAPI, MCP, the command line, the cockpit's
module catalogue and the benchmark inventory. The module id is not `knowledge` because the
registry refuses a builtin module named like a declarative kind. The reader takes candidates
from the index (tracked files, as every kind), the ledger from `crate::session::Ledger`, and
the thresholds from `continuity.rs`'s reading of the policy; it restates none of them, and it
does not claim to read `lib/capture.sh`. The cockpit gets no hand-written page: the module's
catalogue entry and each capability's generated page are the projection (ADR 0012).

## Alternatives rejected

**A scan of the checkout into typed knowledge, with extractors, canonicality and a baseline of
tolerated debt.** This is what `origin/feature/repository-knowledge` built. It infers
knowledge from manifests and prose, which `docs/CLI.md` refuses for every kind — a kind comes
from structure, never from prose — and it declared its subcommands in `cli.rs` rather than in
`capability!` blocks, which `project.rust-canonical-declaration` forbids. Its baseline and
exceptions files are a second store keyed by id, the inventory ADR 0053 rejected because it
goes stale the moment a branch is abandoned, and its `mode` ladder is a third enforcement
vocabulary beside `blocking | advisory`. Its `semantic.enabled` switch exists to let a provider
read the text, which `docs/CONTINUITY.md` and `project.no-network-no-eval` refuse. Its
per-record `visibility` field puts the publication boundary on each producer, where the
source class already draws it once. What is taken from it is the shape of a status answer,
now `knowledge_base.status`, and the principle that a knowledge policy key is declared once
in the policy, the skeleton, the schema and the regenerated allow-list. The name `knowledge
derive` is reused here with the opposite meaning: deterministic, no model, no network.

**Candidates under `.ai/local/state/`, promoted by hand into the tracked tree.** The deriver
would write where the checkpoint writes, and nothing would change on any other machine until
somebody remembered a command. That is the shape ADR 0014 named: a record only one machine can
read. The review a candidate waits for would happen on the one checkout that cannot be shared,
and the stopped-writer finding would be meaningless on every clone.

**A new kind, `knowledge-candidate`, with its own schema.** A candidate and a verified record
differ in one field, `status`, which the existing schema already carries. A second kind would
need a second schema, a second allow-list, a second class and a second set of readers, for a
distinction one enumerator makes. ADR 0010 and `project.no-new-nouns` both refuse it.

**Deriving from the handover's Next Action or the checkpoint's body.** Both are derived
projections of the same ledger, and the Next Action is what to do, not what is true.
Deriving knowledge from a derived record would put a projection of a projection into the
tracked tree, with provenance pointing at a file under `.ai/local/`.

**Classifying a decision by its wording.** A word list that turns `never` into `constraint`
reads prose to decide a kind, which the knowledge compiler refuses on principle, and it is
wrong often enough to be worse than no answer: a decision that says "we no longer need the
old form" carries `no` and binds nobody. The deriver writes `convention`; the person who
promotes the record writes the class, with the evidence beside it.

**Deriving from the end adapter rather than from the close.** The first draft put the call
in `mj_capture_session_end`, after the checkpoint and before the handover. Then `majordomus
session close` typed by a person, and every future caller of the close, would close an
episode with no derivation, and the stopped-writer finding would be red for a close the
adapter never saw. The close is the one path every close takes, so that is where the
deriver runs; the adapter keeps the failure report, which is its job.

**Unique file names through `mj_publish_record`.** The atomic writer every local record uses
appends a random suffix and never overwrites, so a re-run would write a second file with the
same content. Idempotence needs an id-named file, written through a temporary name in the
same directory and renamed over the old one. The `.tmp.` prefix keeps it under the existing
ignore rule and the existing sweep.

**`question:<id>` in `derived_from`.** A question carries no identity in the ledger; inventing
a digest to reference it would be a reference nothing else in the layer could resolve. The
task that owns the question resolves it.

**Measuring a candidate's age from its `date`.** The date is the evidence day, kept for
determinism. A validator that read it as the queue age would name a candidate derived this
morning as fourteen days overdue, because the decision it records was made two weeks ago.
The queue entry has its own age, in the ledger and in git, and that is what is measured.

**Project rules declaring the validators.** Rejected above for a mechanical reason: `doctor`
in every adopter and every test repository would report a validator no rule declares, and
seeding project rules from the skeleton would put another repository's rules in the
project namespace.

**A dedicated cockpit page.** A page is a hand-written route (`cockpit/mod.rs`), a nav area,
and an entry in the page sweep; the declaration already yields a catalogue entry and a page
per capability. A page can be added when a person needs one; nothing in this decision
depends on it.

## Consequences

Every episode boundary that records a decision, resolves a question or finishes a task leaves
a candidate in the tracked tree, and the ledger line `knowledge.derived` says so even when it
wrote nothing. The tree is dirty after such an episode until the person adds the candidates
beside the session record, which the existing `session` finding already asks for and the
advisory rule now names; a candidate is committed content, and `changed_files` of the next
checkpoint lists it, as it lists a session record.

A stopped knowledge writer is reported by `doctor` and `watch` in this repository and in every
adopter, with the same thresholds the handover uses; a repository in which no episode has
closed, or in which no derivation has ever run, is not judged and says so. The finding
compares episodes, not timestamps, because the close derives before it appends
`session.closed`, so the `knowledge.derived` line of an episode precedes its `session.closed`
line in ledger order.

`majordomus knowledge` is no longer read-only. `share/commands.yaml` says so, `docs/CLI.md`
says so, and its exit codes cover every constant `lib/knowledge.sh` uses. The graph compiler
reads a knowledge record's front matter, so its node id is `knowledge:<id>` on both sides and
`knowledge edges --type derived_from` lists what a record came from; a target that only the
ledger or git can resolve is external to the graph and is resolved by the integrity validator
instead, never reported as dangling by the compiler.

The cost is measured where it is paid: `MJ_TIMING=1 majordomus knowledge derive --dry-run`
prints the deriver's own wall time, `MJ_TIMING=1 majordomus knowledge check` prints the
validator's counters, and `majordomus doctor` reports its budget as it does for every
doctrine. The policy gains four keys, declared once and read by the shell and the Rust reader
from the same file.

What this decides does not decide is which candidates deserve promotion. That remains the
person's act, and the advisory rule exists so that the queue is seen, not so that it is
emptied by a machine.
