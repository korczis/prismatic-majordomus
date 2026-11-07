# Phase 06 — Secure multi-host transport and fallback

Read `../EXECUTION-CONTRACT.md` and `../MASTER-PROMPT.md` first. Reconcile with CURRENT HEAD and skip already-landed equivalent implementation only after verifying it.

## Goal and implementation tasks
Once local E2E + durability is stable, evaluate whether existing remote transport is sufficient. Extend fabric with authenticated QUIC-over-UDP when justified, TLS 1.3, ALPN/protocol version, peer trust enrollment, certificate rotation or identity bindings, address selection, connection pooling and flow control. Fallback to TCP/TLS or authenticated relay for blocked UDP. LAN discovery may use mDNS/multicast ONLY for advertising nonsecret reachability hints; no broadcast payloads, no trusting discovery without authenticated handshake. Plan NAT traversal conservatively and do not claim universal hole punching. Test hostile packets, replay, certificate rejection, cross-repo isolation, downshift downgrade, lost datagrams, duplication, partition/healing and rate limits. Make security decisions based on actual repository auth/trust design; document threat model.

## Hard acceptance / evidence
Evidence: two-host/container-network test, UDP-blocked fallback trace, unauthorized peer denial, cert rollover test and no exposed sensitive metadata in discovery.

## Phase completion report
Provide: changed owner modules; deleted legacy/duplication; new/changed canonical capabilities; schema/version implications; security and failure test results; cross-surface status; exact build/test commands/results; benchmarks if relevant; repo SHA/commit/PR/merge status; unresolved blockers with next executable action. Every claim must be OBSERVED/IMPLEMENTED/VERIFIED. Re-run affected canonical quality gates. Never auto-disable checks.
