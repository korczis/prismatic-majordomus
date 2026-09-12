# A repository in which episodes close and no knowledge derivation follows within the stale threshold is reported as a stopped writer

## What it means

`doctor` and `watch` compare two facts nothing else owns together: that episodes are closing, and that derivations are following them. When the newest `session.closed` line names an episode no `knowledge.derived` line names, and that close is older than `session.freshness.stale_minutes`, the finding is `FAIL knowledge` naming the episode, with the read-only reproduce and the remedy after `fix:`. When `session.knowledge_on_end` is on and the lifecycle source never calls the deriver, the finding names the source file. A derivation for that episode clears the finding.

## How it works

`mj_validate_knowledge_lifecycle` in `lib/knowledge.sh` reads the ledger for the newest `session.closed` line, takes its session stamp, and looks for a `knowledge.derived` line whose `episode` field names it; the age of the close is judged against the policy's stale threshold, read and never restated. The judgement is gated the way ADR 0052 gated its own: a checkout in whose ledger no `knowledge.derived` line exists at all is reported as a skip, so the day the deriver arrives no checkout turns red, and the first derivation ends the skip. A repository with no closed episode passes; a policy with the switch off is a skip. The wiring half greps the lifecycle source for the call to the deriver, the way `mj_validate_doctrine_wiring` reads the source. Under `watch` the finding is drift and exits 11. The Rust reader's `knowledge_base.status` makes the freshness half of the same judgement from the same ledger and the same numbers; the wiring half is the shell validator's alone.

## How to see it

```bash
majordomus history --event session.closed               # the newest close and its episode
majordomus history --event knowledge.derived            # which episodes were derived
majordomus doctor | grep -E 'knowledge'                 # OK, SKIP, or FAIL naming the episode
majordomus knowledge derive --episode <id>              # the remedy the finding names
majordomus doctor | grep -E 'knowledge'                 # clears
bin/majordomus-cli knowledge status --format json       # the same freshness judgement from the Rust reader
```

## What it does not cover

It does not derive. A diagnostic that silently mutates state is the watchdog this design refuses; the remedy is a command a person runs. It does not judge a checkout on which the deriver has never run, and it cannot see a close that happened on another machine: the ledger is local, and so is the judgement.

## Why it exists

`majordomus.lifecycle-observed` was written after a week in which every health check passed while no checkpoint and no handover were written; reachability was the question every check asked and running was the question none did. The same shape applies to the knowledge writer, with one more way to fail: a derivation is a step inside the close, and a close path that skips it produces closed episodes and no knowledge, forever, with every record well-formed. `.ai/repo/adrs/0058-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md` extends ADR 0052's invariant to the fourth thing that survives an episode.
