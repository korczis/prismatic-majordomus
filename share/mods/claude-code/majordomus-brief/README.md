# majordomus-brief — a Claude Code mod

Keeps a Claude Code session in a Majordomus-supervised repository briefed on what surrounds it,
without anyone asking (ADR 0136). The surface it reads is `majordomus brief`; see
[`docs/BRIEF.md`](../../../../docs/BRIEF.md).

## What it does

- **Status line.** At session start and then every minute it runs `majordomus brief --format
  json --session <this session's id>` and shows the answer's one line as the status line. A
  brief that cannot be read shows `majordomus brief: not answered`.
- **Self-report on an interval.** Once per interval (`session.brief_interval_minutes`, carried
  by the brief; 20 when it cannot be read) it submits a prompt of its own that carries the full
  brief and asks for a report of two to four lines. The prompt is queued and starts a turn only
  when the session is idle, so a running turn is never interrupted. The first one is owed one
  interval after the start, because the start event's briefing has just told the session the
  same.
- **A prompt after the interval.** A prompt the person sends once the interval has passed
  carries the last brief read with it, dated, as a row the model reads and the person does not
  see typed.

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
against the engine: the status line and its refresh, one self-report per interval, the brief on
a late prompt, an unreadable brief, and silence outside a supervised repository. The test is not
installed.
