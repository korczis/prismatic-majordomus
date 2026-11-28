# Majordomus Fabric: the local contract

What a client and the opt-in host agent say to each other over the user's socket. The
decision is the amendment of 2026-11-24 to ADR 0199 (one host agent per OS user, a
Unix-domain socket of mode 0600, contexts, sessions and workers, every request authorised
against the context and capability it names). The implementation is
`apps/majordomus-cli/src/fabric/`; its rustdoc is the reference, and this page is the
reader's account.

This is the contract and nothing that runs it. No command starts a host agent, binds a
socket or installs a service yet; those are the program's later phases. What exists is the
typed wire format, the rules of a conversation, the identifiers and the refusals, so that
every later phase builds on one definition instead of restating it.

## Three decisions

- **Being on the socket is not authority.** The socket admits this OS user and nobody else.
  Within it, a session is bound to one context and one connection, every call names both its
  session and its context, and a call runs only when both agree with what the host holds. A
  checkout root a client sends is where the host starts its own resolution, never proof.
- **No second registry, no second identity.** A capability is the capability registry's, by
  its canonical id, and a call is admitted against the registry's descriptor; the handler's
  own checks still run after that. A context's id is the checkout's, the id the lease and the
  index already use. The host is the mesh's node and instance.
- **What ran is always said.** Every refusal says whether the request ran, did not, or may
  have. Only a capability whose declared execution policy is idempotent is sent again after
  an unknown outcome, the rule the MCP bridge already keeps.

## Frames

A frame is four bytes of big-endian length, then that many bytes of one JSON message. The
length comes first so the host knows the size before it holds any of it: a prefix over the
bound is refused on its four bytes, and the body behind it is never read.

| Bound | Value | Shared with |
|---|---|---|
| request body | the HTTP request-body bound | `http::server::MAX_BODY_BYTES`, the MCP stdio line bound |
| reply body | the MCP bridge's reply bound | `mcp::bridge::MAX_REPLY_BYTES` |
| nesting | the JSON parser's recursion bound | every JSON reader of the executable |
| requests in flight | `fabric::message::MAX_IN_FLIGHT` per connection | — |
| deadline | `fabric::message::MAX_DEADLINE_MS`; the default is the MCP bridge's request timeout | `mcp::bridge::REQUEST_TIMEOUT` |
| checkout root | `fabric::ids::MAX_ROOT_BYTES` | Linux `PATH_MAX` |
| socket path | the platform's socket address | `sun_path`, checked by the platform's own constructor |

The request and reply bounds are the other transports' own, so a call admitted over HTTP or
MCP is admitted here (rule `project.operation-transport-parity`). A reply over its bound is
not sent: the request is answered with `output_oversized`, which says the capability ran, so
a client never retries a write because its answer was large.

These bytes travel on a socket only this user can open. They are not the bytes a
host-to-host link carries, which that link encrypts and authenticates; nothing here is
signed, because nothing here leaves the machine.

## The envelope

```json
{"request": 7, "message": {"type": "call", "session": "hs-…", "context": "…", "capability": "repository.info"}}
{"reply_to": 7, "message": {"type": "output", "output": {"…": "…"}}}
```

Every request is answered by exactly one host frame carrying its `reply_to`. The host sends a
frame unasked only to refuse a peer of another user, and that frame has no `reply_to`.

Every message reads closed. A field this protocol does not define is `message_malformed`, so
a field a client invents (a `uid`, a `root` on a call) is refused rather than ignored, and
anything new travels behind a negotiated feature. A field documented as optional may be left
out and reads as its default. A frame that is JSON but not a message is still answered on its
`request` when that field could be read.

| Client message | Answered with | Needs |
|---|---|---|
| `hello` | `welcome` | nothing; it is the first message |
| `attach` (a checkout root) | `attached` (a session and the resolved context) | a hello |
| `detach` (a session) | `ack` | the session |
| `call` (session, context, capability, input, optional deadline and idempotency key) | `output` | the session, bound to that context |
| `cancel` (a request number) | `ack` | the `cancel` feature |
| `health` | `health` (counts of contexts and sessions) | a hello |

Any of them can instead be answered with `refused`.

## Versions and features

A `hello` names the range of protocol versions the client speaks, the features it would use,
and the features it requires. The connection runs at the newest version both sides speak,
and with the optional features both have; an optional feature the host does not know is
ignored, which is what lets a newer client talk to an older host. No common version is
`protocol_unsupported`, and a required feature the host lacks is `feature_unsupported`; both
end the connection. The version and features are fixed by the `welcome` for the life of the
connection, so no later frame carries a version. A change an older peer would misread is a
new version; anything else is a feature.

The `welcome` also carries the host's node id and instance id, both the mesh's own, and the
bounds above, so a client keeps within them instead of discovering them by refusal. A client
that reconnects and meets another instance knows every session it held is gone.

## Identifiers

| Identity | Scope | Lives | Derived from |
|---|---|---|---|
| node | the OS user on one machine | as long as its key file | the mesh identity key |
| instance | one run of the host | one process | OS entropy |
| context | one checkout | as long as its canonical root | the digest of the canonical root, as the lease computes it |
| repository | one git repository, every worktree of it | as long as git's common directory | `repository::git_identity` |
| session | one attachment to one context | until detach, revocation or the host's exit | OS entropy, minted by the host |
| request | one request | until its reply | the client's own number |

A repository id is local: two clones of one project on two machines have two. The identity
that is the same across machines is the mesh repository identity, derived from root commits,
and it belongs to host-to-host routing, not to this contract.

A context id is 32 lowercase hex characters, a session id `hs-` and 32 more. A string that
reads as one never reads as the other. Other 32-hex identities read as a context id; one sent
where a context is wanted names no context of the session and is refused at admission.

`attach` resolves the root it is given on the host: the root must be absolute, within the
bound, free of NUL and of `.` and `..` components, and is then canonicalised, so a symbolic
link and its target are one context. The nearest `.ai/` layer at or above it is the checkout.
Unicode is taken as sent, unnormalised; the filesystem's canonicalisation is the only judge
of which directory a spelling names.

## Admission

Every request passes the conversation's rules first, in this order: `hello` first and once,
the message's own checks (a capability id or idempotency key in its grammar, a deadline in its
bounds, `cancel` only when negotiated), a request number not already in flight, and room for
one more in flight. A `call` is then admitted against the host's state, again in order:

1. the session is one the host holds for this connection, else `session_unknown`;
2. the context the call names is the session's own, else `context_mismatch`;
3. the capability is in the registry, else `capability_unknown`;
4. it is executable (not a resource, not merely listed), else `capability_denied`.

`session_unknown` is one code for a session never minted, detached, revoked and another
connection's, so a guessed or leaked handle learns nothing about which. Only after admission
does the capability's handler run, with its own checks, and a handler's error comes back
under the code it has on HTTP and MCP.

## Refusals

A refusal carries its code, a sentence for a person, its delivery and whether the
connection ends. A client branches on the code and never parses the sentence. A client older
than its host reads a code it does not know as `unrecognised`, and acts on the delivery and
the terminal flag, which travel beside the code for exactly that reader.

| Code | Ends the connection | Meaning |
|---|---|---|
| `frame_oversized` | yes | a length prefix over the bound; the body was not read |
| `frame_truncated` | yes | the connection ended or failed inside a frame |
| `message_malformed` | no | a whole frame that is not a message of this protocol |
| `hello_required` | yes | the first message was not `hello` |
| `hello_repeated` | no | a second `hello` |
| `protocol_unsupported` | yes | no common version |
| `feature_unsupported` | yes | a required feature the host lacks |
| `feature_not_negotiated` | no | a message that needs a feature this connection lacks |
| `invalid_identifier` | no | a capability id or idempotency key outside its grammar |
| `root_invalid` | no | a checkout root that is not absolute, bounded and normal |
| `root_unresolved` | no | a root that names no checkout with an `.ai/` layer |
| `socket_path_invalid` | no | a socket path that is not absolute or does not fit the socket address |
| `transport_unavailable` | no | this platform has no local channel in this build |
| `peer_refused` | yes | the peer is not this OS user |
| `session_unknown` | no | see admission |
| `context_mismatch` | no | see admission |
| `capability_unknown` | no | see admission |
| `capability_denied` | no | see admission |
| `deadline_invalid` | no | a deadline of zero or over the bound |
| `deadline_exceeded` | no | the deadline passed before an answer |
| `cancelled` | no | the client cancelled before an answer |
| `outcome_unknown` | no | the request may have run, and nobody knows |
| `output_oversized` | no | the answer is over the reply bound; it ran |
| `request_duplicate` | no | a request number already in flight |
| `too_many_in_flight` | no | the connection's bound on requests in flight |
| `invalid_input`, `not_found`, `refused`, `internal`, `forbidden`, `conflict` | no | the capability's handler, under its own code |

## Delivery

| Delivery | Means | May be sent again |
|---|---|---|
| `not_executed` | refused before it reached a handler | always |
| `executed` | it ran, and its answer could not be delivered | only if the capability is idempotent |
| `unknown` | it may have run | only if the capability is idempotent |

Whether a capability is idempotent is its descriptor's execution policy in the registry,
never its name. A deadline or a cancellation that arrives before a call started is
`not_executed`; after it started, `unknown`. A handler that failed internally is `unknown`,
because it may have changed something before it failed; every other handler error is the
handler declining before it acted. An `output` means the call ran exactly once.

Deadlines are relative, in milliseconds from the host's receipt, on the host's monotonic
clock, so client and host never need to agree on the time of day. Work that needs longer than
the bound is an execution (`executions.start`), which answers at once and is followed rather
than waited on.

## Platforms

The local channel is a Unix-domain socket on every Unix target. Windows gets a named pipe in
ADR 0199; this build does not carry one, and a Windows build answers `transport_unavailable`
rather than pretending to listen.

## Not in this contract

- The listener, the socket's directory and mode, and the peer-credential check on accept
  (the host agent phase).
- The context registry built from leases, revocation, and session expiry (the context phase).
- The MCP stdio bridge onto this socket, and its fallback to today's per-checkout server.
- Event subscriptions and their acknowledgement, which arrive with the event phase as a
  negotiated feature.
- Isolated worker processes: a worker is the host's to start and bound, and no worker
  identity travels here yet.
- Host-to-host links, which are encrypted and admit a host and nothing more; each stream to a
  repository needs that repository's own enrolment.

## Tests

The contract is tested where it is defined, in `apps/majordomus-cli/src/fabric/`: every
message and refusal round-trips and re-encodes to the same bytes; oversized, truncated,
interrupted and failed reads are refused with their codes, under property tests over any
bodies in any chunking; malformed, forged-field and over-nested frames are refused and
answered on their request; versions negotiate to the newest common one and refuse no
overlap; a duplicate request number is admitted once even under concurrent delivery; a
session of one context cannot call into another; the schemas generated from the types
compile and hold the frames the module writes; and the socket path bound is the platform's.

```
cd apps/majordomus-cli && cargo test fabric::
```
