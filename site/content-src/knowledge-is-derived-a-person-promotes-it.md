+++
title = "Knowledge is derived, a person promotes it"
description = "What a session learned usually stays in the conversation that learned it. How Majordomus turns the decisions, answers and outcomes an episode recorded into candidate knowledge at the moment the episode ends, without a model and without reading the conversation, and why only a person makes a candidate verified."
template = "page.html"
[extra]
lang = "en"
related = ["a-session-ends-the-work-does-not/", "a-rule-is-proven-by-what-it-refuses/", "written-once-derived-everywhere/"]
[extra.translations]
cs = "znalost-odvodi-nastroj-povysi-clovek/"
+++

An episode of work records decisions, resolves questions and finishes tasks, and each of those is a typed line in the ledger of one machine. The ledger is local and capped, so a decision stays visible in `majordomus history` only until the cap removes it. Each line is evidence of something true about the repository, and until the knowledge subsystem none of it reached the tracked tree, another checkout or a surface the registry feeds. [ADR 0091](@/adrs/adr-0091.md) (proposed) decides how it does: knowledge is derived at the episode boundary, from the ledger and git, and a person promotes it.

## One kind, not a memory service

The knowledge base is the `knowledge` kind under `.ai/repo/knowledge/`, and nothing else: no new kind, no second store, no memory service and no agent ([ADR 0010](@/adrs/adr-0010.md), proposed, and [project.no-new-nouns](@/rules/project-no-new-nouns-1.md)). A record is one assertion about the repository, in one line. Its class says what is asserted (fact, convention, constraint, memory or lesson), its status who stands behind it (the deriver, a person, or nobody any more), its epistemics how it is known (observed, inferred or decided), and its provenance the evidence, as typed references that resolve: a session, a decision, a task, a commit, an issue, a file or a test.

A record is never a summary of a conversation and never quotes a prompt or a turn. The check that keeps [project.never-store-transcripts](@/rules/project-never-store-transcripts-1.md) applies to its title, its description and its body. A decision record, a rule or a manual is a source of its own, and a record relates to it and never restates it.

## What is derived, and from what

The deriver implements one table and nothing beyond it, and [`docs/KNOWLEDGE.md`](@/docs/knowledge.md) prints that table. A recorded decision becomes a `convention` the episode decided. A resolved question becomes a `fact`, stated with the question it answers. A task that finished blocked, failed or without a match becomes a `lesson`, inferred from the first line of its note's reason. A task that completed with a verify command becomes a `fact`: the verification that passed. A commit of the episode that touched a rule or a decision record adds that commit and that file to every record the episode wrote. A completed task without a verification yields nothing, and so does a handover's next action, which is continuation state, not knowledge.

Every derived decision is a convention. Deciding from its wording whether a decision binds would infer a kind from prose, and that judgement belongs to the person who promotes it. The record's id is the episode's id followed by a digest of its evidence, its date is the day of the evidence and never the clock, and no random value enters the file. So the same ledger and the same git give the same bytes, and two worktrees never write one name for different content. A ledger line that carries a transcript marker yields no record and is reported as skipped. Nothing calls the network, a model or `eval`.

Since [ADR 0118](@/adrs/adr-0118.md) (proposed) a worker can also leave what it met on the way: `majordomus knowledge observe` records a friction, a workaround, a defect, a drift or a repetition about a subject, and the boundary turns it into a `lesson`, observed and routed by its subject to the platform, enforcement, the generator, the documentation or the project. The words are never read, and the route decides nothing.

## The moment it stops being reachable

The deriver runs at two moments: the provider's end event and its compaction event, the moments at which what the episode knows stops being reachable. Every close, whether a provider's hook or a person typing `majordomus session close`, is followed by its `knowledge.derived` line, before the close itself in ledger order. A compaction has no close to ride on, so its adapter calls the deriver itself. Both run whether or not a task is active, because the lifecycle is the episode's, and both can be switched off in the policy.

A trunk that changes only through pull requests cannot commit what a boundary writes there. Since an amendment of 15 October 2026 (I1984), a trunk episode writes its candidates and its session record to a checkout-local outbox, and the next close on any other branch of the same clone carries each file, unchanged and under its own name, into the tracked sections, where that branch's pull request brings it to the trunk. `majordomus context` and `doctor` name a non-empty outbox, and neither treats it as a failure.

## A person makes it verified

`verified` is written by an act, never by the deriver. `majordomus knowledge promote <id>` moves a candidate to `curated/`, sets the class the person chose, keeps its provenance and adds the promoting episode, and appends the evidence the person gives on standard input. An empty input is refused, because an act without evidence is an assertion, and an input that carries a transcript marker is refused for the reason a record may not carry one. `majordomus knowledge reject <id> --reason` keeps the candidate as a superseded record, with the reason and, when given, the record that replaces it. The deriver skips an id that was promoted or rejected, so the next boundary undoes neither act.

The queue is measured and never emptied by a machine. The briefing that opens the next episode names the candidates awaiting review on its branch, and the advisory rule [majordomus.candidates-reviewed](@/rules/majordomus-candidates-reviewed-1.md) reports a queue over the policy's cap, a candidate older than the policy's age and a candidate that version control does not track. A candidate that old, the manual says, was not reviewed; it was ignored.

## Provenance outlives the files it cites

A file or a test that a record cites may later leave the tree. Since I1987 such a reference resolves through git history, to the newest commit that held or deleted the path, and `majordomus knowledge check` names it as historical rather than passing it in silence. A path that no commit of the history ever held still resolves to nothing. Case 3023 holds this by deleting a file a record cites and committing the deletion.

## When the writer stops

The blocking rule [majordomus.knowledge-observed](@/rules/majordomus-knowledge-observed-1.md) reports a stopped writer: the newest closed episode is named by no derivation and is older than the freshness the policy declares, or the switch is on and the lifecycle never calls the deriver. A checkout whose ledger holds no derivation at all is reported as a skip, so the day the deriver arrived no checkout turned red. The blocking rule [majordomus.knowledge-integrity](@/rules/majordomus-knowledge-integrity-1.md) refuses a record that is not schema-valid, uniquely identified, evidenced when verified, fully resolved and correctly superseded, a candidate that claims to be verified, and a record that carries a conversation.

## How to check it

`majordomus knowledge candidates` lists the queue and groups the records about one subject with the episodes behind them, and `majordomus knowledge derive --dry-run` shows what the open episode would write. `majordomus knowledge check` validates both directories, and `majordomus-cli knowledge status` reports the last derivation, the newest close and the stopped-writer judgement. `majordomus history --event knowledge.derived` shows every derivation as the ledger recorded it.

## What this does not claim

It does not claim that the knowledge base holds everything an episode learned. The deriver writes only what its table lists, and nothing that was said in a conversation and not recorded reaches it. Nor does a candidate assert anything until a person promotes it. ADRs 0010, 0091 and 0118 are proposed and the owner has accepted none of them; the deriver, the commands and the rules they describe are on master and run. How the work itself continues across sessions is in [A session ends, the work does not](@/a-session-ends-the-work-does-not.md), how a rule is held to a case that can fail in [A rule is proven by what it refuses](@/a-rule-is-proven-by-what-it-refuses.md), and why a derivation gives the same bytes every time in [Written once, derived everywhere](@/written-once-derived-everywhere.md).
