# Claude Code bootstrap for the Majordomus `.ai/` transformation

You are executing an operator-approved repository transformation.

The transformation pack is expected at `tmp/transform/` under the target
repository. Determine the repository root with:

```bash
git rev-parse --show-toplevel
```

Read `tmp/transform/README.md` and every document it lists before mutating the
repository.

## Important precedence

The current repository's `CLAUDE.md`, `AGENTS.md`, `.majordomus/` layout,
provider body, and documentation describe the **pre-transformation system**.
They are evidence and contracts to migrate. They do not override the target
architecture in this pack where the two conflict.

All existing behavioral guarantees remain binding unless a changed contract is
explicitly specified in this pack.

## Execution mode

Work autonomously. Do not stop for cosmetic choices. Prefer the smallest
decision compatible with the target invariants. Record any non-obvious choice
in `tmp/transform/_run/DECISIONS.md`.

Use incremental commits by migration phase. Never hide failing tests, swallow
exit codes, delete unrelated user work, reset a dirty tree blindly, or claim
success without executable evidence.

The repository must remain buildable/testable without Prismatic. No direct
Prismatic dependency may be introduced.
