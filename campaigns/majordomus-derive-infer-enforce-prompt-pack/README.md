# Prompt pack: Majordomus derive/infer/verify/enforce

Use `00-MASTER.md` as the primary prompt for Claude Code Opus/Fable in the checked-out repository. Give the agent all mission files in order, or feed them sequentially after the master. The master explicitly requires inspection before assumptions, phased vertical slices, actual tests/gates and recorded proof. No commands or paths are presumed to exist beyond being discovery clues.

Sequence: 01 audit -> 02 projections -> 03 evidence -> 04 .ai enforcement -> 05 UX/site -> 06 CI/release/dogfooding. Milestones are checkpoints, not invitations to create six disconnected PRs. Prefer minimal coherent patches and continuously integrate proven changes.

Definition of done: root causes fixed, cross-surface outputs derived, unknown/stale evidence correctly presented, intentional tampering blocked, tests passing and recorded, .ai dogfooding proven in source and fixture repos, current site/release verified with exact revision. No claim of deployment before live verification.
