+++
title = "Majordomus Mesh"
description = "the mesh: how running Majordomus instances discover each other — Ed25519 node identity, one signed bounded envelope over multicast, broadcast and rendezvous, one registry, deny-unknown trust, the off-by-default declaration, the threat model, and how to operate and extend it"
weight = 64
[extra]
source = "docs/MESH.md"
+++

{% raw %}

How Majordomus runtimes of one repository — on one machine or several — find each other,
decide whether to cooperate, and share the state of their work. The decisions are ADR 0050
(discovery) and ADR 0067 (cooperation); the invariants are the rules
`project.mesh-is-observation-not-authority` and `project.mesh-cooperation-is-authenticated`;
the implementation is `apps/majordomus-cli/src/mesh/`. This document is the operator's and
contributor's view: what runs, what travels, what is guaranteed and what is not, how to
operate it, and how it is tested.

Two sentences govern everything here:

> Discovery creates awareness, not authority.
>
> A link is admitted by a handshake, and everything cooperative is a signed event of one
> journal that every runtime folds the same way.

## What it is, and what it is not

**It is** how several AI coding sessions — in several terminals, worktrees and machines —
work one repository without colliding: each runtime sees the others' sessions, an exclusive
claim made anywhere refuses an overlapping claim everywhere, a handover written on one
machine is consumed on another, a review asked on one is answered on another, and a runtime
that disappears stops holding anything, everywhere, on its own.

**It is not** a lock service with strong consistency (a partition lets both sides claim; the
conflict is named after healing), a file synchroniser (git owns source), a remote execution
facility (nothing a peer sends runs anything), an encrypted transport (only an `https://` hub
or seed is reached over TLS; discovery and links to advertised addresses are cleartext, kept off
public networks), or a WAN federation (no NAT traversal; rendezvous and seeds need reachable
addresses).

## The shape

<pre class="mermaid">
flowchart TD
  decl["mesh declaration&lt;br&gt;.ai/repo/mesh/&amp;lt;id&amp;gt;.yaml&lt;br&gt;(enabled, trust, cooperation)"]
  identity["NodeIdentity&lt;br&gt;Ed25519 key per user and machine&lt;br&gt;node id = digest(key)"]
  subgraph discoveryLayer["discovery (ADR 0050) — candidates, never authority"]
    providers["providers: udp_multicast · udp_broadcast · rendezvous"]
    registry["MeshRegistry&lt;br&gt;one record per node × runtime"]
  end
  subgraph cooperationLayer["cooperation (ADR 0067)"]
    link["link handshake (link.rs)&lt;br&gt;signature · freshness · nonce · protocol ·&lt;br&gt;self · repository · trust"]
    table["peer-link table (cooperation.rs)&lt;br&gt;connecting · connected · degraded ·&lt;br&gt;unreachable · expired"]
    journal["Journal (journal.rs)&lt;br&gt;signed events (stream, seq), Lamport,&lt;br&gt;beats, marks"]
    fold["fold (state.rs)&lt;br&gt;sessions · claims · conflicts ·&lt;br&gt;handovers · reviews · digest"]
  end
  surfaces["mesh capability module&lt;br&gt;CLI · HTTP · OpenAPI · MCP · Cockpit"]
  board["peer board (peers.rs)"]
  decl --&gt; discoveryLayer
  decl --&gt; cooperationLayer
  identity --&gt; discoveryLayer
  identity --&gt; link
  providers --&gt; registry
  registry --&gt;|"trusted, same repository"| link
  decl --&gt;|"seeds"| link
  link --&gt; table
  table --&gt;|"sync every heartbeat"| journal
  board --&gt;|"projected each heartbeat"| journal
  journal --&gt; fold
  registry --&gt; surfaces
  table --&gt; surfaces
  fold --&gt; surfaces
</pre>


## Identity

<div class="overflow-x-auto" tabindex="0">

| identity | what it names | derived from | survives |
|---|---|---|---|
| node | a machine's Majordomus key | digest of the Ed25519 public key in `$XDG_STATE_HOME/majordomus/node.json` | restarts, re-addressing; lost with the file |
| runtime | one checkout's server on a node | digest of the checkout id | server restarts; two worktrees are two runtimes |
| instance | one process run of a runtime | OS entropy at start | nothing — a restart is a new instance |
| stream | what writes events: `node-runtime-instance` | the three above | nothing — a restart is a new stream of the same runtime |
| session | one worker's session within a stream | named by the caller (`board-p2` for an MCP session, anything for the CLI) | the stream |
| repository | the repository being cooperated on | digest of its root commits, or `cooperation.repository` | clones, paths, machines |

</div>


A hosted hub has no disk that outlives a boot, so a key minted there would make it a new,
untrusted node at every restart. `majordomus serve --node-seed-env VAR` takes the node key from
the 64-hex-digit seed in that environment variable instead (a secret of the platform), writes
nothing to `node.json`, and keeps one node id across every boot; with the variable unset or
malformed the mesh stays off and its reason names the variable, never the value (ADR 0220,
I3826). The seed's public key is what `trust.allow` lists for the hub.

An address, a port, a host name or a PID is never identity. Two worktrees of one machine
share the node key and are two runtimes; a restarted server keeps its runtime and changes
its instance, so its peers list it once with `restarts` counted.

The repository identity is what isolation rests on. The git directory's path — what
`server.status` digests to tell checkouts of one machine apart — differs between machines and
cannot be it. Root commits are shared by every full clone and by no unrelated repository. A
shallow clone's "root" is where its history was cut, so a shallow clone must declare
`cooperation.repository`, and so must a fork that should not cooperate with its origin; the
doctor says which applies.

## Discovery

Discovery finds candidates. Every datagram heard — multicast (preferred; group, port and TTL
declared), broadcast (a declared fallback), or a rendezvous answer — passes one verification
path in `mesh::manager` (size → shape → version → bounds → staleness → signature → own-runtime
skip → trust policy) into one registry keyed by node and runtime. Envelopes are at most 1200
bytes, signed, and carry the key, runtime, instance, endpoints, transports, repository
identities and version; never a secret, a path or repository content.

A tailnet is a discovery source too. When the declaration's `tailscale.enabled` is on (the
default), the Tailscale provider reads `tailscale status --json` every `interval_seconds`
(60) and asks every peer listed online as a rendezvous at
`http://<tailnet address>:<port>` (port 8791 unless declared). What comes back is the same
signed `mesh.register` answer a declared rendezvous gives, through the same verification
path, so the provider proposes addresses and decides nothing: a machine that joins the
tailnet is found with no address written by hand, and the trust list still decides what
links. Without the Tailscale CLI the provider is listed `stopped` with the reason, which is
no fault, so `mesh doctor` never fails a repository whose machines do not run Tailscale. A
CLI that is there and answers an error is listed `failed`; the others run on either way.

```yaml
tailscale:
  enabled: true        # the default
  port: 8791           # where a peer's server answers mesh.register
  interval_seconds: 60
  command: tailscale   # the CLI to ask
```

Discovery protocol **2** added the runtime slot. A version-2 reader reads version 1 (as the
node's unnamed runtime); a version-1 reader refuses version 2 and counts it as `version`, so
a mixed fleet shows the mismatch instead of going quiet.

Multicast sockets are bound with `SO_REUSEADDR` and `SO_REUSEPORT`, so every runtime of a
machine hears the group. A server bound to `0.0.0.0` advertises its interfaces' addresses,
physical and overlay interfaces before container bridges; a server bound to loopback
advertises loopback, which only its own machine can dial. A server behind a platform's router
dials nobody at its interface addresses, so `serve --advertise URL` advertises that one public
URL instead; `mesh.status` lists what a node advertises as `advertised`. The Gigalixir
deployment's generated Procfile passes both `--advertise` and `--node-seed-env` from the
deployment object (ADR 0219, ADR 0220).

## Trust

Trust is decided by the declared policy, and the default is that no one is trusted:

<div class="overflow-x-auto" tabindex="0">

| policy | a valid unknown key is | notes |
|---|---|---|
| `deny_unknown` (default) | observed, listed, trusted for nothing | with `trust.allow`, the recommended shape for a fleet |
| `allowlist` | observed; listed keys are trusted | the same as `deny_unknown` plus `allow` |
| `tofu` | trusted on first use | a development convenience for a network you control: under TOFU **every** key on the segment that knows the repository identity is trusted and can link, claim and publish; every listing says `tofu`, and TOFU-trusted records are evictable |

</div>


A key withdrawn from `trust.allow` loses its links as soon as the server reads the change: the server puts the declaration's trust in force with every new generation of the repository, ends the link of every peer it no longer trusts, and refuses that peer's next hello `untrusted` — no restart (I2134). This machine's own key is trusted as itself, which is what lets two worktrees link. A node id
reappearing under a different key is rejected under every policy. Trust is a precondition of
a link, not a link: discovery never admits one.

Under `deny_unknown` and `allowlist`, a machine whose own key is not in `trust.allow` keeps the
mesh **closed**: it binds no multicast socket, registers with no hub, dials no seed and opens no
link, and `mesh status` gives the reason with the key to list. This covers a contributor's
clone of a public repository and a new machine nobody has enrolled yet. Without the rule, such
a machine would announce itself on its network and dial every endpoint the repository commits,
while every peer refused it. `mesh doctor` fails its `trust` check with the key and the remedy,
its `runtime` check reads `closed: …`, and the session's briefing says `Mesh: CLOSED — …`.
Listing the key and restarting the server opens it. `tofu` never closes on this ground.

### Which copy of the declaration is obeyed

A working tree may narrow trust and never widen it. When the trunk holds a committed copy of
the declaration at the same path, the runtime of a checkout obeys the working tree's copy with
every widening of the trunk's removed: the mesh is on only if the trunk's copy is on, the
policy is no looser than the trunk's, and every key in `trust.allow`, every rendezvous
endpoint, seed and broadcast network is one the trunk's copy also lists — or, for an
endpoint or a seed, a loopback address, which reaches no other host. When the trunk holds no
copy (a repository that never committed one, or a directory outside git), the working tree
is obeyed as written except `tofu`, which is honoured only from a committed trunk copy.

So a contributor's branch, checked out to be reviewed, cannot make that checkout's runtime
trust its author's key or post its card to its author's host, and an uncommitted edit cannot
either; a machine or a hub takes effect when its line reaches the trunk. `mesh doctor` fails
its `trust-root` check naming every line this checkout ignores, and the server logs them when
it starts. `scripts/ci/mesh-check` refuses a committed declaration that sets `tofu`.
The trunk is the local branch the repository names as its trunk; a local trunk behind the
remote is older trust, which is the safe direction.

## The handshake

A dialer — the runtime with the lower runtime key, or either one when only one can reach the
other, or any runtime with a declared seed — posts a signed **hello** to
`POST /api/v1/mesh/link/hello`: its runtime card (key, runtime, instance, repository, name,
version, features, endpoints), a 32-hex nonce, a timestamp, its link protocol range and,
when the dialer knows whom it dials, the runtime key it means to reach (`to`). The answerer
admits it only if, in the order the code checks:

1. the message is within 900 KiB and has the shape of a hello;
2. the Ed25519 signature verifies under the hello domain (a signature over another message
   kind can never be replayed as a hello);
3. the timestamp is within ±300 s of its clock;
4. the nonce has 32 hex characters;
5. the protocol ranges meet (link protocol 1..1 today; the highest common version is chosen);
6. the peer is not this runtime;
7. a hello that names its recipient names this runtime — a copy captured on its way to
   another runtime of the repository is refused `misaddressed`;
8. the peer's repository identity equals its own;
9. the peer's key is trusted by its policy;
10. the hello is no older than the one that made the peer's live link, and its nonce has not
    been answered before. Both are checked last on purpose: the replay cache holds only nonces
    of trusted keys of this repository, so a stranger's hellos can neither fill it nor evict a
    real peer's nonce. The age check is what the cache cannot be after a restart: a hello
    captured before this runtime restarted, replayed within the skew window once the peer has
    linked again, is refused `replay` rather than displacing that link.

A hello without `to` — from a seed, a first contact, or a release before it — is still
admitted, and is protected from replay to another runtime only by the per-runtime nonce cache
and the skew window.

It then answers with a signed **welcome** — its own card, a link id, its marks, the echoed
nonce — and the dialer applies the same checks to the welcome (signature, echoed nonce,
timestamp, protocol, self, repository, trust). Every failure on either side is a typed
refusal (`malformed`, `oversized`, `signature`, `stale`, `replay`, `protocol_unsupported`,
`self_link`, `misaddressed`, `repository_mismatch`, `untrusted`, `capacity`; a sync can also be refused
`not_active` or `unknown_link`, and an operation `feature_unsupported`),
counted in `refused_in`/`refused_out` and listed with its detail on `mesh.cooperation`.

## Transport

The link protocol posts `Signed` messages through `LinkTransport`. The shared server's
transport is HTTP/1.1 through the crate's one minimal client (ADR 0032: no HTTP client
dependency, no async runtime); unit tests put runtimes behind an in-process transport that runs
the same protocol. An endpoint written `https://host[:port]` — a rendezvous hub or a seed, by
host name or address — is reached over TLS (rustls with the ring provider), the certificate
verified against the Mozilla roots compiled into the executable and the host name checked
against it; a certificate that does not verify is refused naming the endpoint, before any byte
of the request is sent. An `http://` endpoint, and an advertised `host:port` a link dials, is
cleartext, and the client sends it only when every address the host resolves to is loopback,
RFC 1918 private, link-local, `100.64.0.0/10` (a tailnet) or IPv6 unique-local: a public address
is refused before the connection, with the correction `use https://` (ADR 0220). `mesh doctor`
reports the same verdict for every declared hub and seed as its `transport` check, without
dialing. Authenticity is the protocol's; confidentiality is TLS's for an `https://` endpoint and
the network's otherwise — run links on a private network or an overlay (a tailnet, WireGuard).

## Presence, heartbeats and expiry

Three layers, deliberately separate:

- **Links** are this runtime's connections. Every heartbeat (`cooperation.heartbeat_seconds`,
  default 5) the dialer runs one sync round. A link is `connected` with an exchange within
  two heartbeats plus one second, `degraded` beyond that, `unreachable` beyond half the expiry, and `expired`
  — dropped — beyond `cooperation.expiry_seconds` (default 30, at least three heartbeats).
  A failed round backs off exponentially, capped at the expiry, so a returning peer is dialed
  again within one expiry; an unknown link id (the peer restarted) re-handshakes at once, into
  the same table row.
- **Streams** are alive while their beat rises. Each runtime raises its own beat every
  heartbeat; sync rounds carry every stream's beat with how long ago the sender saw it rise,
  so a runtime reached only through a relay is alive, and a crashed runtime stops beating for
  everyone at once. A beat is signed by its origin's key, so a relay can carry a beat but
  never mint one, and the age it reports is clamped to the expiry. Freshness is computed on each runtime's own monotonic clock — no two
  machines' wall clocks are compared. A reloaded journal restores events but no freshness,
  so a restart resurrects nobody's ownership.
- **Sessions** make progress while their worker does real work (I3990). A stream that beats
  proves its runtime is up and nothing about the worker behind one of its sessions: a session
  stuck on a permission dialog, hung or crashed behind a live server used to keep every
  exclusive claim it held for as long as that server ran — the 5d executor held the
  integration lease for hours and never answered (2026-10-22). Real work is an MCP request of
  any method but `ping` and `notifications/*` (over stdio or plain HTTP alike), a board
  announcement, a claim, or an explicit `majordomus mesh beat --session <id> --phase <text>
  [--pid <n>]` (`mesh.beat`) from a long-running process; the stdio bridge pings every 20 s
  whatever its client is doing, so a ping is never progress, while a phase that names a wait
  (`waiting on CI run 123`) is — a process waiting on something outside itself is working.
  While a board session (`board-<peer>`) holds a live claim and works, its runtime writes a
  `session_beat` event carrying the session's ttl (`cooperation.session_ttl_seconds`, default
  600, at least four heartbeats, at most 3600, `0` turns the rule off): at once when it
  claims, then at most once a quarter of the ttl, and only after new work; `mesh beat` writes
  one with its phase (redacted, one line), and a repeat inside a quarter of the ttl with the
  same phase writes nothing. Every runtime ages the newest beat from the moment it stored it,
  on its own clock — the holder against the ttl, every other runtime against the ttl plus the
  expiry — and a held **exclusive** claim whose session's beat is older than that is
  **suspended**: `its holder stopped making progress: no beat for over <ttl>s`. A suspended
  claim refuses nobody and is counted and shown as such (`mesh.state`, `mesh.peers` with the
  session's `progress_ms` and `phase`, the Cockpit, `/ui/mesh`); nothing is written to make it
  so, the session stays open, an advisory claim is untouched, and the claim holds again at
  its session's next beat. Suspension is advisory-first: it stops the refusal and says so, and
  nothing more — no take-over, no fencing. A holder that resumes after another session was
  admitted to its scope does not get it back: its next beat releases the suspended claim
  instead of re-arming it (below); fencing a holder that goes on writing without beating
  waits for an epoch (I3174). A session that never beat — a `mesh claim --session` from the command line that never runs `mesh beat`,
  every session of a runtime older than this layer — keeps the stream rule alone, and a beat
  with ttl `0` asks for exactly that. A runtime writes at most one beat per session a quarter of the ttl however
  often `mesh beat` is called — a new phase inside the interval is held and shown, and the
  next beat carries it — and at most `8 × ceil(3600 s / (ttl / 4))` beats an hour, sized for
  eight exclusive holders at full rate; past that it withholds beats, the claims it cannot
  beat for suspend, and it says so in its log and in `mesh doctor`'s `session_beats` check —
  never silently. Only a session holding a live exclusive claim is beaten for: an advisory
  claim refuses nobody and costs no beat. When a holder resumes after another session was
  admitted over its suspended claim, its next beat does not re-arm the claim: its runtime
  writes `claim_released` with the reason `suspended while its holder made no progress, and
  another session was admitted to its scope`, and the holder claims again through admission
  — so two sessions never both hold one scope. A restarted reader counts the beats its
  journal reloads as stored at the restart, exactly as it counts a stream it relearns
  (`learned_at`): a session silent before the restart reads as progressing for at most its
  ttl plus the expiry after it. The key is left out of the repository's declaration on
  purpose: an older executable refuses a key it does not know.
- **A session waiting on a person says so** (I3992). A beat whose phase opens with `blocked:`
  — `blocked: permission dialog (Claude needs your permission to use Bash)`, `blocked:
  awaiting input` — marks the session as waiting on a person, so that peers and the landing
  seat see it within seconds, and why, instead of reading a ttl's silence. A mark is
  information and never progress: it moves no session's progress clock on any runtime that
  knows marks, so a session that worked and then only waits is suspended by its ttl exactly
  as before, however often it says so, and a session that only ever waited starts no clock
  and keeps the stream rule; it re-arms and releases nothing. A mark is written at once —
  inside the interval, and for a session holding no exclusive claim too — and saying the same
  wait again writes nothing. The first beat after it that is not a wait is the session's next
  real work: it clears the mark at once, is progress, and progress keeps its interval after
  it. Marks and their clears are counted in a window of their own, of the same size as the
  progress beats', so that a session whose turns end often cannot spend the beats that keep
  other sessions' claims excluding; past it a mark is withheld, logged, and still shown by
  the runtime that holds it. `mesh.peers` carries the reason as `blocked` beside the
  `phase`, and `mesh peers`, the Cockpit and `/ui/mesh` show it with the session. A runtime
  older than marks reads a mark as a phase like any other, so on it a mark keeps the claims
  of a session that is already waiting for up to one more ttl.

## Events, ordering and deduplication

Everything cooperative is an event of the journal: `session_opened`, `session_closed`,
`claim_acquired`, `claim_released`, `handover_published`, `handover_consumed`,
`review_requested`, `review_answered`, `review_withdrawn`, `session_beat` (a working session's progress and phase, left out of `mesh events` unless `--beats` asks and never a monitor signal), and a node's offer of its build
cache, `build_cache_offered` and `build_cache_withdrawn` ([Build-cache offers](#build-cache-offers)). A person or an agent reads the journal folded, not raw, through `majordomus monitor`
(`docs/MONITOR.md`, ADR 0133). It drops a board session's unchanged re-projection on each
heartbeat, marks what names the reader, and pages with the full causal key rather than the
Lamport stamp alone. An event carries its stream, a dense sequence, a
Lamport stamp, its origin's wall clock (informational), the repository identity, the origin
key, a body and an Ed25519 signature over the canonical JSON of all of it.

The guarantees, exactly:

- **Identity:** `(stream, seq)`.
- **Authenticity end to end:** a relay forwards bytes; a consumer verifies the origin's key
  and the key-to-stream binding. A relay can withhold an event; it cannot forge or alter one.
- **Delivery:** at-least-once. **Application:** idempotent — a duplicate is counted and changes
  nothing. Not exactly-once.
- **Order:** within a stream, sequence order; an early arrival waits (up to 256 per stream) for
  its gap. Across streams the fold orders by `(lamport, stream, seq)`. Arrival order never
  changes state (a property test permutes and duplicates deliveries).
- **Forward compatibility:** an event of a kind this executable does not know is stored and
  relayed, not interpreted. An event of a known kind out of bounds is refused.
- **Bounds:** 48 KiB per event, 32 KiB per handover body, 64 paths per claim, 64 streams per
  node. Streams expired longer than 15 minutes that nothing still needs are compacted away
  when the journal passes 20 000 events or 768 streams, and a node's own such streams when
  it holds 48 — every worktree of a machine signs with the machine's key and every server
  restart is a new stream, so a machine would otherwise fill its quota by running and its
  later runs would be refused. What compaction keeps, and how a compacted stream that
  returns is heard again, is [Compaction](#compaction).

## State synchronisation

Replication is a comparison of marks, not a flood. A sync request carries the dialer's marks
(per stream: high-water sequence, beat, beat age) and the events the answerer's last marks
lack; the answer carries the answerer's marks and the events the request's marks lack. One
round replicates both ways. A runtime joining late — or rejoining after a partition — gets
exactly what its marks lack in its first rounds, which after compaction is the live state plus
what compaction keeps, not an unbounded history. Once marks agree, rounds carry no events:
replication stops rather than loops.

A round's events get what is left of the 900 KiB message bound once the rest of the message —
its marks above all — is serialized, and never more than 600 KiB. The marks of a journal at its
stream bounds weigh hundreds of kilobytes, and with a fixed 600 KiB of events beside them a
message passed the bound and was refused as oversized, every round, so replication stopped with
no explanation (I2139). The size is checked on the message as signed, where it is built, by the
dialer and the answerer alike; an event that would take it over waits for the next round.

`mesh.state` folds the journal: every runtime with the same events and the same liveness
verdicts computes the same state and the same `digest`.

### Reading the state in one answer

The fold keeps its history — every session that ever opened, every claim ever taken — and on
2026-10-10, with twelve runtimes, the whole of it was 449 kB from `mesh.state` and 313 kB from
`mesh.peers`; a page of 80 journal events was 83 000 characters, 30 of them one session
reopening itself. An answer a worker's client cannot show is not an answer. So both
capabilities take one input (`apps/majordomus-cli/src/mesh/page.rs`, I2414):

<div class="overflow-x-auto" tabindex="0">

| Input | Values | What it does |
|---|---|---|
| `view` | `summary` (default), `full` | The summary counts sessions and claims by standing, gives each runtime its liveness, counts and newest live claim, and lists the open review requests and the handovers nobody has taken — at most 24 of each, the rest counted, every intent cut to 160 characters. `full` is the fold. |
| `claims` | `all` (default), `live` | Only the claims still held; their overlaps follow them. |
| `after` | a Lamport stamp | Only sessions, claims and handovers that moved after it. |
| `session` | a key or a bare id | Only what that session holds, opened or took. |
| `runtime` | `<node>-<runtime>` or a node id | Only that runtime, or every runtime of that machine. |
| `kind` | `sessions`, `claims`, `overlaps`, `handovers`, `reviews` | Only that collection; the others answer empty. |
| `limit` | a count | At most that many of each collection, the newest by Lamport stamp, in the fold's order. |
| `open` | `true` | Only what is still open: active sessions, live claims, review requests nobody has answered, handovers nobody has taken. |
| `matching` | text | Only what mentions it, ignoring case: a claimed path, a branch, an issue, a pull request, a word of an intent or a subject. |

</div>


The `digest` in every answer is the whole fold's, never a page's: it is what two runtimes
compare. `majordomus mesh state` and `mesh peers` take the same input as `--full`,
`--live-claims`, `--after`, `--session`, `--runtime`, `--kind`, `--limit`, `--open` and
`--matching`. A server older than this input
refuses it as invalid; the readers in this repository that need the fold (the integration
lease's claims, the mesh-review advisor) ask again without it, and the answer is the same.

## Claims and leases

A claim is a scope — repository-relative paths — held by a session, `exclusive` (the default)
or `advisory`. Paths meet by `peers::claims_meet`, the peer board's own predicate: `apps`
meets `apps/majordomus-cli`; `app` does not.

- A claim **holds** while it is unreleased, its session is open, its stream's beat rose
  within the expiry. That is the lease: it is renewed by the holder's heartbeat and expires on
  every runtime on its own when the holder stops beating. An exclusive claim of a session that
  ever beat is, in addition, **suspended** — still held, refusing nobody — while that
  session's newest beat is older than its ttl (the holder) or its ttl and the expiry (everyone
  else), and holds again at its next beat (I3990). There is no separate lease table.
- **Admission:** `mesh.claim` refuses an exclusive claim that meets a live exclusive claim of
  another session known to this runtime — `claim_conflict`, naming the claims it meets.
- **Concurrency:** two exclusive claims made where neither runtime could see the other (a
  partition, or the same heartbeat) both enter the journal. The fold names one winner — the
  lowest `(lamport, stream, seq)` of acquisition — on every runtime, and lists the other as
  `conflicted` with the winner's key. Never "last packet wins".
- **Release:** only the holder's current run releases a claim (`not_own` otherwise); a dead
  holder's claim expires instead.
- **Advisory** claims report overlaps and refuse nothing. The peer board's announcements are
  projected as advisory claims of the announcing MCP session.

### Seats are claims

The repository's way of working puts sessions in seats: the one that lands, the one that
releases, the lanes that repair conflicting branches, and the workers. A seat is held by
holding its exclusive claim, so it is subject to everything above: it is refused to a second
session while it is held, it moves when the claim is released, and it falls vacant when its
holder stops beating. Which seats there are and which scope each one claims is
[rule `project.the-cooperation-model-is-the-owners-word`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md)
(ADR 0167), the one record of it; this document does not copy the table. The Mesh screen of
the UI (`/ui/mesh`) asks `mesh.state` for the live claims and draws who holds each seat now.

## Handovers

`majordomus handover` writes a record under the checkout's `.ai/local/state/handovers/`.
`mesh handover publish` (newest record, or `--path` under that directory — nothing else is
ever read) turns it into a journal event: task, branch, head and time from its front matter,
the `--issue`/`--milestone` given, and the Markdown body, identified by the body's digest. No
path of the publishing machine travels. On another runtime, `mesh handover consume <id>
--session <s>` records the consumption and writes the handover as a record of the same schema
into that checkout's handovers directory — once, however often consumed — naming the local
repository and `mesh:<origin runtime>` as its worktree, so `majordomus handover --resolve` on
the same branch finds it as another checkout's handover.

## Build-cache offers

A node that serves a shared build cache (ADR 0157) says so through the journal, not through
a committed endpoint and not through the advertisement datagram, which is size-bound. The
store is declared in the node's own file, `$XDG_CONFIG_HOME/majordomus/build-cache.json`
(else `~/.config/majordomus/build-cache.json`), outside every repository: an endpoint is a
fact about a running machine.

- **Published while the store answers.** On every heartbeat, before it beats, the runtime
  probes its store's readiness route (`mesh::build_cache::probe_health`, bounded at 500 ms).
  A healthy store not offered yet, or offered differently, is written as a
  `build_cache_offered` event: protocol, provider, scope, endpoint, bucket, priority,
  headroom and a lifetime (the cooperation expiry). A store whose probe stops passing, or
  that is no longer declared, is withdrawn with `build_cache_withdrawn` and a reason from a
  fixed vocabulary. Nothing is written while nothing changed; a headroom that moves by less
  than a tenth is not a change.
- **Signed, and nothing secret.** The record is an ordinary event, signed by its node's key.
  It has no field for a credential, and its endpoint is a scheme and an address only: user
  information, a path, a query or a fragment is refused before signing, everywhere.
- **Read only from trusted keys.** A reader verifies each record's signature against its
  stream's key again and reads it only when the trust in force trusts that key; a key
  withdrawn from the allowlist stops being read at once. Presence on a port is never trust.
- **Expiring with the beat.** An offer is fresh while its stream's signed beat rose within
  the offer's lifetime (and within the reader's own expiry) on the reader's monotonic clock.
  A provider whose runtime stops is stale within its lifetime with nobody withdrawing it.
  Each stream's newest record speaks for it; two servers of one machine make one offer, the
  fresher.

`mesh state` answers the reading under `build_cache`: the offers the selection
(`mesh::build_cache::select`) takes, and every record that is not one, with why. An older
runtime stores and relays both kinds without reading them.

## Compaction

Every server restart is a new stream, so a journal that kept everything would grow without
bound and a machine would fill its 64-stream quota by running. Compaction drops the streams
nothing needs any more. Each runtime runs it on its own, every sixty heartbeats (five minutes
at the defaults), and only when its journal holds more than 20 000 events or 768 streams or
when one node holds 48 streams or 15 000 events — then that node's streams alone are due. A
node's own crowding matters because the journal-wide bound is no larger than one node's quota
of 20 000 events: a machine alone fills its quota before the journal is over its bound, and past
the quota every event of that machine is refused, a claim's release among them. A stream can go only
when it has been silent for longer than the expiry plus 15 minutes, measured on the local
clock from the last beat heard or, for a stream never heard beating, from when the journal
learned of it (so the streams a restart reloads are not taken for dead at once). A runtime
never drops its own stream. Of the streams that could go, one stays while:

- **it published a handover nobody has taken** — for at most a day past those 15 minutes
  (`HANDOVER_RETENTION`), counted on the local clock or from the handover's signed
  publication time, whichever lapses first: the local count restarts with every server, so on
  its own a machine whose servers never live a day would keep a dead stream for ever. A
  handover is continuity meant to outlive the run that wrote it;
  the day bounds how much of its node's quota a machine's untaken handovers can hold. A
  handover somebody has taken keeps nothing. A lapsed one is still its author's record, and
  `mesh handover publish` offers it again under the same id.
- **it holds the taking of a handover, or the answer to a review, that stays.** Who took a
  handover and who answered a review are events of the taker's stream. Dropping that stream
  while the handover or the review stays would make a handover still offered look untaken
  and an answered review open again. Keeping one stream can keep the stream that answered
  it, so this is applied until nothing changes.

Who took what and who answered what are read by the fold, as every surface reads them. A
review request keeps nothing on its own: it asks something of the session that made it, and
once that session's runtime is long dead and nothing else keeps its stream, the request goes
with it. So compaction forgets the ended sessions and claims, the taken handovers and the
requests of what it drops — never a claim that holds or a session that is open, never a
handover still offered, and never who took or answered what stays. A property test holds
that over random histories of dead and live runs.

A review request ends in one of three ways, and the fold says which:

- `answered`: another session gives a verdict.
- `withdrawn`: the runtime that asked ends it with `mesh review withdraw` (done, void or
  superseded). Any session of that runtime may, because an MCP session is one connection and
  an asker that reconnects is another session of the same runtime. No other runtime may. The
  note is the requester's own `result` and never an answer.
- `expired`: nobody answered and the asking runtime has stopped beating.

Expiry is a reading of liveness like a claim's, so no runtime reads another's clock. A runtime
that sleeps and beats again reads `open` again, and the asking session closing does not end a
request, since a lane request outlives many connections. A consumer that takes only `open`
requests (`ReviewState::is_waiting`), such as an executor's lane intake, therefore skips
expired ones: it should name them rather than drop them silently. A request may carry a `body` of up to 8 KiB
beside its one-line subject: the evidence, the commands and the question. A runtime older
than the field reads the request without its body, and holds `review_withdrawn` as an opaque
event it relays but does not fold.

**Tombstones.** A dropped stream leaves a tombstone: the sequence it reached, advertised as its
mark so that a peer still holding the stream does not send it again, and the beat it was last
heard at. A late copy of what was held, or an event the stream wrote after it was compacted,
changes nothing; the mark rises over a later event, so a relay that holds it stops offering it.
Tombstones live in memory: a restarted runtime has none, may be sent a compacted stream again,
and drops it again once it has been silent there as long as any stream must be. A tombstone is
told only to a peer whose marks show it holds that stream: to anyone else it says nothing, and
the four thousand a full journal may keep would only weigh down every message.

**A compacted stream that returns.** A laptop that sleeps past the expiry and the 15 minutes is
compacted away by its peers; it wakes as the same process, so as the same stream, and goes on
writing. A fresh beat from it — signed by its key, from a trusted origin, within the expiry,
and above the beat its tombstone holds — lifts the tombstone: the stream's mark falls to
nothing held, and the peers send it again from its first event. From the first event and not
from the tombstone's mark, because what the stream said before it was compacted is live again
with it: a claim it took and never released holds again the moment it beats, and a runtime
holding only the part after the mark would admit a claim that conflicts with it, and would
see releases of claims it never knew. The origin never compacts its own stream, so the whole
stream is always there to fetch; a return costs one refetch, bounded by the node's quota and
a round's event budget. A relay replaying the old beat, however fresh it says it is, lifts nothing.

**Digests while runtimes compact.** Each runtime decides on its own when compaction is due, so
two runtimes can hold different sets of dead streams for a while: one has forgotten a dead
stream's ended sessions and claims, or a taken handover, that the other still lists. The
`digest` covers those records, so `mesh state` and `mesh verify` can print different digests
on two runtimes that agree on everything live — held and conflicted claims, open sessions,
handovers still offered with their takers, answered reviews — for up to one schedule when both
are due, and for as long as one of them is not. Marks are unaffected: a tombstone advertises
the sequence its stream reached, so `mesh verify`'s `converged`, which compares marks, holds.
Compare digests between runtimes that are both past their compaction, or compare the live
claims.

## Sessions and the peer board

The peer board stays the machine-local view (ADR 0003, ADR 0044). Every heartbeat,
cooperation projects it: an attached MCP session becomes the mesh session `board-<peer>` with
its client, intent, checkout, branch and head; each announcement becomes an advisory claim;
a changed announcement releases and re-claims; a detached session is closed. An MCP call to
`majordomus_mesh_claim` without a `session` acts for the caller's own `board-<peer>` session.
Branch and head are read when a session changes, never on a heartbeat.

A provider session is on the mesh before its worker says anything (I2734). The provider's
start event opens `<provider>-<provider session id>` — `claude-code-<uuid>` for Claude Code —
on the checkout's shared server with its client, branch and head, and with the task and its
title while one is active; the briefing prints that id under the `Mesh:` line, so a worker
can claim under it (`majordomus_mesh_claim` with that `session`). A resume or a compaction
says it again; the end event closes it, which ends its claims on every linked runtime at
once. The id is the provider's own, so anything holding the provider's payload names the
same session. With no ready server, or a mesh that does not run, nothing is opened and the
event is unaffected.

A client that crashes or is killed never fires its end event, and its session would stay
active, with every exclusive claim under it blocking, for as long as its runtime beats
(I3810). So the start event opens the session with `--client-pid auto`: the executable walks
up from itself past shells and Majordomus' launchers to the client that ran the hook, and the
session carries that process's pid and start time (`client_pid`, `client_started`). About once
a minute the runtime holding the session asks its own machine's process table whether that
process still exists with that start time, and closes the session once when it does not — a pid
the system has given to another process does not count as the client. Nothing is compared
across machines: the process, the table and the clock are all the holding runtime's. A session
opened without a client process (`mesh session open` by hand, a claim's implicit session) is
left as before.

A task is on the mesh while it works (I2288). `majordomus start` claims the task's scope as
an advisory claim of the mesh session `task-<id>`, with the task's title as its intent and its
issue, and keeps the claim's key on the task record as `mesh_claim`; `finish` closes that
session, which ends its claims on every linked runtime. Every machine sees what the task works
on and reports an overlap; an exclusive claim before building stays the worker's own act. On
a repository whose mesh declaration is enabled, `start` first ensures the checkout's shared
server — a fresh linked worktree has none until something brings it up, since no provider
start event fired there — with the same bounded `serve ensure` the start event and the shell
entry run, under the same switches (`session.ensure_server_on_start`, `MAJORDOMUS_RUNTIME=off`),
and says on stderr when it starts one. When the claim still does not happen (the runtime
switched off, a stale or missing executable, a server that did not converge) `start` does not
fail: it reports `WARN mesh` naming why and the repair. With no enabled mesh, `start` claims
only through a server that already answers.

That claim is enforced (I2289, rule `majordomus.task-on-the-mesh`). On a repository whose
tracked declaration says `enabled: true`, `check` reports an active task whose record carries
no `mesh_claim` under `mesh`, and `finish --outcome completed` refuses it, the refusal recorded
with the rule named. The state is repairable without ending the task: `majordomus serve
ensure`, then `majordomus check`, which claims the running task's scope once a server answers
and records the key; `finish` claims again first as well. `finish --check`, the pre-push gate,
is a question and claims nothing. `partial`, `blocked`, `no_match` and `failed` are not
refused, and a repository with no enabled mesh is not judged.

An edit can be asked for its claim before it happens (I2735). With `session.edits_require_claim:
true` in the policy, the provider's pre-tool hook (`.claude/hooks/majordomus-edit-guard`, for
Edit, Write, MultiEdit and NotebookEdit) runs `majordomus guard hook`, which exits 2 — the
provider's refusal — when neither the active task's scope nor a live mesh claim of the writing
session (`<provider>-<session id>`, the id the start event opened) covers the path, naming the
path and the remedy: `majordomus start "<task>" --scope <path>`, or `majordomus mesh claim
<path> --session <id> --advisory`. The checkout judged is the one the file belongs to, so a
session started in one checkout editing another's file is judged by the other's task and
claims. Local state, a path outside the checkout, a repository with no enabled mesh and a
server that does not answer are let through. The same judgement is `guard.edit` over MCP and
HTTP.

The hook is not wired yet, and until it is, the policy's `edits_require_claim` refuses nothing in
Claude Code (I3010). `majordomus capture install` writes the UserPromptSubmit, SessionStart,
SessionEnd and PreCompact entries of `.claude/settings.json`, and no PreToolUse entry; the hook
script and `guard.edit` exist, but no edit reaches them. Wiring it (advisory first), and then
refusing an unclaimed edit, are the generated-plugin work that follows the alpha. Until then a
person who wants the guard wires it by hand: add this entry under `"hooks"` in
`.claude/settings.json` (shared with everyone who works in the checkout) or in
`.claude/settings.local.json` (this machine only), and start a new Claude Code session, which
reads its hooks when it starts:

```json
"PreToolUse": [ { "matcher": "Edit|Write|MultiEdit|NotebookEdit",
  "hooks": [ { "type": "command", "command": "${CLAUDE_PROJECT_DIR}/.claude/hooks/majordomus-edit-guard" } ] } ]
```

The hook asks `bin/majordomus-cli` and never builds it: where that executable is absent, every
edit is let through.

A provider session that waits on a person says so on the mesh (I3992). The hook
`.claude/hooks/majordomus-session-wait` hands each payload to `majordomus mesh beat --hook
claude-code`, which reads the session (`claude-code-<session id>`, the one the start event
opened) and the event out of it and beats for it on this checkout's ready server: a permission
dialog, an MCP server's question or an idle prompt (`Notification`) and the end of a turn
(`Stop`) mark it `blocked: …` with the reason, and its next prompt (`UserPromptSubmit`) or tool
call (`PreToolUse`) is real work, which clears the mark. Since a provider session beats on its
tool calls, an exclusive claim held under its own id lives by its progress like a board
session's (I3990). The command builds no index — it runs before every tool call, in about
twenty milliseconds — prints nothing and exits 0 whatever happens: no payload, an event that is
no beat, no executable, no server, a refusal. Like the edit guard it is not written by
`capture install` and not wired in this repository's `.claude/settings.json` yet; a person
wires it by hand, as above, and starts a new session:

```json
"Notification": [ { "matcher": "", "hooks": [ { "type": "command",
  "command": "${CLAUDE_PROJECT_DIR}/.claude/hooks/majordomus-session-wait", "timeout": 15 } ] } ],
"Stop": [ { "matcher": "", "hooks": [ { "type": "command",
  "command": "${CLAUDE_PROJECT_DIR}/.claude/hooks/majordomus-session-wait", "timeout": 15 } ] } ],
"PreToolUse": [ { "matcher": "", "hooks": [ { "type": "command",
  "command": "${CLAUDE_PROJECT_DIR}/.claude/hooks/majordomus-session-wait", "timeout": 15 } ] } ]
```

and the same handler beside `majordomus-capture` in the existing `UserPromptSubmit` group.

What a session shares is metadata — intent, issue, task, milestone, branch, head, dirtiness,
a context revision identifier. Context bodies are not replicated; a handover is the
referenced artifact a peer materialises.

## Failure and recovery

<div class="overflow-x-auto" tabindex="0">

| event | what each runtime does | what converges |
|---|---|---|
| a session's worker waits on a permission dialog, hangs or crashes while its server runs | the server keeps beating, and the bridge keeps pinging; neither is progress, so no progress beat is written — where the provider's wait hook is wired, the dialog writes a `blocked: permission dialog` mark at once, which every peer sees and which is not progress either (I3992); once its last beat is older than `session_ttl_seconds` on the holder's runtime (and the ttl and the expiry on every other), its exclusive claims read `suspended` and refuse nobody; its session and its advisory claims are untouched | the scope is admitted to the next claimant everywhere; when the worker works again its next beat makes the claim hold again — or, when another session was admitted over it meanwhile, releases it so the holder claims again through admission; no take-over and no fencing in this slice (I3990; the epoch is I3174) |
| a peer's process is killed | its links go degraded → unreachable → expired; its stream stops beating; its sessions and claims expire | on restart it is the same runtime, new instance; it re-handshakes into one table row and syncs; its previous run's claims stay expired |
| the network partitions | each side expires the other's streams and may claim what the other held | on healing, one round each way; concurrent exclusive claims resolve to one named winner; digests agree |
| a peer restarts while linked | the answerer refuses the old link id as `unknown_link`; the dialer says hello again | the table row is kept, `restarts` counts |
| one machine's dead streams fill its node's event quota | every later event of that machine is refused as `capacity` — claims, releases, closes; `mesh verify`'s `rejections` check fails naming `capacity`; the node is due for compaction at three quarters of its quota, and a dead stream's untaken handover lapses by its signed publication time, so a restart does not renew it | once the dead streams go, the refused events are offered again and accepted; the release arrives and the claim reads released everywhere (I2825) |
| two processes speak for one checkout (a server older than v0.27.0 that never left the mesh when another took its lease) | every peer that hears both replaces one link with the other each round, and `unknown_link: … previous instance` repeats; `mesh verify`'s `instances` check fails naming both instances | stopping the process the lease does not name ends the flapping; the lease's holder is the checkout's one runtime |
| a relay dies in a line A–B–C | A and C expire each other's streams through lost beats | when B returns, marks re-align and nothing is duplicated |
| hostile or broken input | typed refusal, counted, listed; the runtime keeps serving | nothing enters the journal |
| a crash tears the journal file, or the disk refuses an append | an append writes a missing newline first and syncs; a failed append is counted (`write_failures`) and its operation answers `"durability": "not_durable"` — never acknowledged as durable, and `mesh claim`, `mesh session open` and the other writes print `NOT DURABLE`; a line that does not read on reload is counted (`unreadable`), and `mesh doctor` fails its `journal` check; compaction writes a synced file and renames it over the old one; the file is the owner's alone (0600) | every complete event reloads, the event after a torn line reads, and an event that did not reach the disk is still replicated from memory and from peers |
| a journal file larger than the reload bound (64 MiB) | a reload reads up to the bound, to the last whole line, and leaves the rest on disk: counted (`unread_bytes`), named by `mesh doctor`'s `journal` check, and copied byte for byte into the file compaction writes | the events within the bound reload; the rest is neither folded nor relayed by this runtime until the file is moved aside and peers refill it |

</div>


An operation's answer says what reached the disk: `durability` is `synced` when the event was
appended to the journal file and its data synced before the answer, `memory` when the journal
has no file by configuration, and `not_durable` when the write failed. "Synced" is the
operating system's data sync of the journal file (`fdatasync` on Linux, `fsync` on macOS, which
does not flush the drive's own cache), not a promise about the storage under it.

## Security model

What the mesh defends against, and how:

<div class="overflow-x-auto" tabindex="0">

| threat | answer |
|---|---|
| forged advertisement, hello, welcome, sync or event | Ed25519 over canonical bytes under per-message domains; the node id is derived from the key, never carried |
| replayed hello or sync round | nonce cache; per-link rising counter; per-instance sequence for advertisements and events; a hello that names its recipient is refused `misaddressed` by every other runtime, and a hello older than the one that made the peer's live link cannot replace it, even after a restart emptied the cache (I2132) |
| a runtime of another repository | `repository_mismatch` at the handshake, `repository` rejection at ingest |
| an unknown or untrusted key on the network | observed by discovery, refused `untrusted` at the handshake; its relayed events refused `untrusted` at ingest |
| a key withdrawn from `trust.allow`, then a restart | the journal file is input: a reloaded event passes the checks and the trust decision a received one does, and what is refused is reported by reason (`untrusted`); only this machine's own key needs no allowing |
| a relay that alters or invents events | the origin's signature fails at every consumer |
| a trusted key stamping events at the end of the Lamport range | an event more than 2^32 ahead of the receiver's clock is refused `clock_ahead` (`MAX_LAMPORT_LEAD`), so the clock rises at most that much per stored event. A stamp set low to win a claim conflict is not refused: Lamport stamps are their writer's own assertion |
| flooding | bounded datagrams and a bounded queue of them to the registry (1024; a flood beyond it is dropped and counted as `overflow` in `mesh.status`), messages (900 KiB), events, streams (1024, at most 64 per node), events per node (20 000), pending events (256 per stream, 4096 in all), peers (256), dial targets (8 per node, the present runtimes before the stopped ones), listed refusals (128), registry (256; only allowlisted records are never evicted) |
| a peer or a rendezvous that answers too much, or too slowly | the dialer holds a link reply at most at one 900 KiB message in its wrapper (`MAX_LINK_REPLY`) and a register answer at most at 33 envelopes (`MAX_ANSWER_CANDIDATES`) and 81 KiB (`MAX_REGISTER_ANSWER`), counted without the whitespace the server pretty-prints, and refuses what is over before parsing it; a reply not whole within 20 s (link) or 15 s (rendezvous) is a failure however steadily it drips (I2704) |
| a forged or replayed liveness report | beats are signed by their origin and only a higher signed beat counts; a relayed age is clamped to the expiry; a stream is created from a mark only when the mark verifies, its origin is trusted and its beat is fresh |
| a branch or an uncommitted edit that adds a key, a hub, a seed or `tofu` | the declaration in force is the working tree's with every widening of the trunk's committed copy removed; `mesh doctor` names each ignored line (`trust-root`), and `mesh-check` refuses a committed `tofu` (I2135) |
| a hostile handover consumed here | every front-matter field is single-line at ingest; the record's file name keeps only timestamp digits, hex and `[A-Za-z0-9_-]`, and a path outside the handovers directory is refused |
| a host on the network posting claims, sessions, handovers or reviews for this node to sign | a request from an address that is not loopback runs only a capability that reads or whose input is a signed message (`mesh.link.hello`, `mesh.link.sync`, `mesh.register`); every other route answers 403 `forbidden` before its handler runs (ADR 0126) |
| a web page driving the server (DNS rebinding) | a state-changing request from a browser is accepted only from the server's own origin and only when addressed by an IP literal or `localhost` |
| a registration that maps the mesh | a refused registration is answered with the refusal alone; an admitted one learns the hub and, only when its own key is trusted, the trusted nodes of its own repositories, never observed strangers or another repository (I2133, I3826) |
| a remote reader of the mesh's working state | the capabilities that show it (`mesh.status`, `mesh.state`, `mesh.peers`, `mesh.peer`, `mesh.nodes`, `mesh.events`, `mesh.cooperation`, `mesh.doctor`, `mesh.firewall`, `peers.list`, `monitor.stream`) are declared local-only and answer 403 `forbidden` to a caller beyond loopback, and so do `/ui/mesh` and `/cockpit/{mesh,peers,monitor}`; liveness, the repository's reads, `mesh.identity` and the signed-input routes (`mesh.register`, `mesh.link.*`) stay open (ADR 0220 decision 4, I3826) |
| a remote reader of what this process runs | the live channel's upgrade (`/events`), `executions.list`, `executions.get`, `executions.events`, `executions.demonstrate` and `/cockpit/executions` answer a caller beyond loopback 403 `forbidden`: an execution of a local-only read must not leak through the channel that reports it (I3842) |
| a stranger's hellos | the replay cache and the refusal list hold only what a trusted key of this repository sent, or are capped |
| a stale peer holding a scope forever | claims live only while the holder's beat rises |
| publishing arbitrary files | handover publication reads only `.ai/local/state/handovers/` |
| a future protocol | `protocol_unsupported` with both ranges, not a dropped connection |

</div>


What it does not defend against: an observer on the network sees that a Majordomus runs, its
advertisements and — for linked peers — session intents, claim scopes, review subjects and
published handover bodies, because nothing but a request to an `https://` hub or seed is
encrypted; run it on a private network or an overlay. A holder of an allowed node key is that
node. A compromised trusted peer can publish misleading claims and handovers under its own
identity — they are attributed to it and expire
with it, but they are believed until then. The default — no declaration, or `enabled: false` —
exists for environments where presence disclosure is unacceptable.

## Configuration

```yaml
# .ai/repo/mesh/majordomus.yaml
schema: mesh/v1
kind: mesh-declaration
id: majordomus
enabled: true                 # nothing opens without it
multicast: { enabled: true, group: 239.255.77.77, port: 7741, ttl: 1, interval_seconds: 15 }
broadcast: { mode: disabled }
rendezvous: { endpoints: [], interval_seconds: 60 }
trust:
  policy: deny_unknown
  allow:                      # `majordomus mesh identity` on each machine
    - 5b0f…e1
    - 9a41…07
cooperation:
  enabled: true               # default once the mesh is enabled
  heartbeat_seconds: 5
  expiry_seconds: 30          # at least three heartbeats
  seeds: [http://10.0.0.12:8741]   # dial these whether or not discovery hears them
  # repository: my-project    # required for a shallow clone; separates a fork
```

The declaration is the one place mesh behaviour is configured; there are no environment
variables for it. To be dialed from other machines, the server must listen beyond loopback:
`majordomus serve --host 0.0.0.0` (or a deployment object). A server on loopback can still dial
out, and a link, once dialed, replicates both ways.

`--host` lasts as long as that one process. For a machine that should always be dialable,
export `MAJORDOMUS_HTTP_HOST=0.0.0.0` in the shell's startup file: it is not mesh
configuration — it names the interface every local server of this machine binds, whoever
starts it — and [`MCP.md`](@/docs/mcp.md#the-machine-names-the-interface) says what it
exposes.

A linked worktree does not take that variable: its own server stays on loopback unless
`--host` says otherwise, because the machine's face on the network is the primary checkout's
server. A worktree needs no server of its own for the mesh either. `majordomus mesh` in a
checkout with no running server asks a ready server of the same repository — the primary's
first — and names it on stderr (`asking the server of <checkout> at <url>`). The one command
it refuses there is `mesh handover consume` without `--no-materialize`, because the handover
would be written into the other checkout.

## This repository's mesh

This repository runs its own mesh: `.ai/repo/mesh/majordomus.yaml` is committed with
`enabled: true`, so every server of every worktree of it announces, listens and links. That is
this repository's reviewed decision about who is listed. The tool's default is the same shape
with an empty trust list: `majordomus init` seeds `mesh.yaml` in `.ai/repo/mesh/`, enabled and
`deny_unknown`, so nothing opens until a machine runs `majordomus mesh enrol` and the change
is committed (ADR 0137; the guarantee `mesh-off-by-default`).

<div class="overflow-x-auto" tabindex="0">

| | declared |
|---|---|
| multicast | `239.255.77.77:7741`, TTL 1 — the local segment only; the sockets are bound with `SO_REUSEADDR` and `SO_REUSEPORT`, so every server on a machine hears the group |
| broadcast | disabled |
| rendezvous hubs | jetson (`192.168.100.30`, tailnet `100.92.246.32`), lundra (`192.168.100.10`, tailnet `100.65.22.118`) and the owner's two MacBook Pros (tailnet `100.120.245.51` and `100.108.237.68`), port 8791, every 30 s |
| seeds | none |
| cooperation | the defaults: heartbeat 5 s, expiry 30 s |
| trust | `deny_unknown` with five keys: `641bdb94` (the owner's MacBook Pro), `5d81b5c9` (the owner's second MacBook Pro), `aaba18ea` (the owner's iMac), `9d652b2c` (jetson), `25c9758f` (lundra) |

</div>


**Who is trusted.** The owner's five machines and nothing else. The worktrees of one machine
share that machine's key and link as itself; two machines link only when both keys are listed,
which is why the MacBook's own key is on the list. Any other key on the segment is observed,
trusted for nothing, and refused `untrusted` if it dials.

**Where the hubs are.** A hub is a server of this repository on jetson, lundra or one of the
owner's two MacBook Pros, listening beyond loopback on port 8791. jetson and lundra are listed
by their LAN and their tailnet address; a MacBook by its tailnet address alone, because its LAN
address is a DHCP lease that moves — the first MacBook was declared at `192.168.100.93` and the
router has since handed it `.91`, so for a while no Mac reached it. Multicast cannot cross
the tailnet, and the macOS firewall drops it inbound, so the hubs are how machines on different
segments find each other. Every server registers with every hub it can reach, one thread per
hub with a 5-second bound per request. A hub that does not answer is asked less and less often
— the 30-second interval doubles per failure, up to eight times — and delays nothing else: a
server whose six hub addresses are all unreachable starts, discovers and links over multicast, and
answers requests, exactly as one with none (`mesh status` shows the `rendezvous` provider
`running` with `sent 0`).

**What travels.** What the security model above says: advertisements on the local segment and
to the hubs; to linked runtimes of the owner's machines, session metadata, claim scopes, review
subjects and published handover bodies. Nothing is encrypted but a request to an `https://`
hub; every hub address is on the private network or the tailnet, the client refuses cleartext
to a public one, and `test/cases/491_the_mesh_is_on_here.sh` refuses a declaration that names
a public one.

**Adding a machine.** Run `majordomus mesh identity` there, add the key under `trust.allow` with
a comment naming the machine and its node id to the trust row of the table above, and commit
both; `majordomus mesh doctor` on that machine passes its
`trust` check once the key is listed, and names the remedy until then. A new hub is a server
started with `majordomus serve --host 0.0.0.0 --port 8791` on the machine, and its addresses
added under `rendezvous.endpoints`; then `sudo majordomus mesh firewall apply` there, because a
hub whose host firewall drops port 8791 is a hub nobody can register with
([The host firewall](#the-host-firewall)).

**The hubs as services.** On lundra the hub is a systemd user unit,
`~/.config/systemd/user/majordomus.service`, running the release executable of a dedicated
checkout (`~/dev/prismatic-majordomus-hub`, detached at the trunk) with `serve --host 0.0.0.0
--port 8791 --idle 0`, `Restart=always`, and the share of that checkout; updating it is a
fetch and checkout of the trunk there, `just build-release`, and `systemctl --user restart
majordomus`. On the owner's MacBook Pro the hub runs the installed release from a dedicated
worktree (`~/dev/prismatic-majordomus-hub`, detached at the trunk): `majordomus serve --repo
~/dev/prismatic-majordomus-hub --host 0.0.0.0 --port 8791 --idle 0`, started by hand, because
the checkout a person works in already runs its own shared server and a second one is refused.
The launchd agent of the same shape — `~/Library/LaunchAgents/dev.majordomus.hub.plist` with
`RunAtLoad`, `KeepAlive` and those arguments — is the operator's to install, and an installer
that writes either unit is still owed as a typed capability (ADR 0059).

**Hubs follow the release (ADR 0161).** A hub is a node like any other: with `follow: stable`
it keeps its installed Majordomus at the version `releases/latest.json` names, restarts onto it
under its service manager when no session is attached or after a bounded grace, checks its
readiness and a live `mesh verify` round, and rolls back to the previous version by itself when
either fails, naming node, version and failed check on the board. A typed capability writes
the systemd unit and the launchd agent (I2722). Until that lands, the release seat upgrades the
hubs by hand after each release and reads each version back.

**Turning it off.** Commit `enabled: false`; a server started after that opens nothing,
`mesh status` says why, and the briefing says `Mesh: off, as declared`. That reverses ADR 0059,
so it is a decision of its own: case 494 refuses a disabled declaration until the decision and
the case change with it.

**Whether the server holds it.** The declaration says what should run; the shared server
decides what does, and the two can drift — on 2026-09-12 three machines ran an uncommitted
`enabled: true` while the committed file said `false`, and the one reset to it was silently
alone (ADR 0059). So `mesh doctor` ends with a `runtime` check that is the server's verdict:

<div class="overflow-x-auto" tabindex="0">

| `runtime` says | when | verdict |
|---|---|---|
| `active as <node> since …: n of m provider(s) running; …` | the server activated the mesh | holds; fails only when every declared transport failed |
| `off, as declared — …` | the declaration is disabled and the server left the mesh off | holds |
| `the declaration is enabled and this server's mesh is not active — <reason>` | the server could not, or did not, activate an enabled declaration | **fails**, with the impact and the restart that follows fixing the reason |
| `not decided in this process: …` | no server runs in the process asked | holds: an absence, not a verdict |

</div>


`majordomus mesh doctor` asks this checkout's running server for its report when one serves
it — the verdict is the server's — and runs in its own process when none does; it exits 10 on
any failed check. `majordomus run mesh.doctor` is the self-check of the invoking process alone,
under its own state directory, and never asks a server.

**The session start says it.** The briefing the provider's start event injects carries one line
under the server line whenever the repository tracks a declaration and the server is ready:

```
Shared server: ready http://127.0.0.1:8741 pid 123 version 0.10.0
Mesh: active — 2 of 2 provider(s) running; 2 node(s) known, 2 trusted, 2 present
Mesh: off, as declared
Mesh: DECLARED ENABLED BUT NOT ACTIVE — identity: …/majordomus/node.json: Is a directory (os error 21)
```

(one of the three). A tracked declaration the index cannot read is
`Mesh: DECLARATION NOT READ — …`. The line is the executable's `mesh doctor` rendered by the
hook library, which sends no request of its own (SECURITY.md); with no ready server there is no
line, and the server line above says why.

**What holds it.** `test/cases/491_the_mesh_is_on_here.sh`: the committed declaration is the
reviewed one as the executable reads it, the self-check holds over this repository, a machine
whose key is not listed keeps the mesh closed and dials nobody, two worktrees under one listed
key link, and a machine whose key is missing is told so. `test/cases/494_the_mesh_is_declared_and_held.sh`: the declaration is
tracked, enabled, `deny_unknown`, allowlists at least as many keys as the trust row above names
machines, and names a hub — so adding a machine is a key under `trust.allow` and a node id in
that row, in one commit — and a disposable repository proves every briefing answer and exit 10
through the start event. The hubs are started by hand; an installer that runs one as a service
is owed as a typed capability or a Rhai workflow (ADR 0059).

## The sccache client of a node

`majordomus mesh build-cache env` prints the environment of the process that starts this
machine's sccache server: the local disk as the first level and the store this node selected
(ADR 0157) as the second. It reads this machine alone, never a repository, so a compile in any
directory can ask it; `--format json` and the tool `majordomus_mesh_build_cache_env` give the
whole answer with its reason. Nothing is written: `~/.cargo/config.toml` and the shell profile
stay the user's.

The second level is a start-up decision, because sccache fixes its chain when its server starts
and fails the compile when an S3 level does not answer then (a store that stops while the server
runs is only a miss). The store is added only when all of these hold at that moment, and the
answer names the one that did not when it is left out:

1. `sccache --version` answers;
2. the node file `~/.config/majordomus/build-cache.json` (outside the repository) offers or
   knows a store and selection chooses it, which needs the store to answer its readiness route
   within 300 ms (an unknown is never healthy);
3. the credential file `~/.config/mesh-build-cache/credentials` is a regular file of mode 0600
   holding the profile `mesh-build-cache`;
4. the installed sccache has the multi-level chain, measured by starting a throwaway server on a
   private port, in a private home, with a chain level that does not exist: an sccache that knows
   the chain refuses it by name, one that does not starts and is stopped again.

The variables are `SCCACHE_MULTILEVEL_CHAIN=disk,s3`, `SCCACHE_MULTILEVEL_WRITE_ERROR_POLICY=l0`,
`SCCACHE_BUCKET`, `SCCACHE_ENDPOINT`, `SCCACHE_REGION`, `SCCACHE_S3_USE_SSL`,
`SCCACHE_S3_KEY_PREFIX=rust/<rustc host triple>/`, `AWS_SHARED_CREDENTIALS_FILE=<path>` and
`AWS_PROFILE=mesh-build-cache`. A key is never exported: the environment of every compiler and
build script would carry it, so only the credential file's path is named.

`share/libexec/sccache-mesh` is the `RUSTC_WRAPPER` that uses it: when no sccache server listens
on `SCCACHE_SERVER_PORT` (default 4226) it evaluates the script, and then runs sccache with its
arguments unchanged. A running server ignores the environment, so nothing is asked then.
`SCCACHE_MESH_OFF=1` keeps a build local-only; a missing `majordomus` changes nothing.

What a second node can reuse is bounded by sccache's own key, measured on 2026-10-22 with sccache
0.17.0: an entry is keyed by the absolute path of the cargo target directory, and a workspace
crate also by the path of its checkout (`CARGO_MANIFEST_DIR`). Two compiles of one crate in one
checkout with two different target directories both miss; two checkouts that use the same target
directory path hit for registry crates and miss for the workspace's own crates. So reuse between
nodes holds for registry dependencies when their target directory has the same path (not measured here
for ADR 0139's shared build directory) and for workspace crates only at the same checkout path; `--remap-path-prefix` does not change that.

## The host firewall

The declaration says who may link; the host firewall says what reaches the socket at all, and
the two drift silently. On 2026-10-08 lundra, a declared hub, ran ufw with default deny and no
rule for port 8791 or the multicast group: the kernel dropped every registration and every
advertisement from the LAN (187 `[UFW BLOCK]` lines in a week, 109 in the last hour) while
`mesh doctor` reported every check holding — because every check it ran was a fact of the
process, and none was a fact of the host. So the admission is a derived requirement with a
verdict, `mesh.firewall` (`apps/majordomus-cli/src/mesh/firewall.rs`):

<div class="overflow-x-auto" tabindex="0">

| | derived from | rule |
|---|---|---|
| multicast | `multicast.enabled`, `.group`, `.port` | UDP to the group's port, from any source — the TTL keeps it on the segment |
| broadcast | `broadcast.mode`, `.port`, `.networks` | UDP to the port, from the declared networks |
| hub | every `rendezvous.endpoints` entry whose address is one of this machine's | TCP to that port, from the fleet's networks |
| link | `--port`, when this checkout's server listens beyond loopback | TCP to that port, from the fleet's networks |

</div>


The fleet's networks are the enclosing private ranges of every address the declaration names
(hubs and seeds): `192.168.x.0/24`, `10.0.0.0/8`, `172.16.0.0/12`, `100.64.0.0/10` for a tailnet
— and this machine's own ranges when it names none. A public address derives no network, and
case 491 refuses one in the declaration first. With no network to admit from, no rule is
planned for a TCP or broadcast port: only the multicast group is ever admitted from any
source, and opening a port to every address stays the operator's act. A machine that is no
hub, with multicast on, needs exactly one rule: the group's port.

`majordomus mesh firewall` prints the plan, the backend (`ufw`, `nftables`, the macOS
application firewall, or `none`), the commands that admit the plan there, the firewall's own
word about it — `present`, `missing`, `inactive`, or `unobservable` without root — and what
the kernel logged it dropping toward those ports in the last five minutes, which is readable
without root on a machine whose user is in `adm`; it exits 10 when a rule is observed missing
or a drop was logged. `sudo majordomus mesh firewall apply` runs those commands and asks the
firewall again, so the verdict afterwards is the firewall's; it refuses, running nothing,
without root, without a backend, or when the firewall is inactive, because nothing filters
then and no rule is owed. Each command it ran is reported with its own exit, ok or not, and
the tool's words beside it. On nftables the rules are added to the `input` chain of the `inet
filter` table, which the plan presupposes and does not create: a host that filters in another
table gets nft's refusal for each command and the verdict `missing`. Every rule it writes
carries the comment `majordomus mesh: …`, so it can be told from an operator's own and
removed as a set. It is offered on the
command line only: it runs a privileged host tool, which nothing reachable over HTTP or MCP
may do. The doctor's `firewall` check, between the process's checks and the server's
`runtime` verdict, is the same report as one line.

```
$ majordomus mesh firewall
backend     ufw (linux)
plan        2 rule(s) from 100.64.0.0/10, 192.168.100.0/24
  multicast  udp/7741 to 239.255.77.77 from any — …
  hub        tcp/8791 from 100.64.0.0/10, 192.168.100.0/24 — …
observation unobservable: ufw status needs root; run `sudo majordomus mesh firewall` to observe the rules
blocked     7741: 81, 8791: 28 (kernel log, last 300 s)
commands    what `sudo majordomus mesh firewall apply` runs:
  ufw allow in proto udp to 239.255.77.77 port 7741 comment 'majordomus mesh: multicast discovery'
  ufw allow in proto tcp from 100.64.0.0/10 to any port 8791 comment 'majordomus mesh: rendezvous hub'
  ufw allow in proto tcp from 192.168.100.0/24 to any port 8791 comment 'majordomus mesh: rendezvous hub'
verdict     the kernel logged 109 inbound datagram(s) or connection(s) toward the mesh's ports dropped in the last 5 minutes, and the firewall could not be asked without root: run `sudo majordomus mesh firewall apply`
```

**What it is not.** A firewall manager: it admits the mesh's ports from the fleet's private
networks and touches no other rule, never widens a source to a public range, and never
removes anything. On macOS the application firewall admits an executable, not a port, so the
plan there is the server's executable allowed inbound, and a machine whose application
firewall is off needs nothing. Where no backend is found nothing is observed and the check
holds as an absence, like `runtime` in a process no server runs in.

**Recorded, 2026-10-08.** On lundra the report above was taken as the owner, then as root
(`missing`, both rules, 77 and 28 drops in the window), then `sudo majordomus mesh firewall
apply` added three rules (`Rule added`, `Rule added`, `Rule updated` — the LAN hub rule had
been added by hand a quarter of an hour earlier), after which the report as root read
`present` with no drop in the window, and the hub's `mesh nodes`, which had listed nobody, listed both Macs of the LAN as present and trusted, each heard by multicast and registered by rendezvous.

## Operating it

```sh
majordomus mesh doctor            # prerequisites, the host firewall, and the server's runtime verdict; exits 10 on a failed check
majordomus mesh firewall          # what the host firewall must admit, whether it does, and the commands; exits 10 when it does not
sudo majordomus mesh firewall apply   # admit it; needs root, refuses without
majordomus mesh identity          # this machine's key, for trust.allow
majordomus mesh enrol             # list this machine's key in trust.allow, once; then commit
majordomus mesh status            # discovery: providers, tallies, refusals
majordomus mesh nodes             # discovery: one row per node × runtime
majordomus mesh peers             # machines → runtimes → active sessions, each link's state; --full for all
majordomus mesh peer <runtime>    # one runtime; exits 10 when unknown
majordomus mesh state             # counts, each runtime's newest claim, open reviews, untaken handovers, digest
majordomus mesh state --full --live-claims   # every held claim; --after, --session, --runtime narrow
majordomus mesh events            # the journal from the beginning; --cursor <next> resumes; --beats includes the session beats
majordomus mesh verify            # a live round with every dialed peer, convergence; exits 10 on failure
majordomus mesh session open --session s1 --client codex --issue '#184'
majordomus mesh claim apps/majordomus-cli --session s1 --issue '#184'   # exits 10 on claim_conflict
majordomus mesh release <claim-key>
majordomus mesh beat --session lander --phase waiting-on-ci   # keep lander's exclusive claims excluding while it waits
majordomus mesh handover publish --issue '#184'
majordomus mesh handover consume <id> --session s2
majordomus mesh review request feature/x --session s1 --reviewer <runtime>
majordomus mesh review answer <request-key> --session s2 --verdict approved
majordomus mesh review withdraw <request-key> --session s1 --note 'superseded by the rebuilt head'
```

Every command has `--format json`, which prints the capability's answer unchanged.

**Paging the raw journal.** `mesh events` walks the journal by arrival. Each answer carries a
`cursor`, `<instance>:<n>`: the run of the runtime that answered it and the arrival number
of the last event it returned. The CLI prints it as `next --cursor …`. Passing it back as
`cursor` returns what that run applied since then. A walk resumed from every answered cursor
returns each event of the journal exactly once. That includes an event from a runtime that
was partitioned away and kept stamping with its own Lamport clock: when the link returns,
its event arrives stamped at or below a position the walk already passed, and is still
returned once, however often it is delivered. Events are numbered when they are applied to
their stream, never when received, and a duplicate is never applied twice. The test is
`events_resume_across_partition_test` (`cargo test --lib mesh::journal`).

A cursor is ~20 bytes whatever the journal holds, but it names positions of one run only. A
restarted server refuses an older run's cursor as `stale_cursor`, and the reader walks again
from the beginning. A stream that compaction drops leaves its numbers unanswered, never a
short page. `after=<lamport>` is still accepted and walks by stamp, which never returns an
event stamped at or below it that arrives later; `newest` answers the journal's last events
and the cursor of everything held, from which a reader follows what comes next.

**HTTP and OpenAPI.** `GET /api/v1/mesh`, `/nodes`, `/identity`, `/doctor`, `/firewall?port=`, `/cooperation`,
`/peers`, `/peer?runtime=`, `/state`, `/events?cursor=&after=&limit=&newest=`; `POST /api/v1/mesh/register`,
`/verify`, `/sessions`, `/sessions/close`, `/claims`, `/claims/release`, `/handovers`,
`/handovers/consume`, `/reviews`, `/reviews/answer`, `/reviews/withdraw`, `/link/hello`, `/link/sync`. The schemas
are in `/openapi.json`, generated from the capability declarations.

**MCP.** `majordomus_mesh`, `_nodes`, `_identity`, `_doctor`, `_firewall`, `_register`, `_cooperation`, `_peers`,
`_peer`, `_state`, `_events`, `_verify`, `_session_open`, `_session_close`, `_claim`,
`_release`, `_handover_publish`, `_handover_consume`, `_review_request`, `_review_answer`,
`_review_withdraw`. An
agent claims work with `majordomus_mesh_claim` instead of inventing a coordination channel;
without a `session` the call acts for the agent's own board session.

**Cockpit.** `/cockpit/mesh`: this node, cooperation (runtime, repository identity, endpoints,
heartbeat and expiry, protocol, counters, digest), machines → runtimes (liveness badge, "last
heartbeat Ns ago", link state, work counts) → sessions (intent, issue, branch, claims), claim
conflicts, handovers, reviews, refused candidates, discovery providers and nodes. Actions run
the same capabilities from their Capabilities pages.

## Troubleshooting

1. **`mesh doctor`** on each machine. A failed `declaration`, `identity`, `trust`,
   `repository`, `udp` or `multicast` check names its impact and remedy; a failed `firewall`
   check is the host's: a rule the mesh needs is observed missing, or the kernel logged the
   firewall dropping toward the mesh's ports — `sudo majordomus mesh firewall apply`; a failed
   `runtime` check is the server's: the declaration is enabled and its mesh is not active, and
   why. A hub every other machine registers with and that lists nobody is the firewall case
   until `mesh firewall` says otherwise.
2. **`mesh status`** — discovery. No nodes: multicast is not routed between the machines;
   declare `cooperation.seeds` or a rendezvous. Rising `signature` refusals: a broken or
   hostile sender. `version` refusals: a peer runs an executable older than discovery
   protocol 2.
3. **`mesh peers`** / **`mesh.cooperation`** — links and refusals. `repository_mismatch`: the
   runtimes serve different repositories, or one is a shallow clone or fork with a different
   identity — compare `mesh doctor`'s repository line on both. `untrusted`: add the key to
   `trust.allow`. `protocol_unsupported`: upgrade the older executable. A link stuck in
   `connecting` with a `last_error` of "connection refused": the peer listens on loopback.
4. **`mesh verify`** — a live round with every dialed peer, and whether both hold the same
   marks afterwards. `NOT converged` right after a burst of events is one round of lag; a
   persistent one with `journal pending > 0` is a peer relaying a partial stream. Two
   runtimes printing different digests while converged is compaction on different schedules
   ([Compaction](#compaction)), not a divergence of anything live.
5. Server logs carry `runtime_id`, `peer`, `link_id`, `session_id`, `repository_id` and the
   refusal `code` on every link event.

## Testing and evidence

<div class="overflow-x-auto" tabindex="0">

| level | what | where | runs |
|---|---|---|---|
| unit & property | journal dedup, gaps, hostile events, beats; compaction changes nothing live, bounds a handover's hold and hears a returning stream again; fold order/duplicate independence and exclusivity; handshake refusals; three-runtime relay; partition reconciliation; board projection; handover materialisation; repository identity; the firewall plan from a declaration, its rendering per backend, and the judgement of a firewall's own output | `cargo test --lib mesh` | CI `rust` |
| the host firewall | the committed declaration derives the group's port; the doctor carries `firewall`; no declaration needs nothing; a declaration naming this machine as a hub derives the hub rule from its private range alone; `apply` refuses without root, running nothing; the MCP tool answers | `test/run.sh 495_the_firewall_admits_the_mesh` | CI `suite` |
| two runtimes in process | the real link protocol over an in-process transport: a runtime compacted while it slept is heard again whole; takers and answers outlive their runs; a taken handover keeps its stopped publisher nowhere | `cargo test --test mesh_compaction`, `test/run.sh 493_mesh_compaction_keeps_what_is_live` | CI `rust`, `suite` |
| multi-process integration | separate `majordomus serve` processes with separate keys over TCP: two-runtime cooperation (sessions, claims, conflicts, reviews, handover, CLI parity, verify), repository isolation, untrusted key, SIGKILL expiry and restart, three runtimes in a line, hostile messages over HTTP, two worktrees under one key | `cargo test --test mesh_cooperation` | CI `rust`, `macos` |
| network lab | four Linux containers on one docker bridge: multicast discovery with no seeds, full-mesh handshake, repository isolation, cross-node claim and conflict, cross-node handover, three-node convergence, network partition and healing, process crash and restart, `mesh verify` in a node | `test/mesh-lab/run` → `target/mesh-lab/evidence.json` | CI `mesh-lab` |
| physical machines | the procedure below | manual | recorded when run |

</div>


### Physical multi-machine verification

On two machines A and B on one network, each with a checkout of the same repository built
from the same commit:

```sh
# on each machine
majordomus mesh identity                      # note the public key
# in the repository, on both: enabled: true, trust.allow: [keyA, keyB],
# cooperation.seeds on B: [http://<A's address>:8741]
majordomus serve --host 0.0.0.0 --port 8741 --idle 0 &
majordomus mesh doctor
# A and B
majordomus mesh peers                         # each lists the other: connected, trusted
majordomus mesh claim apps --session a1       # on A
majordomus mesh claim apps/x --session b1     # on B: exits 10, claim_conflict naming A's claim
majordomus handover --derive | …              # on A, then:
majordomus mesh handover publish              # on A
majordomus mesh state                         # on B: the handover is listed
majordomus mesh handover consume <id> --session b1   # on B: a local record
majordomus handover --resolve                 # on B, on the handover's branch
# failure: stop A's server with kill -9, watch B's `mesh peers` go unreachable → expired and
# A's claim expire; start A again and watch it reconnect with restarts: 1
majordomus mesh verify                        # on both: ok, converged
```

**Recorded run, 2026-09-15.** A MacBook Pro (Darwin arm64, node `9ce70581…`) and `lundra`
(Linux x86_64, node `94713da7…`) on one /24 LAN, each with its own checkout, its own state
directory and `cooperation.repository` declared, allowlisting each other's key, seeds both
ways, heartbeat 2 s, expiry 10 s. Measured: the Mac heard lundra by UDP multicast on the
first poll; lundra dialed the Mac and the link was connected (round trip 11 ms); a claim
written on the Mac was held on lundra 500 ms later and lundra's overlapping exclusive claim
was refused with 422; a handover published on lundra was consumed into the Mac's checkout;
both reported one digest; lundra's server was killed with `kill -9` and the Mac expired the
link after 10 s with lundra's claim `expired: its runtime stopped beating`, after which the
Mac could claim that scope; lundra restarted as the same runtime and reconnected, listed once
with `restarts: 1`; `mesh verify` passed on both (lundra: round trip 9 ms, converged). An
earlier attempt the same evening failed because the procedure's stop step orphaned the Mac's
first server (`$!` named a subshell), which kept an empty allowlist and refused every link as
`untrusted` — the refusal was visible on both sides, and `mesh verify` now fails when a runtime
is refused as untrusted, protocol-incompatible or mis-signed, and when the declaration names
seeds and no peer is connected, instead of reporting `ok`.

## Guarantees, and what is not guaranteed

Guaranteed and tested: authenticated links with typed refusals; repository isolation;
cross-runtime exclusive claims with deterministic conflict resolution; liveness and expiry
without clock agreement; reconnection without duplicate peers; convergence through relays with
each event held once; at-least-once delivery with idempotent application; handovers and reviews
across runtimes; compaction that changes nothing live and hears a returning stream again; one
state across every surface.

Not guaranteed: exactly-once delivery; strong consistency or linearisable claims; availability of
a claim during a partition beyond each side's view; confidentiality on the wire; WAN operation,
NAT traversal or routed multicast; compatibility with executables older than discovery protocol 2
and link protocol 1; replication of anything but session metadata, claims, handovers and reviews.

## Extending it

A new discovery mechanism implements `mesh::provider::MeshProvider` and needs no other change.
A new transport implements `mesh::link::LinkTransport` inside `src/mesh/`. A new kind of
cooperative event is a variant of `mesh::journal::EventBody`, a case of `mesh::state::fold`
and a capability of the mesh module; older runtimes relay it untouched. A change to the link
messages raises `LINK_PROTOCOL_MAX` and keeps the previous version readable until the fleet has
moved, or it is a deliberate break recorded in an ADR.
{% endraw %}
