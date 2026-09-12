# A policy switch that is off derives nothing and says so; a derivation that fails inside a provider hook is recorded as a failed event and the hook still exits 0

## What it means

Two things can stop a derivation, and neither is silent. When `session.knowledge_on_end` or `session.knowledge_on_compact` is `false`, the adapter derives nothing and says so on stderr; the ledger shows a closed episode with no derivation and the stopped-writer check treats the switch as deliberate. When the deriver fails — the directory is unwritable, the policy does not parse — the adapter writes a `provider.event.failed` line naming the reason, closes the episode as it would have anyway, and returns success to the provider.

## How it works

The end adapter in `lib/capture.sh` reads the close's status and stderr: `mj_session_close` runs the deriver on its way to `session.closed`, and a failure inside it surfaces as the last line of the close's output, which the adapter passes to `mj_capture_session_failed` with the event name `end`. The compaction adapter calls the deriver directly and reports the same way with the event name `compact`. Both switches are read from the policy at the moment of the event with `mj_pol`, absent meaning on, and `session.knowledge_on_compact` is independent of `checkpoint_on_compact`: a compaction that skips its checkpoint still derives. No hook exits non-zero, for the reason `capture prompt` never does.

## How to see it

```bash
sed -i.bak 's/knowledge_on_end: true/knowledge_on_end: false/' .ai/repo/policy.yaml
printf '{"session_id":"e1","reason":"other"}' | .claude/hooks/majordomus-session-end
                                                        # stderr: session.knowledge_on_end is false, so no knowledge is derived
mv .ai/repo/policy.yaml.bak .ai/repo/policy.yaml
chmod 0500 .ai/repo/knowledge/candidates
printf '{"session_id":"e2","reason":"other"}' | .claude/hooks/majordomus-session-end; echo "exit $?"   # exit 0
majordomus history --event provider.event.failed        # event end, reason names the knowledge derivation
chmod 0755 .ai/repo/knowledge/candidates
```

## What it does not cover

A failure inside `majordomus knowledge derive` typed by a person is reported by its exit code and its last line, not by a `provider.event.failed` line: that event is the receipt of a provider hook, and a person at a terminal has the exit code. The switch does not disable `majordomus knowledge derive` by hand.

## Why it exists

An adapter that receives an event and declines to act on it writes a line to stderr, which nobody keeps, and returns 0, which is what a provider hook must do. Afterwards, an event that never fired and an event that fired and did nothing are the same observation. ADR 0052 introduced the failed-event receipt for the checkpoint and the handover; `.ai/repo/adrs/0058-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md` applies it to the derivation, so a derivation that stops is a typed line and not a silence.
