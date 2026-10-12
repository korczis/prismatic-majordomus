# Snapshot notes used to construct this pack

Source: `prismatic-majordomus-context-20260911.zip` supplied in the Majordomus project context.

These notes are **not authoritative over current master**. They explain why the prompts emphasize convergence rather than rewrite.

Observed in the snapshot:

- `AGENTS.md` required `.ai/` discovery, `majordomus context`, canonical worktree topology, peer announcements and generated capability projections.
- `docs/PLANNING.md` defined a milestone as an **executable specification of an outcome** and an issue as a **bounded execution contract**.
- Planning explicitly rejected authored `status`; issue/milestone state was derived from lifecycle facts, dependencies and evidence.
- Milestone completion required its own evidence after issues were done.
- Issue dependency graphs were validated and used to derive execution waves.
- `docs/ROADMAP.md` treated roadmap as a projection of milestone graph state, not a hand-written list.
- Sessions, session-context, handovers and blocker continuity had behavioral tests.
- Provider/session separation and provider hooks existed.
- Evidence, obligations, doctrine wiring, canonical architecture, strict derivation, cross-surface projections, OpenAPI, MCP and public-site/deployment checks already had dedicated test areas.

Therefore the intended implementation strategy is:

```text
existing planning + continuity + evidence + projection primitives
                    ↓
            canonical Intent semantics
                    ↓
              intent compiler
                    ↓
      intent-aware execution continuity
                    ↓
      re-evaluable satisfaction/evidence
                    ↓
        same canonical surfaces + gates
```

Do not regress newer live architecture to match this snapshot.
