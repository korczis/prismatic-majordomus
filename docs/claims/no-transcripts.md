# Handovers carry durable facts, never conversation transcripts

## What it means

A handover is a short structured record of what is true — objective, current state, next action, optionally decisions, verification, risks — not a dump of what was said. Majordomus never stores, summarises or reads a conversation. The generated worker instructions say so in one of the ten rules: handovers transfer state, not transcripts.

## How it works

Structurally: the required sections force durable statements; the identity-field check refuses bodies that try to carry record metadata; and there is no command that ingests a transcript. The `no_match` / `failed` distinction and the typed outcomes exist for the same reason — facts a supervisor can act on, not prose it has to interpret.

## How to see it

```bash
majordomus handover --help
# body needs these non-empty level-one headings: the policy's handover.required_sections
grep -rn transcript share/skeleton/providers/body.md
```

## What it does not cover

Nothing stops a worker from pasting a transcript under `# Current State`. The rule is projected into its instructions and the sections make it awkward; it is not detected.

## Why it exists

Transcript-as-memory produced a ten-gigabyte notes directory and a fifteen-to-thirty-minute manual recovery runbook in the source environment. The next session needs a few hundred bytes of state, not the history of how it was found.
