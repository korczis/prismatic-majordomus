# majordomus-brief — a Claude Code mod

Keeps a Claude Code session in a Majordomus-supervised repository briefed on what surrounds it,
without anyone asking (ADR 0136). The mod is a relay: it submits `SessionBrief.report.prompt`
and appends `SessionBrief.report.reminder` verbatim. What the report contains is declared once,
in `share/brief/report.yaml` of the Majordomus distribution, and composed by `brief.session`; the
brief itself is [the session brief](https://majordomus.dev/docs/brief/).

## What it does

- **Status line.** At session start and then every minute it runs `majordomus brief --format
  json --session <this session's id>` (with `--since <the instant the session was last told>`
  once it has told it anything) and shows the answer's one line as the status line. A brief that
  cannot be read shows `majordomus brief: not answered`.
- **The report, when something changed.** When the interval has passed
  (`session.brief_interval_minutes`, carried by the brief; 20 when it cannot be read) and one of
  the brief's per-section fingerprints changed since the session was last told, it submits
  `report.prompt` as a prompt of its own. The prompt is queued and starts a turn only when the
  session is idle, so a running turn is never interrupted. The fingerprints are the brief's own,
  of its facts with every instant and age taken out, so a brief in which only the clock moved is
  not news, and the mod reads no prose to decide it. A change that reverts before the interval
  passes is never reported.
- **A prompt after the interval.** A prompt the person sends under the same two conditions
  carries `report.reminder` as a row the model reads. Either way of telling the session counts
  as telling it.

The prompt's first words are the contract's marker, which the brief carries as
`report.marker`; the mod never briefs a prompt that opens with it, nor one that opens with the
marker an earlier release of the mod used (`LEGACY_MARKER` in `hooks/register.ts`). An
executable older than the report contract carries no report: the mod then submits that
marker, the brief's lines, and the sentence that the executable predates `brief-report/v1`.
Claude Code adds a framing sentence of its own after any prompt a plugin submits; that sentence
is Claude Code's, and this mod neither writes it nor can remove it.

It never builds the executable (`MAJORDOMUS_NO_BUILD=1`), it passes the provider's background
tasks to the brief only where the engine exposes them (this engine does not, and the brief says
`provider background: not exposed`), and in a repository without `.ai/manifest.yaml` it does
nothing at all.

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
against the engine: the submitted prompt is `report.prompt` byte for byte and the reminder
`report.reminder`; the guard knows both markers; an older executable is said to predate the
contract; only a changed section fingerprint is a change; no report over twelve hours in which
only the clock moved; one report per change, at most once per interval; the reminder on a late
prompt only when a fact changed; an unreadable brief; and silence outside a supervised
repository. Case 4250 runs it, with mutations of the comparison and of the relay that must fail
it, where `claude` is installed. The test is not installed.
