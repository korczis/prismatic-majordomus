---
schema: knowledge/v1
id: the-chatgpt-project-was-promoted
kind: knowledge
class: fact
title: "The owner's ChatGPT project was read once in full on 2026-10-18, and every record it became names the messages it came from"
description: "The coverage and provenance ledger of the first promotion of workspace chatgpt-majordomus: what each conversation became, and where each promoted record came from."
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - chatgpt-project
  - provenance
  - workspace
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/workspaces/chatgpt-majordomus.yaml
---

# The ChatGPT project was promoted on 2026-10-18

The owner's ChatGPT Project *Majordomus* (workspace `chatgpt-majordomus`) held 297 conversations, created from 2026-09-03 to 2026-10-18. On 2026-10-18 every one of them was read through the owner's own signed-in session, extracted message by message, verified against master and the peer board, critiqued per intent, interlinked, and promoted the Majordomus way (issue I3100). The raw conversations are kept outside every checkout, in the operator's own data directory, until the workspace store of I1303 exists; they are never committed, and this note and the records carry identifiers only.

A source is written `chatgpt-majordomus:<alias>#<n>`. The alias (`c001`, `c002`, ...) numbers the conversations in the order they were created and is never renumbered; the map from alias to the vendor's conversation id is kept only in the operator's own store, so this public tree names no conversation id. `n` is the message's position in the conversation's current branch at the time of reading; a position that was not captured is never cited. Positions move if a message is edited or regenerated upstream, and pinning them to the vendor's message ids is planned work of milestone `a-promoted-record-opens-its-message`. Intents trace through the milestones and issues that realise them.

## What the conversations were

| relevance | conversations |
|---|---|
| core | 246 |
| related | 41 |
| offtopic | 10 |

Off-topic conversations are counted and not listed: nothing of them was promoted.

## What they became

| kind | records that cite a conversation |
|---|---|
| issue | 199 |
| knowledge | 44 |
| adr | 37 |
| milestone | 35 |

Everything else was found already recorded on master, superseded by a later decision, left as an open question for the owner, handed to the session that owns the record (on the mesh), or counted as noise.

## Every conversation

| created | conversation | relevance | promoted into |
|---|---|---|---|
| 2026-09-03 | `c001` | core | I3133, I3134, I3319 |
| 2026-09-04 | `c002` | core | none: found already recorded, superseded, or an open question |
| 2026-09-04 | `c003` | core | I3310 |
| 2026-09-04 | `c004` | core | none: found already recorded, superseded, or an open question |
| 2026-09-04 | `c005` | core | none: found already recorded, superseded, or an open question |
| 2026-09-04 | `c006` | core | none: found already recorded, superseded, or an open question |
| 2026-09-04 | `c007` | core | none: found already recorded, superseded, or an open question |
| 2026-09-04 | `c008` | core | I3314 |
| 2026-09-04 | `c009` | core | none: found already recorded, superseded, or an open question |
| 2026-09-04 | `c010` | core | none: found already recorded, superseded, or an open question |
| 2026-09-04 | `c011` | related | none: found already recorded, superseded, or an open question |
| 2026-09-04 | `c012` | core | none: found already recorded, superseded, or an open question |
| 2026-09-04 | `c013` | core | none: found already recorded, superseded, or an open question |
| 2026-09-04 | `c014` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c015` | core | I3213 |
| 2026-09-05 | `c016` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c017` | core | I3238 |
| 2026-09-05 | `c018` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c019` | core | I3239, I3240 |
| 2026-09-05 | `c020` | related | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c021` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c022` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c023` | related | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c024` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c025` | core | I3241 |
| 2026-09-05 | `c026` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c027` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c028` | core | I3309 |
| 2026-09-05 | `c029` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c030` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c031` | core | I3111, I3305 |
| 2026-09-05 | `c032` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c033` | core | I3273 |
| 2026-09-05 | `c034` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c035` | core | I3305 |
| 2026-09-05 | `c036` | core | none: found already recorded, superseded, or an open question |
| 2026-09-05 | `c037` | core | I3238 |
| 2026-09-06 | `c038` | related | I3134 |
| 2026-09-06 | `c039` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c040` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c041` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c042` | core | I3315 |
| 2026-09-06 | `c043` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c044` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c045` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c046` | core | I3298 |
| 2026-09-06 | `c047` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c048` | core | I3300 |
| 2026-09-06 | `c049` | core | I3311 |
| 2026-09-06 | `c050` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c051` | core | I3212 |
| 2026-09-06 | `c052` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c053` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c054` | core | I3273 |
| 2026-09-06 | `c055` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c056` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c057` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c059` | core | none: found already recorded, superseded, or an open question |
| 2026-09-06 | `c060` | core | none: found already recorded, superseded, or an open question |
| 2026-09-07 | `c061` | core | none: found already recorded, superseded, or an open question |
| 2026-09-07 | `c062` | core | I3116 |
| 2026-09-07 | `c063` | core | none: found already recorded, superseded, or an open question |
| 2026-09-07 | `c064` | related | none: found already recorded, superseded, or an open question |
| 2026-09-07 | `c065` | core | I3285 |
| 2026-09-07 | `c066` | core | I3279, I3286 |
| 2026-09-07 | `c067` | core | none: found already recorded, superseded, or an open question |
| 2026-09-07 | `c068` | core | none: found already recorded, superseded, or an open question |
| 2026-09-07 | `c069` | core | I3127, I3321 |
| 2026-09-08 | `c070` | core | none: found already recorded, superseded, or an open question |
| 2026-09-08 | `c071` | core | none: found already recorded, superseded, or an open question |
| 2026-09-08 | `c072` | core | none: found already recorded, superseded, or an open question |
| 2026-09-08 | `c073` | core | none: found already recorded, superseded, or an open question |
| 2026-09-08 | `c074` | core | I3313 |
| 2026-09-09 | `c076` | core | none: found already recorded, superseded, or an open question |
| 2026-09-09 | `c077` | core | I3316 |
| 2026-09-09 | `c078` | core | none: found already recorded, superseded, or an open question |
| 2026-09-09 | `c079` | core | I3121 |
| 2026-09-09 | `c080` | core | none: found already recorded, superseded, or an open question |
| 2026-09-09 | `c081` | core | none: found already recorded, superseded, or an open question |
| 2026-09-09 | `c082` | core | I3138, I3199, I3295, I3317 |
| 2026-09-09 | `c083` | core | none: found already recorded, superseded, or an open question |
| 2026-09-09 | `c084` | core | I3119 |
| 2026-09-09 | `c085` | core | I3309 |
| 2026-09-09 | `c086` | core | I3119, I3309 |
| 2026-09-09 | `c087` | core | I3120, I3122, I3238, I3309 |
| 2026-09-09 | `c088` | core | I3273, I3312 |
| 2026-09-09 | `c089` | core | I3208, I3238 |
| 2026-09-10 | `c090` | core | none: found already recorded, superseded, or an open question |
| 2026-09-10 | `c091` | core | none: found already recorded, superseded, or an open question |
| 2026-09-10 | `c092` | core | none: found already recorded, superseded, or an open question |
| 2026-09-10 | `c093` | core | I3293, I3312 |
| 2026-09-10 | `c094` | core | none: found already recorded, superseded, or an open question |
| 2026-09-10 | `c095` | core | none: found already recorded, superseded, or an open question |
| 2026-09-10 | `c096` | core | I3179, I3196 |
| 2026-09-10 | `c097` | core | I3148, entry-proves-what-it-handed, the-session-domain-is-written-once |
| 2026-09-10 | `c098` | core | a-claim-stands-only-on-its-evidence, declared-is-not-enforced |
| 2026-09-10 | `c099` | core | I3151, I3304 |
| 2026-09-10 | `c100` | core | I3122, finished-work-reaches-a-verified-release |
| 2026-09-10 | `c101` | core | none: found already recorded, superseded, or an open question |
| 2026-09-10 | `c102` | core | I3122, I3190, I3228, I3230 |
| 2026-09-10 | `c103` | core | I3112, I3147, I3149, I3150, I3152, adr-0181, finished-work-reaches-a-verified-release |
| 2026-09-10 | `c104` | core | none: found already recorded, superseded, or an open question |
| 2026-09-10 | `c105` | core | I3154, I3155, repository-automation-is-typed |
| 2026-09-10 | `c106` | core | none: found already recorded, superseded, or an open question |
| 2026-09-11 | `c107` | related | none: found already recorded, superseded, or an open question |
| 2026-09-11 | `c108` | related | none: found already recorded, superseded, or an open question |
| 2026-09-11 | `c109` | core | I3111, I3113, I3141, I3143, I3144, I3303, I3320, a-claim-stands-only-on-its-evidence and 3 more |
| 2026-09-11 | `c110` | core | I3157, repository-automation-is-typed |
| 2026-09-11 | `c111` | core | I3115, I3121, I3208, a-release-is-proven-end-to-end, a-test-fails-when-the-code-is-wrong, finished-work-reaches-a-verified-release |
| 2026-09-11 | `c112` | core | I3113, I3200, I3204, a-partial-green-is-not-the-verdict, delivery-flow-is-measured, finished-work-reaches-a-verified-release |
| 2026-09-11 | `c113` | core | I3120, I3175, I3200, a-test-fails-when-the-code-is-wrong, adr-0190, mesh-peers-execute-assigned-work |
| 2026-09-11 | `c114` | core | positioning-leads-with-the-outcome |
| 2026-09-11 | `c115` | core | I3110, I3112, a-claim-stands-only-on-its-evidence, adr-0188, declared-is-not-enforced |
| 2026-09-11 | `c116` | core | I3142, I3145, I3146, a-test-consistent-with-the-code-is-not-a-test-of-the-contract, adr-0182, the-session-domain-is-written-once |
| 2026-09-11 | `c117` | core | a-render-is-an-experience-target-not-data |
| 2026-09-11 | `c118` | core | I3195, I3196, adr-0199, adr-0204, chatgpt-prompts-assumed-phoenix, lundra-is-the-always-on-hub, the-hub-is-reached-from-any-device, the-mesh-is-operated-from-any-device |
| 2026-09-11 | `c119` | core | I3130, I3132, I3133, adr-0207, adr-0208, declared-is-not-enforced, the-product-shows-what-it-caught |
| 2026-09-11 | `c120` | core | I3119 |
| 2026-09-12 | `c121` | core | none: found already recorded, superseded, or an open question |
| 2026-09-12 | `c122` | core | I3254 |
| 2026-09-12 | `c123` | core | I3261 |
| 2026-09-12 | `c124` | core | a-claim-stands-only-on-its-evidence |
| 2026-09-12 | `c125` | core | I3225 |
| 2026-09-12 | `c126` | core | I3127, I3133, I3321, a-brief-is-reconciled-not-transcribed |
| 2026-09-12 | `c127` | core | finished-work-reaches-a-verified-release |
| 2026-09-12 | `c128` | core | I3153 |
| 2026-09-12 | `c129` | core | I3110, I3260, adr-0188, declared-is-not-enforced |
| 2026-09-12 | `c130` | core | I3111, I3113, I3200, I3234, I3238, a-claim-stands-only-on-its-evidence, adr-0185, adr-0186 and 5 more |
| 2026-09-12 | `c131` | core | none: found already recorded, superseded, or an open question |
| 2026-09-12 | `c132` | core | a-prompt-pack-in-the-owners-word |
| 2026-09-13 | `c133` | related | none: found already recorded, superseded, or an open question |
| 2026-09-13 | `c134` | core | I3223 |
| 2026-09-13 | `c135` | core | I3111, a-claim-stands-only-on-its-evidence, adr-0188, declared-is-not-enforced |
| 2026-09-15 | `c136` | related | none: found already recorded, superseded, or an open question |
| 2026-09-15 | `c137` | core | I3128, I3223, a-stranger-adopts-it-unaided, adr-0206, public-writing-is-evidence-led |
| 2026-09-15 | `c138` | core | I3176, I3191, the-mesh-is-operated-from-any-device, the-mesh-is-operated-from-one-page |
| 2026-09-15 | `c139` | core | I3110, I3111, I3112, I3120, a-claim-stands-only-on-its-evidence, a-test-fails-when-the-code-is-wrong, adr-0188, adr-0190 and 1 more |
| 2026-09-15 | `c140` | core | I3159, repository-automation-is-typed |
| 2026-09-15 | `c141` | core | I3247 |
| 2026-09-15 | `c142` | core | a-claim-stands-only-on-its-evidence, finished-work-reaches-a-verified-release, integration-capacity-bounds-the-backlog |
| 2026-09-15 | `c143` | core | I3119, I3122, a-claim-stands-only-on-its-evidence, evidence-machinery-must-end-in-a-shipped-result |
| 2026-09-15 | `c144` | core | I3111, I3166, adr-0188 |
| 2026-09-15 | `c145` | core | I3182 |
| 2026-09-15 | `c146` | core | I3251, adr-0191 |
| 2026-09-15 | `c147` | core | I3119, a-claim-stands-only-on-its-evidence, declared-is-not-enforced, operation-convergence |
| 2026-09-15 | `c148` | related | I3114 |
| 2026-09-15 | `c149` | related | declared-is-not-enforced |
| 2026-09-16 | `c152` | core | none: found already recorded, superseded, or an open question |
| 2026-09-16 | `c153` | core | none: found already recorded, superseded, or an open question |
| 2026-09-16 | `c154` | core | I3260, mesh-peers-execute-assigned-work |
| 2026-09-16 | `c155` | core | none: found already recorded, superseded, or an open question |
| 2026-09-16 | `c156` | core | I3134, features-are-category-hygiene, positioning-leads-with-the-outcome |
| 2026-09-16 | `c157` | related | none: found already recorded, superseded, or an open question |
| 2026-09-16 | `c158` | related | I3120, I3122, a-test-fails-when-the-code-is-wrong, adr-0190 |
| 2026-09-17 | `c159` | related | I3117, public-writing-is-evidence-led |
| 2026-09-20 | `c160` | core | I3199, who-when-how-are-separate-axes |
| 2026-09-23 | `c162` | core | I3298 |
| 2026-09-24 | `c164` | core | none: found already recorded, superseded, or an open question |
| 2026-09-24 | `c165` | related | none: found already recorded, superseded, or an open question |
| 2026-09-24 | `c166` | core | I3190 |
| 2026-09-24 | `c167` | core | none: found already recorded, superseded, or an open question |
| 2026-09-29 | `c168` | core | I3230 |
| 2026-09-30 | `c169` | core | none: found already recorded, superseded, or an open question |
| 2026-09-30 | `c170` | core | none: found already recorded, superseded, or an open question |
| 2026-10-01 | `c171` | related | none: found already recorded, superseded, or an open question |
| 2026-10-01 | `c172` | related | I3228 |
| 2026-10-01 | `c173` | core | none: found already recorded, superseded, or an open question |
| 2026-10-02 | `c174` | core | I3117, I3273, I3300, I3302, I3322, I3323, a-provider-carries-the-contract-natively, adr-0213 and 3 more |
| 2026-10-02 | `c175` | related | none: found already recorded, superseded, or an open question |
| 2026-10-02 | `c176` | core | I3133, I3257, features-are-category-hygiene |
| 2026-10-02 | `c177` | related | positioning-leads-with-the-outcome |
| 2026-10-02 | `c178` | related | none: found already recorded, superseded, or an open question |
| 2026-10-02 | `c179` | related | I3133, declared-is-not-enforced |
| 2026-10-03 | `c180` | core | a-generator-that-can-list-everything-shows-everything, overview-first-evidence-one-click-away |
| 2026-10-03 | `c181` | core | none: found already recorded, superseded, or an open question |
| 2026-10-03 | `c182` | related | none: found already recorded, superseded, or an open question |
| 2026-10-03 | `c183` | core | finished-work-reaches-a-verified-release |
| 2026-10-04 | `c184` | related | none: found already recorded, superseded, or an open question |
| 2026-10-04 | `c185` | core | I3252, adr-0191 |
| 2026-10-04 | `c186` | core | I3228, overview-first-evidence-one-click-away |
| 2026-10-06 | `c187` | core | I3225, the-ui-runs-and-explains |
| 2026-10-07 | `c188` | core | none: found already recorded, superseded, or an open question |
| 2026-10-07 | `c189` | core | I3260, I3261, the-intent-loop-was-built-on-branches-that-never-landed |
| 2026-10-07 | `c190` | related | I3117 |
| 2026-10-07 | `c191` | related | I3230, features-are-category-hygiene |
| 2026-10-07 | `c192` | core | I3160, I3161, adr-0184, repository-automation-is-typed |
| 2026-10-08 | `c193` | core | I3110, I3111, I3113, I3116, I3117, I3123, I3129, I3135 and 15 more |
| 2026-10-08 | `c194` | related | I3133, I3300, a-provider-carries-the-contract-natively, adr-0214 |
| 2026-10-08 | `c195` | core | I3238, I3239, I3242, I3245, a-speed-up-that-hides-a-check-is-not-one, adr-0187, adr-0196, finished-work-reaches-a-verified-release |
| 2026-10-08 | `c196` | core | I3247, I3255, I3260, adr-0192, adr-0193, what-idd-is-not |
| 2026-10-08 | `c197` | core | I3163, I3164, I3214, I3215, I3217, adr-0183, adr-0215, the-ui-works-on-a-phone |
| 2026-10-08 | `c198` | core | I3119, I3133, I3247, I3250, I3260, I3276, I3295, a-brief-is-reconciled-not-transcribed and 2 more |
| 2026-10-08 | `c199` | core | I3118, I3202, a-partial-green-is-not-the-verdict, delivery-flow-is-measured |
| 2026-10-08 | `c200` | core | I3177, I3178, I3192 |
| 2026-10-09 | `c201` | related | none: found already recorded, superseded, or an open question |
| 2026-10-09 | `c202` | core | none: found already recorded, superseded, or an open question |
| 2026-10-09 | `c203` | core | I3174 |
| 2026-10-09 | `c204` | core | I3203, I3206, adr-0196, delivery-flow-is-measured |
| 2026-10-09 | `c205` | core | I3246, I3248, I3251, I3256, adr-0191, adr-0192 |
| 2026-10-10 | `c206` | core | none: found already recorded, superseded, or an open question |
| 2026-10-10 | `c207` | core | I3110, I3205, adr-0188, adr-0197, delivery-flow-is-measured, finished-work-reaches-a-verified-release |
| 2026-10-10 | `c208` | core | I3220, I3237, a-change-is-judged-in-minutes, adr-0196, every-surface-follows-what-changed |
| 2026-10-10 | `c209` | core | I3207, adr-0197, delivery-flow-is-measured, finished-work-reaches-a-verified-release |
| 2026-10-10 | `c210` | core | I3119, I3169, I3170, I3174, I3273, I3274, I3275, adr-0180 and 1 more |
| 2026-10-10 | `c211` | core | none: found already recorded, superseded, or an open question |
| 2026-10-10 | `c213` | core | I3298 |
| 2026-10-10 | `c214` | core | I3110, adr-0188, declared-is-not-enforced, finished-work-reaches-a-verified-release |
| 2026-10-10 | `c215` | core | I3223, I3224, I3324, a-visitor-understands-and-checks-majordomus, read-the-ui-rules-before-proposing-a-ui |
| 2026-10-10 | `c216` | core | I3203, I3205, I3216, I3260, adr-0185, delivery-flow-is-measured, the-ui-works-on-a-phone |
| 2026-10-10 | `c217` | core | I3111, adr-0188, positioning-leads-with-the-outcome |
| 2026-10-10 | `c218` | core | I3136 |
| 2026-10-11 | `c219` | related | none: found already recorded, superseded, or an open question |
| 2026-10-11 | `c220` | core | adr-0201, finished-work-reaches-a-verified-release, integration-capacity-bounds-the-backlog |
| 2026-10-11 | `c221` | core | I3247, I3249, I3258, I3260, adr-0192, adr-0193, operation-convergence |
| 2026-10-11 | `c222` | core | I3308, a-provider-carries-the-contract-natively |
| 2026-10-11 | `c223` | core | I3226, I3227, the-ui-runs-and-explains |
| 2026-10-11 | `c224` | related | an-adopters-work-never-waits-for-majordomus |
| 2026-10-11 | `c225` | core | I3156, I3225, adr-0216 |
| 2026-10-11 | `c226` | core | I3133, I3172, I3252, mesh-peers-execute-assigned-work |
| 2026-10-11 | `c227` | core | I3218, I3228, adr-0197, adr-0216, finished-work-reaches-a-verified-release, integration-capacity-bounds-the-backlog |
| 2026-10-11 | `c228` | core | I3238, I3239, I3240, I3241, I3245, a-change-is-judged-in-minutes, adr-0186, adr-0187 and 2 more |
| 2026-10-12 | `c229` | core | none: found already recorded, superseded, or an open question |
| 2026-10-12 | `c230` | core | I3154, I3155, I3157, I3160, repository-automation-is-typed, rhai-limits-are-not-a-clock |
| 2026-10-12 | `c231` | core | I3298, declared-is-not-enforced |
| 2026-10-12 | `c232` | core | I3220, every-surface-follows-what-changed |
| 2026-10-12 | `c233` | core | I3156, I3179, I3180, adr-0184 |
| 2026-10-12 | `c234` | core | I3293, adr-0213, claude-code-reports-usage-through-opentelemetry, every-sessions-usage-is-observed |
| 2026-10-12 | `c235` | core | I3224, I3229, I3230, a-visitor-understands-and-checks-majordomus |
| 2026-10-12 | `c236` | core | I3288, I3289, I3293, I3294, I3295, I3296, I3297, I3300 and 4 more |
| 2026-10-12 | `c237` | core | I3187, I3189, adr-0202, an-authorised-node-connects-from-anywhere, mesh-connectivity |
| 2026-10-12 | `c238` | core | I3203, I3208, a-release-is-proven-end-to-end, delivery-flow-is-measured, finished-work-reaches-a-verified-release |
| 2026-10-12 | `c239` | core | I3173, I3254, I3283, I3285, I3286, a-promoted-record-opens-its-message, a-workspace-reaches-the-repository, adr-0194 and 3 more |
| 2026-10-12 | `c240` | core | I3177, I3184, I3185, I3189, I3195, adr-0199, the-host-knows-its-runtimes |
| 2026-10-12 | `c241` | core | I3111, I3127, I3129, I3321, a-stranger-adopts-it-unaided, adr-0188, adr-0206, positioning-leads-with-the-outcome |
| 2026-10-12 | `c242` | core | I3212, finished-work-reaches-a-verified-release |
| 2026-10-12 | `c243` | related | none: found already recorded, superseded, or an open question |
| 2026-10-13 | `c244` | core | I3157, I3159, I3305, I3306, a-second-implementation-drifts, adr-0196, finished-work-reaches-a-verified-release, repository-automation-is-typed and 1 more |
| 2026-10-13 | `c245` | core | I3168, I3169, I3198, I3225, I3252, I3272, I3276, adr-0180 and 3 more |
| 2026-10-13 | `c246` | core | I3124, I3125, adr-0189 |
| 2026-10-13 | `c247` | related | I3253, adr-0180, the-owners-shorthand-names-contracts |
| 2026-10-13 | `c248` | related | none: found already recorded, superseded, or an open question |
| 2026-10-13 | `c249` | related | positioning-leads-with-the-outcome |
| 2026-10-13 | `c250` | core | I3134, I3135, I3136, I3230, a-dashboard-is-cheap-continuity-is-not, adr-0210, positioning-leads-with-the-outcome, public-writing-is-evidence-led |
| 2026-10-13 | `c252` | related | I3171, I3298, naive-fan-out-burns-tokens |
| 2026-10-13 | `c253` | core | I3287, I3288, I3289, I3290, I3291, I3292, I3294, adr-0213 and 2 more |
| 2026-10-13 | `c254` | core | none: found already recorded, superseded, or an open question |
| 2026-10-13 | `c255` | core | I3202, I3226, a-partial-green-is-not-the-verdict, delivery-flow-is-measured |
| 2026-10-13 | `c256` | core | I3126, I3127, I3129, I3130, I3133, I3321, a-stranger-adopts-it-unaided, adr-0207 and 3 more |
| 2026-10-13 | `c257` | core | I3232, evidence-machinery-must-end-in-a-shipped-result |
| 2026-10-14 | `c258` | core | I3208, a-release-is-proven-end-to-end, finished-work-reaches-a-verified-release |
| 2026-10-14 | `c259` | core | I3110, I3270, adr-0188, adr-0198, the-plan-says-what-a-fix-unblocks |
| 2026-10-14 | `c260` | core | I3219, I3231, adr-0216, every-surface-follows-what-changed, liveness-differs-by-surface |
| 2026-10-14 | `c261` | core | I3300, a-provider-carries-the-contract-natively |
| 2026-10-14 | `c262` | core | I3126, I3131, I3221, I3234, finished-work-reaches-a-verified-release, integration-capacity-bounds-the-backlog |
| 2026-10-14 | `c264` | related | none: found already recorded, superseded, or an open question |
| 2026-10-15 | `c265` | core | I3233, I3236, a-plan-on-an-unlanded-branch-is-planned-again, adr-0185, do-less-work-before-doing-it-faster |
| 2026-10-15 | `c266` | related | none: found already recorded, superseded, or an open question |
| 2026-10-15 | `c267` | core | I3173, I3278, I3279, I3280, I3281, I3282, I3284, I3285 and 7 more |
| 2026-10-15 | `c268` | core | I3156, a-test-consistent-with-the-code-is-not-a-test-of-the-contract, adr-0184 |
| 2026-10-16 | `c269` | core | I3111, I3116, I3127, I3260, I3321, a-stranger-adopts-it-unaided, adr-0188, adr-0207 |
| 2026-10-16 | `c270` | core | I3247, I3249, I3254, I3257, I3259, adr-0192, operation-convergence, what-idd-is-not |
| 2026-10-16 | `c271` | core | adr-0198 |
| 2026-10-16 | `c272` | related | I3271, a-system-that-measures-itself-looks-worse-first |
| 2026-10-16 | `c273` | core | a-worktree-is-not-a-security-boundary |
| 2026-10-16 | `c274` | core | I3162, I3163, I3165, adr-0183, knowledge-acts-on-what-it-learns |
| 2026-10-16 | `c275` | core | adr-0205, mesh-connectivity, operation-convergence |
| 2026-10-16 | `c277` | related | adr-0207, adr-0208, the-product-shows-what-it-caught |
| 2026-10-16 | `c278` | core | I3166, I3298, I3299, every-sessions-usage-is-observed |
| 2026-10-16 | `c279` | related | I3187, adr-0202, an-authorised-node-connects-from-anywhere, mesh-connectivity |
| 2026-10-16 | `c280` | core | I3118, I3203, delivery-flow-is-measured |
| 2026-10-16 | `c281` | core | none: found already recorded, superseded, or an open question |
| 2026-10-16 | `c282` | core | I3187, I3188, I3190, adr-0202, adr-0203, an-authorised-node-connects-from-anywhere |
| 2026-10-16 | `c283` | related | I3208, I3276, a-release-is-proven-end-to-end |
| 2026-10-17 | `c284` | core | I3267, I3268, I3269, adr-0195, the-plan-says-what-a-fix-unblocks |
| 2026-10-17 | `c285` | core | none: found already recorded, superseded, or an open question |
| 2026-10-17 | `c286` | core | I3244, I3298, finished-work-reaches-a-verified-release, not-run-on-master-means-the-ledger-lacks-ci-runs, operation-convergence |
| 2026-10-17 | `c287` | core | I3203, I3206, adr-0198, delivery-flow-is-measured, finished-work-reaches-a-verified-release |
| 2026-10-17 | `c288` | core | I3119, I3182, I3183, I3220, I3255, adr-0200, liveness-differs-by-surface, the-runtime-supervises-itself and 1 more |
| 2026-10-17 | `c289` | related | lay-audiences-and-the-target-user |
| 2026-10-17 | `c290` | core | I3155, I3159, repository-automation-is-typed |
| 2026-10-17 | `c291` | core | I3201, a-release-is-proven-end-to-end, adr-0196, finished-work-reaches-a-verified-release |
| 2026-10-17 | `c292` | core | I3233, I3234, I3235, I3236, I3237, a-change-is-judged-in-minutes, a-derive-answers-within-a-second, a-metadata-fingerprint-is-not-content-identity and 3 more |
| 2026-10-17 | `c293` | core | I3113, I3120, I3122, a-test-fails-when-the-code-is-wrong, adr-0190 |
| 2026-10-18 | `c294` | core | I3174, I3218, I3222, I3254, a-claim-is-not-a-lock, adr-0216, every-surface-follows-what-changed, liveness-differs-by-surface |
| 2026-10-18 | `c295` | core | I3200, I3203, I3213, I3276, a-release-is-proven-end-to-end, adr-0198, delivery-flow-is-measured, integration-capacity-bounds-the-backlog |
| 2026-10-18 | `c296` | core | I3137, I3138, I3139, I3140, I3192, adr-0201, adr-0209, prismatic-lumen and 2 more |
| 2026-10-18 | `c297` | core | I3119, I3167, I3181, I3273, a-release-is-proven-end-to-end, adr-0180, adr-0198, intent-intake |

## Sources

- issue I3100, which this promotion executed
- the workspace declaration `.ai/repo/workspaces/chatgpt-majordomus.yaml`
- the records listed above, each citing its own sources
