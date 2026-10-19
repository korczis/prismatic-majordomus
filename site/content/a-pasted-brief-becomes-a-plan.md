+++
title = "A pasted brief becomes a plan"
description = "A brief pasted into a session is a wish on its date, not an instruction. How this repository keeps the brief as it arrived, turns what it asks for into records a gate can refuse, and what that mechanism still has to prove."
template = "page.html"
[extra]
lang = "en"
related = ["proof-carrying-documentation/", "the-bet-on-a-layer/", "everything-followed/"]
[extra.translations]
cs = "vlozene-zadani-se-stava-planem/"
+++
{% raw %}

Most of the work in this repository starts the same way. I paste something into a session. Sometimes it is a prompt pack: a directory of numbered Markdown files, written in one sitting, saying what I want built and in what order. Sometimes it is a report: another model's review of the site, or of the plan, with a list of what it found wrong. Often it is in Czech, because that is the language I think in.

The obvious thing for an agent to do with a brief like that is to treat it as instructions and start building. That is also the failure. A brief is what I wanted on the day I wrote it, written by someone who had not looked at the tree that morning. Half of what it asks for may already exist. Some of it contradicts a decision the repository has recorded. And once a session has acted on it, the brief itself is usually gone — it lived in a downloads folder or in a chat, and the reason a subsystem exists goes with it.

So the question this article answers is narrow: what happens between the paste and the first commit, and which of those steps a gate can actually refuse. I come back to the briefs of 18 and 19 October 2026 at the end, because the work planned from them has not reached master yet, and that is part of the honest answer.

## First, the brief is kept as it arrived

A brief that drove work is provenance, so it is kept, byte for byte. The convention is in `campaigns/README.md`: every operator-authored pack is copied into `campaigns/` unmodified, and `campaigns/MANIFEST.sha256` records the bytes of every file in it. The same README says, in bold, that these are history and not instructions. Nothing loads them, nothing validates them as documents of the layer, and where a pack and the repository disagree, the repository is right.

The rule behind it is [a campaign brief this repository keeps is the one it was handed](@/rules/project-a-brief-is-kept-as-received-1.md), and `scripts/ci/campaigns-check` decides three things about it: the manifest covers exactly the files present, with their bytes, in both directions; every pack directory has a row in the README; and every row names a directory that exists. It decides nothing about what a pack *says*. A pack edited afterwards to agree with what was built would still read like the brief that drove the work, and that is the edit the check exists to catch.

A brief written in Czech stays in Czech. The repository writes in English, but rewriting a brief would forge the record, so the file of a Czech pack is declared in `scripts/ci/english-only-allow.txt` as a fixture with that reason, rather than translated.

## Then, a statement of what must become true

A brief is not yet anything the repository can reason about. The first record that is, is an intent: a typed statement of what must become true, with no plan attached yet.

`intents.propose` writes exactly that. Its input is a statement, plus an optional title and id. It writes one new file under `.ai/repo/project/intents/` with the statement verbatim and empty milestones, criteria and invariants, refuses to overwrite an existing record, and links the file into place so it appears whole or not at all. Such a record has the derived stage `declared`, and no issue can serve it: there is no milestone to place work under and no criterion to serve, so the preflight refuses work that claims it. The decision is [ADR 0144](@/adrs/adr-0144.md), which is still `proposed` — I allowed building on it before accepting it — and case 2978 is the test that holds it.

The alternative the ADR rejects is the tempting one: have the capability write placeholder milestones and criteria so the record validates on day one. That would put invented work into the plan, and nobody reading it later could tell a placeholder from a decision.

One consequence I did not see coming. The statement is written verbatim, and the language gate reads every authored YAML file. A brief pasted in Czech cannot become the statement as it is. Somebody — a person or a model — has to say in English what must become true, and that act of restating is the first place where the brief's meaning can drift. I come back to it below.

## Planning is where a brief becomes refusable

Planning a proposal is a separate act, and it is the step where most of the value is. An intent is planned through three kinds of record:

- **Criteria with evidence.** Each satisfaction criterion is one sentence that a named piece of evidence settles, typically a test file or a case.
- **Milestones and issues.** An issue lives under a milestone and names, in `serves`, the criterion it exists to make true.
- **Dependencies.** An issue lists what it `depends_on`, and the execution waves are derived from that graph: wave zero is every issue with no dependencies, and each issue sits one layer past its deepest one. Waves are computed on every read and stored nowhere, and two issues in the same wave whose scopes overlap are serialised, because a false parallel costs a conflict found after the work is done.

`majordomus intent validate` judges the shape of that plan. Once the intent has any work, a criterion that no live issue serves is refused; an intent with no work yet gets one warning instead. An issue under the intent's milestone that serves nothing is refused too. Work under a milestone no intent names is reported as maintenance, never assigned an invented purpose. The decision is ADR 0073, also still `proposed`; the planning model is described in [How planning works](@/docs/planning.md).

This is the step that turns "the site should feel more alive" into something a gate can say no to. Not because the gate understands the wish, but because every piece of work now has to point at a sentence, and every sentence has to point at a test.

## A critique comes before the work

Before work on an intent starts, the plan gets an adversarial pass, recorded under `.ai/repo/project/critiques/`. Each finding is classed by the question it asks — a missed requirement, an unproven assumption, insufficient or unnecessary work, a regression risk, a surface that must also change, a verification the delivery needs — and each is either blocking or not, and resolved as open, planned into an issue that serves the intent, or rejected with a reason.

The refusal is specific: an issue serving the intent that is active, in verification or done while the intent has no critique, or while a blocking finding is open, fails `intent validate`. Case 386 holds it through the built executable.

The limit is just as specific. A critique names its reviewer in `reviewed_by`, and validation reads that field as text. Two of the critiques on master say plainly that they are self-reviews, each written by the session that wrote the issues it reviews, and that a critique carries more weight when another session or I write it. That is stated rather than hidden, which is right. But it means independence is a sentence in the record, not a property anything decides.

## Uncertainty is reasoned about, not guessed through

A brief often asks for something where the right design is not obvious. That is what `majordomus reasoning` is for, and [Reasoning](@/docs/reasoning.md) is the manual. A session records an assessment: the subject, how material the uncertainty is, its hypothesis and the evidence behind it. An assessment marked material, high or critical is refused without evidence references. A policy then decides whether independent review is worth having; `scripts/advisor-consult` asks whichever advisors it selects, and their answers are evidence the session weighs, never verdicts. A disagreement is settled by an experiment, not by counting votes, and the conclusion is recorded so a later session reads it instead of asking again.

Advisors are optional. A checkout with none installed reasons and decides exactly as before, and says that it did so locally. The decision is [ADR 0098](@/adrs/adr-0098.md), still `proposed` as well.

## The work is offered and claimed, not grabbed

Once issues exist, several sessions on several machines may want them. The mesh, described in [Majordomus Mesh](@/docs/mesh.md), is where that is settled. A handover published on one machine is offered to the others and consumed on another. A claim is a scope of paths held by a session, exclusive by default: `mesh.claim` refuses an exclusive claim that meets a live exclusive claim of another session, naming the claims it meets. When two runtimes claim the same scope without seeing each other, both claims enter the journal, and the fold names the same single winner on every runtime. A claim lasts as long as its holder keeps beating; a runtime that disappears stops holding anything, everywhere, on its own.

That matters for briefs in particular. A pasted brief tends to be pasted into more than one session, and two sessions building the same subsystem in one afternoon is a failure this repository has already had.

## The gate at the end of the path

The last thing between a session and a push is the gates. The one most relevant here is `scripts/ci/english-only-check`, a structure gate that CI runs on every change; the pre-push hook runs the plan's structure gates before a push, within a bound it states. It refuses an authored file containing a letter that occurs in Czech, Slovak or Polish and never in English, or a whole other script. What it decides, and what it does not, is described on [the guarantee page](@/guarantees/authored-files-are-english.md).

The check is honest about its limit in its own header: it tests an alphabet, not a language. A Czech sentence spelled without diacritics passes, and always will. Session records are excluded on purpose, because they quote what was said, and what was said is not always English. So the gate keeps the common case out of the tree — a Czech paragraph pasted into a plan record — and it cannot tell whether a plan says in English what the brief said in Czech.

## What happened on 18 and 19 October

Work planned from what I handed over on those two days sits on a branch that has **not landed on master**. Part of its source was not a pack pasted into a session but conversations of a ChatGPT project, read through my own signed-in session, and their raw capture is kept outside every checkout on purpose, not under `campaigns/`. So this article gives no link to those records, no count from them, and no claim about which steps of this path they went through or what came of them. Until they land, they are not something the site can show you, and the gates on master have not judged them.

The campaigns README already says what that state means: where a plan is not yet on master, the branch carries it. It also means that a session reading master to choose its next piece of work does not see that plan at all, and may plan the same thing again. Landing the plan is part of planning, not an afterthought.

## What this proves, and what it does not yet

<div class="overflow-x-auto" tabindex="0">

| Step | Record | What decides it | What it does not decide |
|---|---|---|---|
| Keep the brief | `campaigns/`, `MANIFEST.sha256` | `scripts/ci/campaigns-check` | whether anything the brief asked for exists |
| State the intent | `.ai/repo/project/intents/` | `intents.propose`, case 2978 | whether the statement says what the brief meant |
| Plan it | criteria, milestones, issues, `serves`, `depends_on` | `majordomus intent validate` | whether a criterion is met — that is the evidence's question |
| Critique it | `.ai/repo/project/critiques/` | `intent validate`, case 386 | whether the reviewer is independent of the author |
| Reason about it | `majordomus reasoning` records | the evidence requirement on a material assessment | whether the conclusion is right |
| Share it out | mesh claims and handovers | `mesh.claim` and the journal's fold | whether the claimant finishes |
| Push it | the authored tree | `scripts/ci/english-only-check` | a language, as opposed to an alphabet |

</div>


Four things are still owed, and I would rather name them than let the table imply otherwise.

The decisions this path rests on — ADR 0144, ADR 0073 and ADR 0098 — are proposed, not accepted. Building on a proposed decision is allowed here, and the code and its tests exist; the acceptance is still my act, and it has not happened.

The restatement from brief to intent is unchecked. The pack is kept verbatim and the intent is validated, but nothing compares the two. If the English statement quietly narrows what the Czech brief asked for, every gate after it stays green.

A critique's independence is declared, not decided. The record says who reviewed, in prose, and the most that can be said today is that the self-reviews on master say so, by convention, not because anything checks.

And the work planned from the briefs of 18 and 19 October has to land. Until its records are on master and its gates have passed over them, this article describes the mechanism, not what it made of those briefs. If the mechanism holds, the follow-up should be a link, not a paragraph.

The same idea, applied to documentation rather than to plans, is the subject of [proof-carrying documentation](@/proof-carrying-documentation.md).
{% endraw %}
