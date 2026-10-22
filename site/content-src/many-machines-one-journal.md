+++
title = "Many machines, one journal"
description = "A peer board lives in one process's memory, so a session on another machine cannot read it. How the Majordomus mesh admits a link between runtimes by a signed handshake, replicates sessions, claims, handovers and reviews as signed events of one journal that every runtime folds the same way, and lets a claim expire with its holder's heartbeat."
template = "page.html"
[extra]
lang = "en"
related = ["seats-not-names/", "claimed-before-it-is-built/", "a-session-ends-the-work-does-not/"]
[extra.translations]
cs = "mnoho-stroju-jeden-denik/"
+++

By the middle of September 2026 the mesh could find other machines. [ADR 0050](@/adrs/adr-0050.md) (proposed) had given it discovery: signed advertisements, one registry, and trust as a label, and deliberately nothing more. Measured on 15 September, the mesh could not support the product it was presented as, and [ADR 0067](@/adrs/adr-0067.md) records why.

| what was measured | why |
|---|---|
| nothing cooperative crossed a machine | claims lived on one server's peer board and handovers under one checkout's `.ai/local/`; no event, session or review travelled |
| one repository on two machines was two repositories | advertisements carried a digest of the git common directory's path, and a macOS checkout's path never matched the Linux path of the same clone |
| presence was a sighting, not a relationship | a node was present for sixty seconds after an advertisement, whether or not anything could be exchanged with it |

## A board stays in its process

The peer board already modelled cooperation correctly, but its scope was one process. [ADR 0044](@/adrs/adr-0044.md) (accepted) gathers the boards of one machine's checkouts through their leases, and the board stays in memory by design. Sending it to other machines was refused: it is positional, so `p1` is reused, it lives in memory and it is unsigned, and replicating it would replicate its ambiguities. Two sentences in [`docs/MESH.md`](@/docs/mesh.md) govern what was built instead. Discovery creates awareness, not authority. A link is admitted by a handshake, and everything cooperative is a signed event of one journal that every runtime folds the same way.

## A machine is trusted by a commit

A node is an Ed25519 key created once per user and machine, and its id is the key's digest, never an address; a runtime is one checkout's server on that node. The repository is named by the digest of its root commits, which every full clone shares, and runtimes of different repositories never link.

Nobody is trusted by default: under `deny_unknown` a valid unknown key is observed, listed and trusted for nothing, and a machine whose own key is not listed keeps the mesh closed. Adding a machine means running `majordomus mesh identity` there and committing its key under `trust.allow`. A working tree may narrow trust and never widen it, so a branch checked out for review cannot make the checkout trust its author's key.

Trust is a precondition of a link, not a link. A dialer posts a signed hello, and the answerer checks, among other things, its signature, a timestamp within 300 seconds of its own clock, the protocol range, the repository identity and the trust policy, and last whether the nonce was answered before. The welcome is checked the same way, and every failure is a typed refusal, counted and listed.

## One journal, folded the same way

Everything cooperative is an event of one journal: a session opened or closed, a claim acquired or released, a handover published or consumed, a review requested, answered or withdrawn. Each event carries its stream, a sequence number and a Lamport stamp, and is signed by its origin's key, so a relay can withhold an event and cannot forge or alter one. Delivery is at least once, and a duplicate changes nothing. Within a stream events apply in sequence order, across streams by Lamport stamp, stream and sequence, and a property test holds that arrival order never changes the state. A sync round compares marks and carries only what the other side lacks. Every runtime with the same events and the same liveness verdicts computes the same state and the same digest.

What crosses machines is sessions, claims, handovers and reviews, and the manual guarantees the replication of nothing else; a session shares metadata such as its intent, branch and head, never a context body. The board feeds the journal every heartbeat: an attached MCP session becomes the mesh session `board-<peer>`, and each announcement an advisory claim. No surface keeps peers, links or claims of its own. Otherwise, says the rule [project.mesh-cooperation-is-authenticated](@/rules/project-mesh-cooperation-is-authenticated-1.md), the Cockpit ends up showing a green dot for a runtime the CLI knows is gone.

## A claim lives on a beat

A claim is a set of repository-relative paths held by a session, exclusive by default or advisory. An advisory claim, such as a board announcement or the scope `majordomus start` claims for a task, reports overlaps and refuses nothing. An exclusive claim that meets a live exclusive claim of another session the runtime knows is refused as `claim_conflict`, and the refusal names the claims it meets. Two exclusive claims made where neither runtime could see the other both enter the journal, and the fold on every runtime names the same winner, the one acquired with the lowest Lamport stamp, stream and sequence. The rule calls this the product's promise: an exclusive claim excludes everywhere or nowhere.

A claim holds while it is unreleased, its session is open and its stream's beat rose within the expiry; that is the whole lease. Each runtime raises its own signed beat every heartbeat, five seconds by default, and sync rounds relay every stream's beat with its age, so a crashed runtime stops beating for everyone at once. Freshness is judged on each runtime's own monotonic clock, never by comparing two machines' wall clocks. The expiry is thirty seconds by default, and a reloaded journal restores events but no freshness, so a restart resurrects nobody's ownership.

The manual records this on 15 September, between a MacBook Pro and the Linux machine lundra. A claim written on the Mac was held on lundra half a second later. When lundra's server was killed with `kill -9`, the Mac expired the link after the run's ten-second expiry, with lundra's claim `expired: its runtime stopped beating`, and could then claim that scope.

## Handovers and reviews cross the link

`mesh handover publish` turns the newest record `majordomus handover` wrote under `.ai/local/state/handovers/`, or one named there and nothing else, into a journal event carrying its task, branch, head, time and Markdown body, and no path of the publishing machine. On another machine, `mesh handover consume <id> --session <s>` records the consumption and writes the handover into that checkout's handovers directory, so `majordomus handover --resolve` on that branch finds it. That is the only file a remote event can cause to be written, and only by an explicit local command; nothing a peer sends executes anything.

A review asked on one machine is answered on another. A request ends in one of three ways: another session answers it with a verdict, the runtime that asked withdraws it, or it expires because nobody answered and the asking runtime stopped beating. The asking session closing does not end it, because a lane's request outlives many connections.

## What the mesh does not do

It is not a lock service with strong consistency: a partition lets both sides claim, and the conflict is named after healing. It is not a file synchroniser, because git owns the source. Nothing is encrypted, so an observer on the network sees the session intents, claim scopes, review subjects and published handover bodies of linked peers, and there is no NAT traversal. A compromised trusted peer can publish misleading claims and handovers under its own identity, believed until they expire with it.

None of it is on by default: the skeleton a new repository starts from ships no mesh declaration, and this repository committed its own ([ADR 0059](@/adrs/adr-0059.md), proposed). ADR 0067 is still proposed; the rule that carries it is active and blocking, held by `scripts/ci/mesh-check`, by tests that link separate server processes over TCP, and by a lab of Linux containers that partitions and kills nodes.

The bootstrap the repository generates for its workers sums it up: claim work that must not overlap across machines exclusively with `majordomus_mesh_claim`, hand it over with `majordomus_mesh_handover_publish`, take it with `majordomus_mesh_handover_consume`, ask for a review with `majordomus_mesh_review_request`, and invent no other coordination channel. How a claim holds a role is in [Seats, not names](@/seats-not-names.md), why a session claims before it builds in [Claimed before it is built](@/claimed-before-it-is-built.md), and the whole mesh in [`docs/MESH.md`](@/docs/mesh.md).
