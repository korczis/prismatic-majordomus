# Majordomus two-folder consolidation — Claude Code execution instructions

This archive contains a sequential implementation program for Claude Code/Fable-class models with a large context window. It is intentionally split into phases so each run can spend its context on one architectural problem and leave a handover for the next run.

## Target

After `majordomus init` in a target repository, normal Majordomus-owned repository state should be concentrated into:

```text
.ai/
.majordomus/
```

with only minimal, stable, ownership-tracked compatibility bridges outside those directories when a tool actually requires them, most notably a tiny `AGENTS.md` block and optionally `.envrc`.

## Where to unpack

Recommended:

```bash
mkdir -p ~/dev/prismatic-majordomus/tmp/majordomus-two-folder-consolidation
cd ~/dev/prismatic-majordomus/tmp/majordomus-two-folder-consolidation
unzip /path/to/majordomus-two-folder-consolidation-prompts.zip
```

The prompts are **instructions for an agent working from the actual repository root**, not a replacement project. Keep the archive under `tmp/` or another ignored location so the prompt pack itself does not pollute the source tree unless you deliberately want to version it.

## Before first run

From the actual repository root:

```bash
cd ~/dev/prismatic-majordomus
git status
```

Make sure the current worktree/branch is the one you intend to use. Follow the repository's current issue/milestone/worktree rules. If Majordomus already enforces sibling `-wt` worktrees, use that mechanism rather than improvising another worktree directory.

Do not manually pre-create `.ai`/`.majordomus` files just to help the model. The whole point is to force discovery of the existing implementation and evolve it coherently.

## Execution mode A — recommended: one fresh Claude Code session per phase

For each phase:

1. Start Claude Code from `~/dev/prismatic-majordomus`.
2. Give it `00_MASTER_ORCHESTRATOR.md` for global context on the first phase, then the relevant numbered prompt.
3. Tell Claude to read repository-local `AGENTS.md`, `CLAUDE.md`, `.ai/**` and `.majordomus/**` instructions before editing.
4. Let the phase complete fully, including tests and the repository-native handover/session-context update.
5. Review the diff and test results.
6. Start a fresh session for the next prompt, allowing the new session to consume the persisted handover plus actual git state.

The fresh-session boundary is intentional. It tests the session-context/handover architecture while preventing a single model conversation from accumulating irrelevant implementation debris.

## Execution mode B — one 1M-context session

You may load `00_MASTER_ORCHESTRATOR.md` and ask Claude to execute phases sequentially, but require a checkpoint after every numbered phase:

```text
Do not continue to the next numbered phase until you have:
- completed the current acceptance criteria,
- run relevant gates,
- updated the canonical handover/session context,
- summarized git status and unresolved risks.
```

This mode is faster operationally but easier for a model to blur boundaries and “finish” later phases with hand-waving. Humans invented project phases for a reason, mostly after learning what happens without them.

## Prompt order

```text
01 forensics + architecture
02 filesystem contract + schemas + README hierarchy
03 unified repository model + discovery
04 init/sync reconciler + ownership + uninstall
05 root/env/provider/completion bridges
06 legacy migration + registry/root cleanup
07 enforcement + tests + performance
08 CLI/API/OpenAPI/MCP/Cockpit/docs projections
09 adversarial e2e audit + release readiness
```

Do not execute Phase 06 before the reconciler exists. Moving files first and designing ownership second is how “cleanup” becomes data loss with a nicer commit message.

## How to feed a prompt

If your Claude Code version supports loading a file directly, use that facility. Otherwise from the repo root:

```bash
cat /path/to/prompts/01_forensics_and_target_architecture.md
```

and paste/feed the full contents into the session.

Do not strip the shared execution contract from the prompts. Repetition is deliberate here: each fresh model session must receive the same invariants without relying on memory.

## What not to do

- Do not tell Claude exact crate/file names unless it has discovered them. The prompt deliberately uses conceptual names where repo reality is unknown.
- Do not permit a second registry “temporarily” unless there is a migration with a deletion date/gate.
- Do not accept generated documentation edited by hand when a generator exists.
- Do not accept `.envrc` containing real repository logic.
- Do not accept provider-specific semantic copies of rules/doctrines as canonical state.
- Do not accept `rm -rf`-style uninstall semantics for files that may contain user-authored knowledge.
- Do not accept “idempotent by inspection”; demand a test.
- Do not accept a UI that independently scans directories instead of consuming the canonical model/API.
- Do not allow source-repo developer tooling to be confused with files copied by `majordomus init` into target repos.

## Review checkpoints

After phases 1–3, review architecture before broad migration. Specifically verify:

```text
.ai             = portable semantic truth
.majordomus     = control-plane/runtime/ownership
root artifacts  = tiny adapters only
model           = typed + discoverable
registries      = derived, not manually enumerated
```

After phases 4–6, run fixture diffs for fresh init, repeated init, legacy migration, and uninstall.

After phases 7–9, insist on full CI-equivalent gates, docs build, schema drift checks, and an e2e demonstration.

## Useful final proof

The best final evidence is not a screenshot. It is a reproducible shell/integration test that captures trees/diffs around this lifecycle:

```text
clean fixture
→ majordomus init
→ verify minimal root diff
→ majordomus init again
→ verify zero diff
→ add valid `.ai` artifact
→ verify discovery on all applicable surfaces
→ sync
→ uninstall default
→ verify external bridges removed and authored `.ai` preserved
```

## References

`references/Dynamic-CLI-banner.txt` contains the prior environment/banner architecture discussion. Treat it as a design reference, not as proof that the repository already implements every mentioned type/command/path.
