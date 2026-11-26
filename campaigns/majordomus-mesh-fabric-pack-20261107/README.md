# Majordomus Mesh Fabric prompt pack

**Start:** feed `MASTER-PROMPT.md` to Claude Code in the current repository, then execute `PACK.json` phases one by one. Each phase reads `EXECUTION-CONTRACT.md`. Do not blindly execute on historical archive.

**Historical evidence (2026-09-11 archive):** `AGENTS.md` mandates `.ai` context resolution, canonical capability declarations and generated surfaces. `docs/claims/mcp-peers.md` describes a connection-scoped in-memory peer board in `src/peers.rs`, `peers.list`/`peers.announce`, without persistence or direct routed chat. These observations are **not** a claim about 2026-11-07 HEAD.

**Core success:** two real supported agent runtimes exchange request/reply without using board polling as transport, under enforced policy, with durable evidence.

**Order:** 01 discovery, 02 typed protocol, 03 local direct messaging, 04 durability, 05 agent adapters, 06 remote QUIC, 07 routing/telemetry, 08 derived surfaces, 09 rollout.

**Suggested execution:** `cat MASTER-PROMPT.md` in Claude Code, then have it read `PACK.json` and phase files. Do not simply paste all nine into independent agents editing same files: they share canonical owners and must integrate in dependency order.
