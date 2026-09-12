+++
title = "The start briefing names the candidates awaiting review on this branch, count and ids, bounded and within the briefing budget, and prints their absence rather than omitting it"
description = "The briefing a new episode is handed (ADR 0017) gains one bounded section. It names the candidate records whose episode was on this branch, the count on one line and the ids beneath it, bounded the way the open-questions block is and within session.briefing_budget_lines. A candidate whose episode this checkout cannot place is named on its own line as unattributed rather than hidden. When there is nothing, the section says so in one sentence, because absence printed is an answer and absence omitted is a gap."
weight = 190
[extra]
claim_id = "briefing-names-candidates"
status = "guaranteed"
source = "docs/claims/briefing-names-candidates.md"
+++
{% raw %}

## What it means

The briefing a new episode is handed (ADR 0017) gains one bounded section. It names the candidate records whose episode was on this branch, the count on one line and the ids beneath it, bounded the way the open-questions block is and within `session.briefing_budget_lines`. A candidate whose episode this checkout cannot place is named on its own line as unattributed rather than hidden. When there is nothing, the section says so in one sentence, because absence printed is an answer and absence omitted is a gap.

## How it works

`mj_derive_briefing_body` in `lib/derive.sh` calls `mj_knowledge_briefing_lines` with the current branch after the open-questions block and before the handover block. A candidate's branch is the branch of the episode its `derived_from` names, read from the tracked session record under `.ai/repo/sessions/` or, while that record is not yet written, from the ledger's `session.started` line; a candidate whose episode neither source knows is listed under a distinct line naming how many are unattributed. The ids are bounded through `mj_derive_bounded`, which prints the first few and a count of the rest. The same section reaches `majordomus context`, and `majordomus knowledge candidates --json` carries `branch: null` for an unattributed candidate.

## How to see it

```bash
printf '{"session_id":"e1","source":"startup"}' | .claude/hooks/majordomus-session-start
                                                        # No knowledge candidates await review on this branch. ...
majordomus decision add "Something decided" --why "because"
printf '{"session_id":"e1","reason":"other"}' | .claude/hooks/majordomus-session-end
printf '{"session_id":"e2","source":"startup"}' | .claude/hooks/majordomus-session-start
                                                        # Knowledge candidates awaiting review on this branch: 1
                                                        # - e1-<digest>  Something decided
majordomus context | grep -A3 'Knowledge candidates'
```

## What it does not cover

It does not print the records' bodies; a briefing carries references and never a conversation, and a candidate's title is the assertion, not an excerpt. It does not list candidates of other branches; `majordomus knowledge candidates` lists every branch. It is subject to the budget like every other section, and is dropped after history and files when the budget is exceeded.

## Why it exists

A record nothing loads is a record nobody reads. The start briefing is the one moment at which a continuation record reaches a worker without the worker remembering to ask, and a review queue that is visible only to a person who runs a command is a queue that grows. `.ai/repo/adrs/0058-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md` fixes the section, its position and its bound.
{% endraw %}
