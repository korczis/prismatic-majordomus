# Mission 02 — P0 mesh durability, identity and convergence

Trace from `docs/MESH.md` and real Rust implementation. The documented mesh is NOT linearizable, NOT a TLS transport, NOT generic remote execution and does NOT guarantee NAT traversal. Do not erase these honest limits.

## Stream A: durable journal, I2706 / GitHub #1137

Locate journal append, ACK publication, queue fanout, reloading, truncation recovery, event deduplication and fold. Define explicit state transitions: `proposed → validated → durable → acknowledged → broadcast` or correct documented local ordering. What is durable means syscall/storage-appropriate, no false fsync claims. Never expose success after failed append. Ensure bounded reload (bytes, count, per-frame cap, time/memory budget) with typed errors. Preserve existing replay and migration semantics. Test full disk (ENOSPC), denied write, partial write, CRC/schema corruption where applicable, process crash boundaries, duplicated event id, old/new format and resumption.

## Stream B: trust, key lifecycle, I2705 / #1136; trust reload #981 and revocation #989

Trace key generation and filesystem permissions from first byte, handshake signature binding to repository, runtime, protocol, freshness and nonce. Implement/complete authenticated succession after rotation, reject old key on every fresh hello, perform live revocation on effective trust policy reload, revalidate loaded events. Define how partitions heal without granting unauthorized authority. Ensure key material never leaves node or enters event serialization. Test multi-node rotation, replay, revoked identity, peer spoofing, malicious clocks, interrupted rotation and restart. Never claim crypto protocol audited unless independent review exists.

## Stream C: bounded dialer, I2704 / #1135; hello ordering #1134

Cap inbound/outbound frame sizes, candidate count, pending connections and retries; exponential backoff with bounded jitter, measurement and clear terminal conditions. Refuse unknown protocol fields or explain compatibility mechanism. Tests with unresponsive malicious peer, huge input, slowloris-like drip, duplicate candidates and handshake order mutation.

## Stream D: canonical event contract, I2708 / #1139; event pagination #993

Audit every payload field, version, allowlist, redaction and logging path. Unknown field behavior explicit and tested (forward-compatible reserved extension namespace only if canonical). `mesh.events` pagination must not skip/duplicate events under concurrent writes, reconnect and independent streams. Revisit PR #922's described 986 duplicated session_opened events in one 1,000-event sample. Add regression proving long unchanged intent projected once per semantic update, bounded journal amplification and idempotent fold. Record post-fix rate on real fixture; don't claim 98.6% is current.

## Fault matrix

Test: ENOSPC, EIO, truncated journal, bad signature, replayed nonce, clock skew, concurrent writers, event reordering, partition/heal, peer disconnect, stale trust cache, 10x payload size, unknown fields, restart/recovery. Favor deterministic simulated clocks/network and model/property tests over flaky sleeps. Cross-node tests only when available; local emulation must be labeled.

Outputs: code + tests + bounded telemetry fields + docs/MESH accuracy + claims fixes + executable gates. Acceptance: any ACK implies durable write according to documented durability contract; replay deterministic; malicious/out-of-trust inputs never become authority; resources bounded; all claims match tests.
