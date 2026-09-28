# Security

## Reporting

Report vulnerabilities privately through GitHub's security advisory form for this
repository rather than in a public issue. Expect an acknowledgement within a week.

## What Majordomus promises

These are design commitments for v0.1. A test in `test/cases/` will back each one before
it is described as real.

- **Local only.** No telemetry. No update checks. Nothing leaves the machine. There is
  one request, and it is declared rather than tolerated: `majordomus context` reads the
  peer board of the shared MCP server this repository itself started, from the loopback URL
  that server wrote into its own lease file, so that a worker is told who else is holding
  their paths. It is bounded (`--max-time`), it is optional — no lease, no `curl`, no
  answer, or a lease naming anything but loopback, and the section is simply not written —
  and it sends nothing but the request. `test/cases/08_no_forbidden_constructs.sh` refuses
  every other network client in `bin/`, `lib/` and `share/`, and refuses this one if it
  stops being that single bounded call.
- **The mesh is off until a person turns it on.** The Rust executable's mesh (ADR 0050,
  ADR 0067) is the one declared exception on the executable's side: no discovery socket and
  no link opens until the repository commits a `mesh` declaration with `enabled: true`.
  Enabled, a server announces a signed advertisement (its key, runtime, endpoints, version
  and repository identity digest — no path, secret or content) and links only to runtimes of
  the same repository whose keys the declaration trusts; over those links it shares session
  metadata (client, intent, issue, branch, head), claim scopes, review subjects and handover
  bodies a person explicitly published. Every message is Ed25519-signed; nothing is
  encrypted, so a mesh belongs on a private network or an overlay. Nothing a peer sends
  executes anything, and the only file a peer's event can cause to be written is a handover,
  into this checkout's handovers directory, by an explicit `mesh handover consume`.
  `scripts/ci/mesh-check`, `apps/majordomus-cli/tests/mesh_cooperation.rs` and
  `test/mesh-lab/run` hold it; `docs/MESH.md` has the threat model.
- **No evaluation of generated text.** Nothing that came from a worker, a model, a
  handover body, or a policy file is ever passed to `eval`, a shell, or a template
  engine that executes.
- **No credential handling.** Majordomus never reads, stores, or asks for secrets.
- **Writes are confined.** Every write goes under `.ai/` or to a projection
  target named in the policy. Path arguments are canonicalised and refused if they
  resolve outside the repository root.
- **No silent overwrite.** Overwriting requires an explicit flag; the default is refusal
  naming the existing file. `state/` is never overwritten by any command.
- **No recursive deletion.** Retention rotates to archived files; nothing is deleted.
- **Handovers are `0600`.**
- **Authorisation is derived, not ambient.** Any input that could relax a rule is either
  computed by Majordomus from git or corroborated against a real git object. An
  environment variable never lifts a rule. The one bootstrap hatch is honoured only while
  the ledger does not exist and records itself.
- **Fail closed.** If it is unclear whether a check passed, it failed.

## What Majordomus does not promise

A local tool cannot stop a determined person with write access to the repository. It can
stop accidental and casual bypasses and make deliberate ones loud and recorded. Documented
limits: calling `git` with hooks disabled, editing `.git/hooks` directly, or pushing from
a machine without Majordomus installed all bypass it. These are stated limits, not
silent failure modes.
