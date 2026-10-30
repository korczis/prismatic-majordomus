+++
title = "Working from the owner's packs"
description = "working from the owner's prompt packs: how a brief kept under `campaigns/` becomes milestones and issues that cite it, the packs of 2026-10-11 and 2026-10-12 with the master records each one maps onto, what was not planned and which decision or record says why, where to start, and the gate that holds packs and plan together"
weight = 65
[extra]
source = "docs/OWNER_PACKS.md"
+++

{% raw %}

A prompt pack is a brief the owner handed over: what was wanted, on a date, in an order. The
repository keeps every pack byte for byte under `campaigns/`, and it does not execute them.
What a pack asked for becomes the repository's own plan — milestones and issues under
`.ai/repo/project/` — and the plan is what a worker starts from. This page says how the two
are connected, maps the packs of 2026-10-11 and 2026-10-12 onto the records master holds, and
says where to begin.

## For whom

- **A worker picking up work.** Start from an issue, not from a pack. The issue says what was
  measured, what to change and how it is decided; the pack is where to read why it was asked.
- **The owner.** Each pack below names the records that came of it, the master records its asks
  were folded into, and what in it was not planned because the repository already has it or
  has decided otherwise.
- **A reviewer.** A record cites its pack by path, so the brief behind a change can be opened.

## How a pack becomes work

<pre class="mermaid">
flowchart LR
  zip["a pack in ~/Downloads"] --&gt; kept["campaigns/&amp;lt;pack&amp;gt;&lt;br&gt;kept byte for byte"]
  kept --&gt; manifest["MANIFEST.sha256&lt;br&gt;and the index row"]
  kept --&gt; measured["measured against the tree:&lt;br&gt;what exists, what is planned, what is decided"]
  measured --&gt; records["milestones and issues&lt;br&gt;.ai/repo/project/"]
  records -- "cites campaigns/&amp;lt;pack&amp;gt;" --&gt; kept
  records --&gt; roadmap["roadmap, plan pages,&lt;br&gt;the UI's Work screen"]
  records --&gt; github["GitHub milestones and issues&lt;br&gt;scripts/github-sync"]
  gate["scripts/ci/campaigns-check"] -. "bytes, index,&lt;br&gt;cited or unplanned" .-&gt; kept
  gate -.-&gt; records
</pre>


1. **Kept.** The pack directory is copied in unmodified; `campaigns/MANIFEST.sha256` records
   its bytes and `campaigns/README.md` its row.
2. **Measured.** Before anything is planned, each thing the pack asks for is checked against
   the tree and against the plan. Much of it usually exists already, some is planned by a
   record written from another brief, and some contradicts a decision the repository has made.
   None of those becomes a new issue.
3. **Planned or folded.** What remains is written as issues of about one pull request each,
   under the milestone master already holds for that subject where there is one. An ask that a
   master issue already plans is folded into that issue: its missing criteria are added there,
   and the record's `why` cites the pack file. Every record cites the pack file it came from.
4. **Projected.** The roadmap, the plan pages of the site, `docs/PLAN_STATUS.md` and the UI
   are derived from the records. GitHub receives them when `scripts/github-sync --apply` is
   run on the trunk.
5. **Held.** `scripts/ci/campaigns-check` fails when a record cites a pack that is not kept,
   when a pack is neither cited by a record nor named in
   `.ai/repo/ci/campaigns-unplanned.txt` with the reason, and when a line of that list names a
   pack a record has come to cite, so the list only shrinks.

## The packs of 2026-10-11 and 2026-10-12, mapped onto master

These packs were first planned on 2026-10-13 by PR #1058 as ten milestones and sixty-three
issues, which never landed. Master moved two weeks past that measurement and came to plan or
deliver part of it from other briefs, so on 2026-10-30 each of those records was measured
again against master 51b38035a71 and either carried under a master milestone, folded into the
master issue that already plans it, or dropped because master delivered it. Carried records
take new ids (I4480-I4511); the ids of 2026-10-13 are not reused.

<div class="overflow-x-auto" tabindex="0">

| pack | carried as | under | folded into or delivered by |
|---|---|---|---|
| `campaigns/majordomus-incremental-derive-prompt-pack` | none: master holds I2521-I2533 | `a-derive-does-only-the-work-its-inputs-changed`, `a-derivation-computed-once-is-reused` | landed with those records |
| `campaigns/majordomus-autonomous-idd-maximal` | I4480, I4481, I4482 | `the-runtime-supervises-itself`, `runtime-trust`, `the-intent-loop-is-proved-end-to-end` | I3971, I3256, I3257, I3203 |
| `campaigns/majordomus-intent-ui-pack` | I4483, I4484, I4485 | `the-ui-runs-and-explains`, `idd-reads-at-a-glance` | I3225, I2902, I3252, I3260; delivered by I2781 and I2789; I4000 |
| `campaigns/majordomus-shellfall` | I4486-I4492 | `automation`, `repository-automation-is-typed` | I1801, I1804, I3155, I3973, I3304; delivered by I1800 |
| `campaigns/majordomus-ui-speed-pack` | I4493-I4499 | `a-screen-is-fast-bounded-and-says-why` (new) | I3227, I3226 |
| `campaigns/majordomus-profile-system-v2-prompt-pack` | I4500-I4511 | `a-profile-is-resolved-once-and-enforced` (new) | none: nothing on master plans a resolver |
| `campaigns/majordomus-convergence-pack-20261012` | none | none: it was executed as the campaign `majordomus-autonomous-convergence` | named in `.ai/repo/ci/campaigns-unplanned.txt` |

</div>


Two later packs are planned elsewhere and say so in `.ai/repo/ci/campaigns-unplanned.txt`:
`majordomus-convergence-pack-2026-10-14` became I2700-I2709 and the issues
`campaigns/README.md` lists, and `majordomus-mission-control-pack-2026-10-22` became
`a-session-proceeds-through-the-mesh`, I4000 and on, whose objectives name its findings B1-B6.

### Each record of 2026-10-13, and where it went

<div class="overflow-x-auto" tabindex="0">

| 2026-10-13 | now | why |
|---|---|---|
| I2502 plan dispatch | folded into I3971 | I3971 makes plan next pass over work that is taken; the exclusive-claim, non-parallel-safe, incomplete-board and determinism criteria were added there. Its cap on open pull requests is not carried: I3204 and ADR 0197 (proposed) make the backlog bound report, not refuse |
| I2503 an execution survives a restart | I4480 | nothing plans a journal; it reuses I4001's UnknownOutcome instead of a state of its own |
| I2505 an effect ceiling per transport | I4481 | nothing plans it; it shares one policy section over effect classes with I1206 |
| I2506 a deployment settles a criterion | folded into I3256 | I3256 revives it; two criteria added |
| I2507 intent explain reaches the deployment | folded into I3257 | I3257 revives it; three criteria added |
| I2508 the wait from merge to deployment | folded into I3203 | I3203 measures every delivery phase; three criteria added |
| I2511 the sentence-to-verdict case reaches a deployment | I4482 | it continues I3260's case past the merge and across a restart |
| I2536 an execution opens on the UI | I4483 | no /ui route answers one execution |
| I2537 a capability starts from its UI inspector | folded into I3225 | three criteria added |
| I2538 one key, one execution | planned by I4000 | `chore/a-session-proceeds-through-the-mesh-is-planned` carries I4000 differently, so nothing was added to it here; its missing asks are listed below |
| I2539 an issue's moves on its inspector | delivered by I2781; folded into I2902 | one criterion added: the issue inspector links to its move |
| I2540 a mesh node offers cooperation | not carried | I3139 and I3192 decide one action, mesh.review.request; a panel of every mesh command contradicts that |
| I2541 an intent opens its gap and critique | I4484 | the realisation panel was delivered by I2789; gap and critique are shown nowhere |
| I2542, I2543 proposals written by capabilities | folded into I3252 | six criteria added; I3252 allocates ids itself, which the 2026-10-13 records left to the caller |
| I2544 an issue opens its trace | I4485 | the inspector is /ui/prs?issue= and reads no trace |
| I2545 the sentence-to-verdict case | folded into I3260 | I3260 revives it; two criteria added |
| I2549 ADR 0069 on master and a gate for dangling ADR numbers | folded into I1801 and I3973 | ADR 0069 landed in fbb9aaf4763; I3973 plans the gate, and its baseline now names the nine numbers cited with no record |
| I2550 the shell baseline | folded into I3155 | five criteria added |
| I2551 the engine is part of the executable | dropped: delivered as I1800 (7ac2fde4a45) | its untested limits moved to I1801 |
| I2552 a scripted capability is a kind | folded into I1801 | the same objective; four criteria added |
| I2553 a program is run by one typed capability | I4486 | nothing plans an argument-list process runner |
| I2554 the ratchet as one typed capability | folded into I1804 | four criteria added |
| I2555 a hand-written recipe is one call | I4487 | it narrows I3154's allowance, which it depends on |
| I2556 a behavioural case may be a Rust test | I4488 | I3160 ports cases with no mapping or count ratchet |
| I2557 the first ten Rust-only cases ported | I4489 | three of its named cases run bin/majordomus on master; three others are chosen by rule |
| I2558 the lifecycle units leave lib | I4490 | master plans the capabilities and keeps the shell; this deletes it after parity |
| I2559 embedded shell is judged | I4491 | nothing compares an anchor with its file |
| I2560 an exemption names its issue | I4492 | no exemption names an issue |
| I2561 a provider hook reaches the executable | folded into I3304 (with I3143) | two criteria added |
| I2563, I2565, I2566, I2568, I2569, I2570 | I4493, I4494, I4495, I4497, I4498, I4499 | none is planned on master |
| I2564 a pointed-at link is read ahead | folded into I3227 | five criteria added |
| I2567 a reading explains itself | I4496, narrowed to perf.readings | its Server-Timing half is folded into I3226 (two criteria) |
| I2573-I2584 the profile resolver | I4500-I4511 | nothing on master resolves a profile; I0201-I0206 hold only the runtime limits |

</div>


The milestones of 2026-10-13 that are not carried are mapped, not lost:
`started-work-is-explained-bounded-and-survives-a-restart` and
`an-intent-is-followed-to-the-deployment-that-carries-it` are spread over the master milestones
above, `the-ui-starts-work-through-the-executor` is `the-ui-runs-and-explains`,
`an-intent-is-carried-from-a-sentence-to-a-verdict` is `idd-reads-at-a-glance`,
`an-outcome-becomes-a-reviewed-plan` and `the-intent-loop-is-proved-end-to-end`,
`automation-is-scripted-capabilities` is `automation`, and `shell-shrinks-to-its-bootstrap` is
`repository-automation-is-typed` with the case work in `automation`.

### Asks that a branch, not master, now holds

I4000 is carried differently by `chore/a-session-proceeds-through-the-mesh-is-planned`, so these
asks of `campaigns/majordomus-intent-ui-pack/02-ACTION-PLANE.md` were not written into it, and
whoever lands that branch should weigh them: the same key with another capability or input is
refused as invalid input and starts nothing; the key is compared after redaction and leaks no
value; a form mints one key per render; `docs/EXECUTIONS.md` says a key is forgotten with its
execution; and two concurrent starts answer one id. I3260's ask that a repeated start of
plan.transition answer the first id would add a dependency on I4000 and waits for it.

## What was not planned, and why

Each pack was read as a brief, and a brief can ask for what the repository has decided
against. These are recorded so that nobody plans them again by accident.

<div class="overflow-x-auto" tabindex="0">

| the pack asks for | the repository's decision | so |
|---|---|---|
| a UI that changes things itself (an "action plane") | a screen reads and never executes (ADR 0142) | the UI posts a form to the executor's own route (I3225) |
| executions with queues, retries, pause and a scheduler | an execution is a watched capability call (ADR 0033) | a journal across a restart (I4480) and plan next explaining what is taken (I3971) |
| the tool proposing criteria and plans by itself | the tool does not reason; a worker writes records and capabilities validate them (ADR 0144, amended by the ADR I3252 cites) | writers that accept only what the caller supplies (I3252, I3396) |
| stored intent stages | no status is stored; a stage is derived (ADR 0070) | nothing |
| merge, release and deploy buttons | the lane executor merges, one pull request at a time (ADR 0101) | nothing |
| a refusal of new work above an open pull request cap | the backlog bound reports and does not refuse (I3204, ADR 0197 proposed) | nothing, unless the owner rules otherwise |
| page and fragment caches | a cache is invisible; readings are kept, pages are not (ADR 0142) | conditional requests from a screen's readings (I4493) |
| a workflow engine with its own nouns | a capability may be scripted, and nothing else is added (ADR 0069) | the scripted-capability kind (I1801) |
| every recipe generated, shell removed at once | the shell half is what the installer ships and what the hooks run | an ordered migration with counts that cannot rise (I3155, I4488, I4492) |
| peer scheduling and provider-side enforcement of a profile | the tool starts no worker and invokes no model (ADR 0032, ADR 0044) | one refusal at the capability executor (I4506) |
| every mesh command offered on a node's inspector | one cooperation action, under a resolved identity (I3139, I3192) | nothing beyond those |

</div>


## Starting

```bash
majordomus plan next
majordomus plan validate
```

`plan next` names the issue to take; `plan validate` checks the graph. An issue's record is
`.ai/repo/project/issues/<id>.yaml`: read its `current_state` before touching code, because it
says what was measured on 2026-10-30 and the tree moves. Work under a task whose scope is the
issue's `scope`, claim the paths on the board before building, and allocate ADR and case
numbers from the allocators; no record here reserves one.

Read these before the issues that lean on them:

- **I4001** adds the execution states I4480 and I4482 rely on.
- **I3123, I3128, I3199 and I3395** assume an effective profile that I4502 is the first to
  resolve; **I0201-I0206** (`runtime-adapters`) hold the runtime limits and must consume the
  resolver rather than resolve a profile again.
- **I3154** decides how just is derived; I4487 narrows it and depends on it.

## Adding the next pack

Copy it in as `campaigns/README.md` says, regenerate the manifest, add its row. Then measure
each ask against the tree and the plan, fold what a master issue already plans into that
issue, write the rest as records with their citations, add the planning row, and run the gate.
A pack that is executed directly, with no records, is named in
`.ai/repo/ci/campaigns-unplanned.txt` with that reason.

## Where it is

<div class="overflow-x-auto" tabindex="0">

| what | where |
|---|---|
| the packs, their index and their bytes | `campaigns/`, `campaigns/README.md`, `campaigns/MANIFEST.sha256` |
| the plan | `.ai/repo/project/milestones/`, `.ai/repo/project/issues/` |
| packs with no plan records, and why | `.ai/repo/ci/campaigns-unplanned.txt` |
| the gate | `scripts/ci/campaigns-check`, `test/cases/340_a_brief_is_kept_as_received.sh` |
| the projection to GitHub | `scripts/github-sync`, `scripts/ci/github-check` |
| the rule | `.ai/repo/rules/project/a-brief-is-kept-as-received.v1.md` |

</div>

{% endraw %}
