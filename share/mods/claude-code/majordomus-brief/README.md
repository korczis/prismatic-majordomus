# majordomus-brief — a Claude Code mod

Keeps a Claude Code session in a Majordomus-supervised repository briefed on what surrounds it,
without anyone asking (ADR 0136). The surface it reads is `majordomus brief`; see
[the session brief](https://majordomus.dev/docs/brief/).

## What it does

- **Status line.** At session start and then every minute it runs `majordomus brief --format
  json --session <this session's id>` and shows the answer's one line as the status line. A
  brief that cannot be read shows `majordomus brief: not answered`.
- **Self-report when something changed.** When the interval has passed
  (`session.brief_interval_minutes`, carried by the brief; 20 when it cannot be read) and a fact
  in the brief changed since the session was last told, it submits a prompt of its own. The
  prompt is queued and starts a turn only when the session is idle, so a running turn is never
  interrupted. A brief in which only the clock moved is not news: the `at`, every RFC 3339
  instant and every span of time ("4 min ago", "38 h ago", "21 h 44 min", "(16s)", "36m") are
  taken out before the comparison, and so is why a leaseholder did not answer. What the session
  was last told starts as the first brief the mod reads, which is what the start event's
  briefing has just said. A change that reverts before the interval passes is never reported.
- **A prompt after the interval.** A prompt the person sends under the same two conditions
  carries the brief with it, dated and opening with what changed, as a row the model reads
  and the person does not see typed. Either way of telling the session counts as telling it.

### What the person reads

The submitted prompt opens with what changed in one plain sentence, then the brief, then one
line for the agent:

```text
Majordomus brief: since the last report, main moved 1a2b3c4 → 9f8e7d6 and a peer attached.

<the brief, in a code block>

To the agent: tell the person in two to four lines what this changes for the work in hand, reading live state before acting on it, then carry on or stop.
```

Its first words (`SELF_REPORT` in `hooks/register.ts`) are how the mod knows its own prompt and
never briefs it. Claude Code adds a framing sentence of its own after any prompt a plugin
submits ("This is how Claude Code surfaces a prompt a plugin submits between turns ...").
That sentence is Claude Code's; this mod neither writes it nor can remove it.

It never builds the executable (`MAJORDOMUS_NO_BUILD=1`), and in a repository without
`.ai/manifest.yaml` it does nothing at all.

## How it is installed

`majordomus capture install` writes this folder to `.claude/skills/majordomus-brief` when the
policy declares `session.brief_on_interval: true`, and removes a copy it wrote when the policy
says anything else; a folder it did not write is left alone. Claude Code loads a mod from
`.claude/skills/<name>` by itself, so no settings file is changed. The rules are those of the
handover mod beside it (ADR 0123): the hooks module carries the `Written by` marker, a copy that
drifted is rewritten, and the provider's type declarations under `.claude-plugin/types/` are
neither compared nor tracked.

## Its test

`claude plugin test share/mods/claude-code/majordomus-brief` runs `hooks/register.test.ts`
against the engine: the status line and its refresh; the normalisation of every relative age and
instant `majordomus brief` emits; no self-report over twelve hours in which only the clock moved;
one self-report per change, at most once per interval, naming the change; the brief on a late
prompt only when a fact changed; an unreadable brief; and silence outside a supervised
repository. A mutation of the comparison fails it. Case 4250 runs it, and that mutation,
where `claude` is installed. The test is not installed.
