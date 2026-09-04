# Concepts

The vocabulary, in ordinary engineering language. Everything Majordomus prints, and
every file it writes, uses these words and no others.

| Term | Meaning | Where it lives |
|---|---|---|
| **policy** | the one canonical, provider-neutral operating policy | `.majordomus/policy.yaml` |
| **profile** | a named bundle fixing capability class, effort, verbosity, presentation, context toggles, verification, checkpoint interval, and output contract for a task class | `.majordomus/profiles/<name>.yaml` |
| **projection** | a provider-specific instruction file generated from the policy, headed with the command that regenerates it, fingerprinted | `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, or any target the policy names |
| **fingerprint** | the hash of a projection at generation time; a differing hash means a hand edit | `.majordomus/generated/fingerprints.yaml` |
| **task** | the one active unit of work in a checkout | `.majordomus/state/current.yaml` |
| **session** | one execution episode of one worker: it opens, it may touch several tasks, and it closes into an immutable record that references what the episode produced and copies none of it | `.majordomus/state/session-current.yaml` while open, `.majordomus/state/sessions/` once closed |
| **envelope** | what a closed session record is: identity, a temporal boundary, and references — never a copy of the records it names | `.majordomus/state/sessions/<file>.md` |
| **scope** | the normalised set of repository paths a task may touch | `scope:` in the task record |
| **claim** | the same scope, seen from another worktree; overlap is reported, never blocked | reported by `start` and `check --overlap` |
| **checkpoint** | a compact progress record inside an active task, capped by policy so that it stays quotable rather than becoming a report; also updates `checkpoint_at`, from which staleness is measured | `.majordomus/state/checkpoints/`, `checkpoint`, `check --checkpoint` |
| **handover** | an append-only continuation record with computed front matter and required sections, written when a worker stops | `.majordomus/state/handovers/` |
| **decision** | what was decided, why, what was rejected, and which task decided it; superseded by a later entry naming it, never edited | `.majordomus/state/decisions.md` |
| **open question** | something unresolved, recorded as state rather than prose; any entry naming the active task refuses `finish --outcome completed` | `.majordomus/state/open-questions.md` |
| **doctrine** | a rule the tool enforces, declared once with the validator that decides it, the commands that run it, and the test that proves it; `blocking` stops a command, `advisory` reports and does not | `share/doctrines.yaml`, read by `doctrine`, `check --rule`, and every enforcing command |
| **validator** | the function that decides one doctrine; it reports findings and never decides their level, because the doctrine's class does that | `mj_validate_<name>` in `lib/` |
| **history event** | one line of the ledger: what happened, when, for which task, at which head | `.majordomus/state/ledger.jsonl`, read by `history` |
| **context** | the assembled briefing for whoever works next: durable state in authority order, within a line budget, with every exclusion named | printed by `context`; never stored |
| **authority order** | git, then task and profile, then blockers, then authored records, then history — the order sections appear in and the reverse of the order they are dropped in | `context` |
| **prompt asset** | a small, versioned, provider-neutral framing in the repository, rendered against a closed set of state tokens | `.majordomus/prompts/<name>.md`, `prompt` |
| **record resolution** | choosing the right prior record: same worktree and branch, else same branch, else nothing — never repository-wide | `handover --resolve`, `checkpoint --show`, `context` |
| **outcome** | one of `active`, `completed`, `partial`, `blocked`, `no_match`, `failed`, `handed_over`; the only field a command changes after `start` | `outcome:` in the task record |
| **finish contract** | the checklist `finish` evaluates before accepting `completed`; every line printed pass or fail | `verification.finish_requires` in the policy |
| **ledger** | append-only events written only by Majordomus; retention-capped | `.majordomus/state/ledger.jsonl` |
| **wired** | an enforcement whose executable resolves and is invoked, without a swallowed exit code, by the hook or CI file the policy names | checked by `doctor` |
| **drift** | a deterministic disagreement between policy, projection, state, and git | reported by `watch` |
| **divergence label** | `exact`, `advanced`, `diverged`, `different_context`: how a recorded head relates to the current one | computed at read time from git |
| **capability class** | `fast`, `standard`, `strong`, `strongest`; never a vendor model name | `capability:` in a profile |
| **effort** | `low`, `medium`, `high`, `xhigh`, `max`; reasoning depth, independent of everything else | `effort:` in a profile |
| **verbosity** | `terse`, `concise`, `detailed`; how much the worker says, independent of how hard it thinks | `verbosity:` in a profile |
| **presentation** | `machine`, `engineering`, `summary`; the final layer, chosen by the profile | `presentation:` in a profile |

## The two outcomes people confuse

`no_match` means the work was done and the thing sought does not exist.
`failed` means the work could not be done.

"We searched and found nothing" and "the source could not be searched" look alike in
a transcript. A supervisor that cannot tell them apart cannot decide whether to retry,
escalate, or accept. The typed field decides; prose never does.

## What is deliberately not a concept

No agent, persona, role, tier, registry, or catalogue. A supervisory
tool that adds nouns becomes the thing it supervises. The only actor Majordomus knows is
`owner`, a free-form string on the task record.

## Task, session, handover: three objects, three questions

They overlap in time and are easy to collapse into one, and each collapse loses something
specific.

A **task** answers *what is being worked on, under what constraints, within which paths*.
It is scoped, it has a profile, and it outlives the worker: a task begun on Tuesday can be
continued on Thursday by somebody else.

A **session** answers *what one worker did between sitting down and stopping*. It is not
scoped and it constrains nothing. It may span several tasks, and one task may be spanned
by several sessions. It is the only object that can answer which work happened together
and in what order.

A **handover** answers *what the next person needs in order to continue this work*. It is
authored, deliberate, rare, and it has required sections.

Collapse the session into the task and you lose the episode: two decisions recorded an
hour apart under different tasks look unrelated, because nothing records that one worker
made both in one sitting. Collapse the session into the handover and you get a
transcript — a narrative of an episode rather than a set of pointers to what it produced,
which is the failure mode this whole design exists to avoid. Collapse the task into the
session and scope becomes meaningless, because an episode does not have one.

## The two records people confuse

`checkpoint` and `handover` look alike — both are append-only Markdown with computed
front matter — and treating them as one thing costs you both.

A checkpoint is written often and read whole. It says what was true a few minutes ago and
what comes next, in a few lines, so that the next briefing can quote it verbatim. The
policy caps its length, and a body over the cap is refused rather than truncated.

A handover is written rarely and read deliberately. It has required sections, it is
refused if any is empty, and it is the package another worker resumes from.

The cap is what keeps them distinct. Without it, checkpoints grow into reports, reports
are too long to include in a briefing, and the briefing degrades to a pointer — which is
where the tool started before either record existed.

## Absence is a concept

`No relevant handover.` is an answer, not a failure. Resolution considers the same
worktree and branch, then the same branch, then stops. A record from an unrelated branch
is never offered, because a worker cannot tell that borrowed context is wrong until it has
acted on it. Absence is better than incorrect memory.

## What is deliberately not a concept, still

No agent, persona, role, tier, registry, or catalogue of prompts. `prompts/` holds a few
reusable framings, not a library; nothing ranks them, and nothing loads one unless asked.
No summariser, no embedding, no vector store, no transcript. Majordomus stores, validates,
resolves, projects and verifies. It never calls a model.
