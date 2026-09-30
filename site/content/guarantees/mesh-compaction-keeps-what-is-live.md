+++
title = "Compacting the cooperation journal changes nothing live — a handover nobody has taken keeps its stream for a bounded retention, who took a handover and who answered a review stay with them, and a compacted runtime that beats again is fetched again whole"
description = "Every server restart is a new stream of the journal, so runtimes compact away streams that"
weight = 199
[extra]
claim_id = "mesh-compaction-keeps-what-is-live"
status = "guaranteed"
source = "docs/claims/mesh-compaction-keeps-what-is-live.md"
+++
{% raw %}

## What it means

Every server restart is a new stream of the journal, so runtimes compact away streams that
have been dead for long. Compaction takes away the records of those streams and nothing
anyone still relies on: a claim that holds and a session that is open are never touched; a
handover nobody has taken keeps its stream for a day past the retention, and no longer, so a
machine whose runs each handed over something cannot fill its quota with the dead; a handover
still offered keeps everyone who took it, and a review keeps every answer, even when the
taker's or the answerer's runtime has stopped. And a runtime compacted away while it slept —
a laptop past the expiry and the fifteen-minute retention — is heard again when it wakes:
its peers fetch its stream again from the first event, so a claim it took before it slept
holds again there and refuses a conflicting one.

## How it works

`Journal::compact` (`apps/majordomus-cli/src/mesh/journal.rs`) considers only streams silent
for longer than the expiry plus the retention. Of those, it keeps a stream that offers a
handover the fold lists with no taker, while its silence is within `HANDOVER_RETENTION` past
the retention, and — to a fixed point — a stream holding the taking of a handover or the
answer to a review whose own stream is kept. Who took and who answered is read by
`mesh::state::fold`, the reading every surface shows. What goes leaves a tombstone: the
sequence it reached, advertised so it is not sent again, and the beat it was last heard at.
A late event of a compacted stream is absorbed into the tombstone. `Journal::merge_marks`
lifts a tombstone on a verified, trusted beat that is fresh and above the tombstone's beat;
the stream's mark falls to nothing held and the peers send it again from its first event.
The server's supervisor compacts every sixty heartbeats with `PEER_RETENTION` and
`HANDOVER_RETENTION` (`apps/majordomus-cli/src/mesh/cooperation.rs`).

## How to see it

```
test/run.sh 493_mesh_compaction_keeps_what_is_live
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test mesh_compaction
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib mesh::journal
```

The case runs `tests/mesh_compaction.rs`: two runtimes linked through the real signed
protocol over an in-process transport, one compacted while it sleeps and heard again whole
when it wakes, a taker and a reviewer that stop while the publisher runs, and two stopped
publishers of which only the one whose handover nobody took is kept. The journal's unit
tests reproduce each defect PR 588 recorded, and its property test,
`compacting_a_crowded_node_changes_nothing_live`, holds over random histories that no live
claim, open session, handover taker or review answer changes.

## What it does not cover

No black-box case reaches compaction in bounded time — a server compacts every sixty
heartbeats and keeps a dead stream for fifteen minutes, and only the heartbeat and the expiry
are declarable — so the proof is the library's own runtimes rather than two servers. Two
runtimes can print different digests while each compacts on its own schedule: one has
forgotten a dead stream's ended records that the other still lists. What is live agrees, and
marks agree. A review request of a dead runtime keeps nothing on its own and goes with its
stream. Tombstones are not persisted: a restarted runtime may be sent a compacted stream
again and drops it again later.

## Why it exists

Compaction is what keeps a machine that restarts its servers from filling its quota, and a
bound that forgets what is live is worse than no bound: a handover that looks untaken is
taken twice, a review that opens again is answered twice, and a runtime that wakes up unheard
holds claims its peers cannot see. PR 588 recorded all three; this claim is their closure.
{% endraw %}
