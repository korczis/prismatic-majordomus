# Majordomus Observatory — Infinite Living Screens

**Unified implementation prompt pack** | 2026-10-14 | Czech design intent / English LLM execution

## Goal
Turn `/ui/screensaver` into an unattended, cinema-quality, real-time, data-derived, explainable generative-art exhibit for a large hall LCD, not a decorative dummy dashboard.

## Start
1. Place/unpack this pack where Claude Code can read it inside the active `prismatic-majordomus` checkout (or pass its absolute path).
2. Paste **`prompts/00-MASTER-EXECUTION.md`** into Claude Code.
3. That prompt requires the agent to read *all* files in this pack, inspect current repository reality, implement incrementally, test, document, and deliver evidence.
4. Run the existing repository governance checks. Never mistake this prompt pack for completed implementation.

## Contents
- `prompts/00-MASTER-EXECUTION.md`: mission, autonomous execution policy, precedence, done criteria.
- `prompts/01-REPOSITORY-DISCOVERY.md`: mandatory source-of-truth and capability audit.
- `prompts/02-IMPLEMENTATION-PLAYBOOK.md`: typed models, realtime transport, scene director, renderer, kiosk and staged rollout.
- `contracts/OBSERVABILITY-CONTRACT.md`: canonical projection/event/provenance/privacy and connection semantics.
- `contracts/DIRECTOR-AND-SCENES.md`: eight scenes, scene lifecycle, fairness, interruptions, failure modes.
- `design/ART-DIRECTION-AND-SKETCHES.md`: aesthetic, UI wireframes, p5/WebGL guidance, interaction and animation mapping.
- `verification/TEST-MATRIX-AND-RELEASE.md`: comprehensive checks, soak tests, observability and launch gates.
- `reference/CONVERSATION-CONSOLIDATION.md`: detailed preservation of user goals and earlier sketches, plus fact/proposal distinction.
- `reference/ILLUSTRATIVE-UI.html`: standalone *synthetic* animation sketch to communicate the proposed aesthetic, never live data.

## Non-negotiable
**Observed facts -> canonical typed projection -> explainable artistic semantics -> scene director -> renderer.**
Unknown != zero. Synthetic tests only; live/replay require real records. Animations != evidence. Frontend != independent domain authority.

## Source reliability note
Earlier discussion included references to repository routes and capabilities (`mesh.peers`, `mesh.events`, `monitor.stream`, `/api/v1/monitor`, `web.surfaces`, design tokens, etc.). These are **investigation hypotheses based on prior conversation**, not a fresh verified checkout audit. The implementing agent MUST inspect actual sources and adapt without inventing capabilities or claiming proof from this pack.


## FINAL OVERRIDING REQUIREMENT — REAL DATA ONLY
All production/runtime exhibition modes (including marketing and demo) MUST use real observed live records or clearly-labeled REPLAY of real captured, authorized, redacted records. Synthetic events/metrics/topology MUST NEVER flow to a running exhibition. Synthetic data is permitted ONLY in isolated automated tests and explicitly non-deployable conceptual sketches. Decorative motion does not represent operational activity. When data does not exist, display unknown/unavailable/idle and reason. This overrides any earlier wording suggesting synthetic demonstration.


**Easiest:** paste `ALL-IN-ONE-CLAUDE-PROMPT.md` to Claude Code from the unpacked repository. The HTML file is a non-deployable, explicitly synthetic art-direction sketch only.
