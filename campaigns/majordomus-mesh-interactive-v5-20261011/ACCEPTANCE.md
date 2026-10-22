# E2E acceptance matrix

| Test | Expected |
|---|---|
| Load /ui/mesh | Server-owned route, SSR, actual node list and links |
| Pick node/edge by mouse, touch, keyboard | Shared selected canonical ID, inspector + deep URL |
| Reload deep link/back | Selection restored, no duplicate websocket subscriptions |
| Local/remote node identity | Stable node/runtime/instance distinction |
| Authorized run | Canonical executor, execution ID, events, evidence |
| Unsupported remote run | Honest disabled/absent action with explanation |
| Forged/revoked identity | No action, no privileged stream |
| Event loss/duplicates/reorder | Gap detection, replay/resnapshot, no invented continuity |
| Peer disappears | Stale/unreachable, retained last-seen provenance |
| Empty dataset | Helpful truthful empty state, no generated demo nodes |
| Offline/browser sleep | Reconnect and bounded buffer |
| Mobile/reduced-motion/no-JS | Reachable graph equivalent and action alternatives |
| Multiple surfaces | Identical entity IDs, state semantics and execution IDs |
| Release/deploy | Real GitHub and production evidence, not prompt assertion |
