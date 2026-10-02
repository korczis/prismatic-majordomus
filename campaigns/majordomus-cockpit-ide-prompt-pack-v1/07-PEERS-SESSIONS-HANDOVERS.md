# Prompt 07 — Peers, Collaboration, Session Contexts, Handovers

## Mission

Make peer cooperation visible and operational from Cockpit and ensure it is automatically used by supported clients.

The repository already requires workers to announce scope and use a shared server. Build on that.

## Peer board

Cockpit must show for each peer:
- stable/ephemeral peer identity according to current model;
- client/provider type;
- connection state;
- announced intent;
- claimed paths;
- claimed identifiers;
- active task/mandate;
- branch/worktree;
- last activity if canonically known;
- conflicts/overlap diagnostics;
- reconnect state.

Do not expose secrets or model credentials.

## Cooperation

Provide canonical actions, where backend semantics exist, for:
- announce/update intent;
- inspect other peers;
- check overlaps;
- request/record handover;
- inspect branch/worktree collisions;
- fan out work safely;
- attach execution/task to a peer.

A claim is not a lock unless repository rules define it as one.

## Session context

Audit the current session context architecture for:
- canonical storage;
- discovery;
- automatic loading;
- update cadence;
- detail/completeness;
- relation to task/branch/worktree;
- relationship to knowledge and handover;
- stale detection.

Cockpit should display the current effective session context with provenance and freshness.

Do not create `cockpit-session.json`.

## Handover

A handover should be a typed canonical artifact containing, as applicable:
- task/intent;
- current state;
- decisions;
- changed files;
- remaining work;
- tests run;
- failing tests;
- branch/worktree;
- commits;
- issues/milestones;
- risks;
- next actions;
- evidence links.

Derive fields automatically where possible.

## Automatic attachment

Ensure Claude Code, Codex, Gemini and supported clients:
- start/reuse the shared MCP server from canonical generated config;
- register as peers;
- announce or are prompted/enforced to announce before mutation;
- refresh announcement after reconnect where required;
- consume session context.

The rule belongs in `.ai` and generated bootstrap, not hand-edited `CLAUDE.md`.

## Acceptance

Simulate at least two clients/workers:
- both become visible;
- overlap is detected;
- claims update;
- reconnect semantics work;
- handover is visible through CLI/API/MCP/Cockpit;
- no client-specific duplicate peer model exists.
