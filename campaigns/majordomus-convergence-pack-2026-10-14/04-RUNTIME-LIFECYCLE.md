# Mission 04 — runtime, lease, startup and version safety

Find implementation corresponding to I2700 #1131, I2701 #1132, I2702 #1133, I2707 #1138, I2163 #1000, I2127 #983 and related claims. Verify which are already implemented.

- One checkout's runtime address must be deterministic when documented; explain collision resolution, IPv4/IPv6, daemon termination and port fallback if allowed.
- Lease is typed: PID alone insufficient. Include stable checkout/runtime identity, random lease token, process start identity as appropriate. Probe a cheap liveness endpoint and validate holder token; never mistake arbitrary service on same port for legitimate runtime.
- Start/stop/replace operations are idempotent; stale leases are invalidated only with proof; losing lease stops advertising and hands over safely. Persist no stale false health.
- Version contract: client/runtime/schema compatibility matrix, readable rejection reason, graceful transition to newer server only under authorized supported workflow, no unsafe auto-replacement.
- Idle timeout comes from canonical runtime configuration; CLI override validated and introspectable, no hidden hardcoded counter in JavaScript.
- Doctor clearly distinguishes policy health from runtime health and discovers stale server; no 'healthy' on unknown runtime.
- Unit tests: parallel starters, stale PID reused, token mismatch, crash, restart, stop midway, bind collision, version mismatch, no server, partial mesh outage.
- Integration: 2+ checkouts sharing a server, reconnect with state rehydration, no ghost sessions, no lease stolen by losing peer, recover after forced kill.
- Expose same typed lifecycle state across CLI, API, MCP, UI if meaningful; where unavailable return unknown with cause.

Deliver code, tests, docs, updated claims, exact observed startup latency and memory footprint if measured. Preserve portability and basic CLI without server.
