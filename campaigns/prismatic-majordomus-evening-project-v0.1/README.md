# Prismatic Majordomus

A lightweight supervisory control layer for AI-assisted work.

The canonical distributable artifact is the self-contained `.majordomo/` directory.

## Quick start

```bash
cp -R .majordomo /path/to/your/project/
```

Then add to `AGENTS.md`, `CLAUDE.md`, or equivalent:

```markdown
## Majordomus

Follow `.majordomo/README.md` for all non-trivial AI-assisted work.
```

Optionally validate with:

```bash
./scripts/majordomus-doctor
```

## v0.1

Majordomus provides canonical AI execution policy, context hygiene, execution profiles, durable state, verification, handovers, completion criteria, and deterministic structural checks.

It deliberately does not include model invocation, a server, database, dashboard, daemon, telemetry backend, or autonomous agent runtime.
