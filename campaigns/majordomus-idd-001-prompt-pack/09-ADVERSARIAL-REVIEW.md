# MISSION 09 — ADVERSARIAL ARCHITECTURE AND SAFETY REVIEW

Before marking any increment complete, play independent critical reviewer. Challenge every layer on:

1. Is this duplicate of an existing capability, policy, schema or renderer?
2. Does any UI operation bypass shared executor, authorization, audit or version checks?
3. Can two users/peers execute the same effect twice because of a retry?
4. Can stale/out-of-order WebSocket events move the UI backwards or falsely report completion?
5. Can a disconnected/restarted peer cause lost work, lease theft or phantom success?
6. Can generated docs and screenshots claim more than runtime evidence supports?
7. Can model-proposed/retrieved text mutate policy or authorize a dangerous tool?
8. Can logs, markdown or graph labels execute script or expose credentials?
9. Is the graph a visualization of domain facts or another independent state machine?
10. Is the governance delta an enforceable improvement or decorative doctrine inflation?
11. Did this increase the user task's measured total cost, CI time or operational toil?
12. Is every enabled action understandable, reversible when practical, and properly gated?
13. Is mobile, keyboard, accessibility and disconnected behavior actually tested?
14. Are implementation promises anchored in observed facts and meaningful integration tests?
15. Can a feature be added without manually editing multiple consumer inventories?

Record severity, minimal reproduction, root cause, rejected shortcuts, test that catches it, fix and re-run evidence. Block release for unmitigated critical/major issues under current repo policy. Do not sign off your own output through intuition alone.
