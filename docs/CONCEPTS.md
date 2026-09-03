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
| **scope** | the normalised set of repository paths a task may touch | `scope:` in the task record |
| **claim** | the same scope, seen from another worktree; overlap is reported, never blocked | reported by `start` and `check --overlap` |
| **checkpoint** | an update to `checkpoint_at`; staleness is measured from it | `check --checkpoint`, `handover`, `finish` |
| **handover** | an append-only continuation record with computed front matter and required sections | `.majordomus/state/handovers/` |
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
