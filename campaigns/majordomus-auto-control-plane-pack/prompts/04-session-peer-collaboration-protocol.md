# 04 — Session, Peer and Collaboration Protocol

A socket connection is transport, not cooperation. Build/reconcile the canonical collaboration semantics.

## Required protocol concepts

Use existing event/protocol machinery if available. Otherwise establish typed/versioned equivalents for:

- SessionAttached / SessionDetached
- PresenceHeartbeat
- CapabilitiesPublished
- PeerObserved / PeerExpired
- WorkClaimed / WorkClaimRenewed / WorkReleased / WorkExpired
- HandoverPublished / HandoverLoaded
- ContextCheckpointed
- DiagnosticPublished
- resync/snapshot events

Names may differ according to repository conventions.

## Event envelope

Define explicit schema/version and correlation identity. Include project/worktree/session/agent scopes where relevant.

If event order matters, define sequence/cursor semantics. If replay exists, test resume after disconnect.

## Presence

Presence must:

- register automatically,
- heartbeat/renew,
- expire stale agents,
- reconnect cleanly,
- distinguish sessions/providers/worktrees,
- survive control-plane restart according to defined semantics.

## Work claims

Claims must be leases, not immortal locks.

Define scope and conflict behavior. A claim should help detect two agents doing the same work without making recovery impossible after crashes.

Integrate with existing issue/milestone/worktree concepts where canonical metadata already exists. Do not invent a competing issue tracker.

## Session context + handover

Unify current session context/handover loading and persistence.

Discover and migrate conflicting legacy locations/formats rather than leaving several active systems.

Auto-attach must load relevant context without a human pasting it.

## Transport

WebSocket/SSE/other transport must carry typed canonical events. Do not let Cockpit and agent clients receive separate incompatible message formats.

## Tests

- two peers see each other,
- heartbeat expiry,
- disconnect/reconnect,
- control-plane restart/resync,
- claim conflict + release,
- claim expiry after crash,
- handover publish/load,
- two worktrees stay distinguishable,
- deterministic snapshot after event replay.
