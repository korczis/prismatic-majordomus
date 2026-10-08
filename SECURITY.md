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
  and it sends nothing but the request. A server bound to every interface publishes the
  unspecified address (`0.0.0.0`), which names no host; it is asked at `127.0.0.1` on the
  same port, so the request still cannot leave the machine
  (`test/cases/106_context_peers.sh`). `test/cases/08_no_forbidden_constructs.sh` refuses
  every other network client in `bin/`, `lib/` and `share/`, and refuses this one if it
  stops being that single bounded call.
- **A local server listens on loopback until this machine says otherwise.** The Rust
  executable's `serve` and `mcp` bind `127.0.0.1`. Nothing tracked can change that: the one
  thing that moves it is `MAJORDOMUS_HTTP_HOST` in a machine's own environment, or `--host`
  on a command line, and a server bound beyond loopback says so in its log every time it
  starts. The surface has no authentication, so a machine that names `0.0.0.0` hands every
  host that reaches it the read surface and the declared writing commands
  (`docs/MCP.md`, "The machine names the interface";
  `test/cases/992_the_local_bind_is_the_machines_to_name.sh`).
- **The mesh is off until a person turns it on.** The Rust executable's mesh (ADR 0050,
  ADR 0067) is the one declared exception on the executable's side: no discovery socket and
  no link opens until the repository commits a `mesh` declaration with `enabled: true`.
  Enabled, a server announces a signed advertisement (its key, runtime, endpoints, version
  and repository identity digest — no path, secret or content) and links only to runtimes of
  the same repository whose keys the declaration trusts; over those links it shares session
  metadata (client, intent, issue, branch, head), claim scopes, review subjects and handover
  bodies a person explicitly published. Every message is Ed25519-signed; nothing is
  encrypted, so a mesh belongs on a private network or an overlay. When the declaration
  also enables `bonjour` (ADR 0120; off unless the block says so), the server registers one
  service instance with the operating system's DNS-SD daemon and opens no socket for it:
  the instance's name (`majordomus-` and sixteen hex characters of the node id and runtime
  slot), its HTTP port and, as its TXT record, the same signed advertisement become
  answerable to every device on the network segment that browses mDNS, not only to
  listeners of the mesh's multicast group. It adds no fact to the advertisement, crosses no
  router, and grants nothing: what is found that way is verified and trusted exactly as
  before. Nothing a peer sends
  executes anything, and the only file a peer's event can cause to be written is a handover,
  into this checkout's handovers directory, by an explicit `mesh handover consume`.
  `scripts/ci/mesh-check`, `apps/majordomus-cli/tests/mesh_cooperation.rs` and
  `test/mesh-lab/run` hold it; `docs/MESH.md` has the threat model. The skeleton a new
  repository starts from ships no declaration. This repository commits its own enabled:
  multicast and Bonjour on the local segment only, rendezvous hubs on the owner's private
  network and tailnet, and `deny_unknown` trust listing the owner's five machines' keys and no
  other
  (`docs/MESH.md`, "This repository's mesh"; `test/cases/491_the_mesh_is_on_here.sh`).
  Every session start says whether this checkout's server holds that declaration: the hook
  runs the executable's `mesh doctor`, which asks the server on loopback, so the hook library
  itself still sends no request (ADR 0059; `test/cases/494_the_mesh_is_declared_and_held.sh`).
- **Pull-request integration reaches the forge only when asked to.** The Rust executable's
  second declared exception (ADR 0101): `majordomus prs refresh`, `prs drain`,
  `prs cleanup` and `prs repair --apply` run the GitHub CLI (`gh`), `git fetch` and
  `git ls-remote` against this repository's own `origin`, with the person's own `gh` credentials. Nothing else does.
  `prs status`, `plan`, `explain`, `events`, `repair` without `--apply`, the HTTP routes
  under `/api/v1/pull-requests`, the MCP tools and the Cockpit render the observation last
  recorded under `.ai/local/state/integration/`, with its moment, and never reach the
  network. The executor never passes `--admin` and never rewrites a branch: the one push it
  makes, bringing master into a pull request's branch, is a plain (never forced) push of a
  fast-forward of the head it observed, made only while origin still serves that head
  (`test/cases/912_repair_acts_under_the_lease_and_the_trail.sh`).
  It never closes a pull request without `--apply` and proof that its work is on master, or
  that a declared successor landed (`test/cases/720_integration_follows_the_current_master.sh`).
  A declaration counts only from an owner, member or collaborator in a pull request of this
  repository, and nobody else's holds or closes anything
  (`test/cases/937_an_unauthorised_declaration_neither_holds_nor_closes.sh`). Two residuals
  are known and accepted (ADR 0101 §6): anyone who can mention a pull request more often
  than its cross-references are read holds it, and never closes it
  (`test/cases/938_a_truncated_cross_reference_read_holds.sh`); and the forge's `MEMBER`
  covers every member of an organisation, whatever their access to this repository.
  Nothing outside it merges: `scripts/ci/backlog-check` refuses a `gh pr merge`, a REST or
  GraphQL merge, an auto-merge action and the retired `scripts/unblock` anywhere in the
  scripts, recipes, workflows, libraries and shared assets
  (`test/cases/910_nothing_merges_around_the_integrator.sh`).
- **No evaluation of generated text.** Nothing that came from a worker, a model, a
  handover body, or a policy file is ever passed to `eval`, a shell, or a template
  engine that executes.
- **No credential handling.** Majordomus never reads, stores, or asks for secrets.
- **Writes are confined.** Every write goes under `.ai/` or to a projection
  target named in the policy. Path arguments are canonicalised and refused if they
  resolve outside the repository root.
- **No silent overwrite.** Overwriting requires an explicit flag; the default is refusal
  naming the existing file. `state/` is never overwritten by any command.
- **No recursive deletion but the four named here, and every deletion is accounted for.**
  Retention rotates to archived files rather than removing them. A directory tree is removed
  whole in four places only:
  - the tool's own temporary directories;
  - `majordomus web compose` empties its destination before it copies — `target/site`, or
    the path `--destination` names — whatever that path already holds, and before it checks
    that any producer ran, so `--destination` must name only a directory meant to be regenerated;
  - `majordomus rules vendor update` replaces the vendored package under
    `.ai/repo/rules/vendor/` whole, and refuses without `--force` when it was hand-edited;
  - `majordomus worktree migrate` removes the original of a worktree only once its copy was
    fingerprinted identical and git no longer registers it.

  `majordomus migrate` removes the legacy `.majordomus/` files it replaces: derived files and
  templates byte-identical to the tool's own outright, provider templates only after a backup
  verified byte for byte, and then the directories left empty. Otherwise it removes files it
  writes and owns (its own locks, leases and caches), and `majordomus recover` removes records,
  and only what it has accounted for:
  - a duplicate closed record of one episode, once its `changed_files` and `commits` are
    folded into the oldest record, which says so in a `## Recovery` section;
  - the open-session file of an episode stranded past the `session.stranded_after`
    threshold (or an explicit `--older-than`), once its closed record is published;
  - through the `recover.orphans` capability, a regular file of the two temporary shapes
    this tool writes (`.tmp.*` in a record store, `<target>.mj-tmp`), older than that same
    threshold, that is empty, whose episode is already published, or whose target already
    exists. A temp holding the only copy of a record is published first and removed only
    once its record exists; content it cannot classify, a file whose age it cannot read, and
    every directory are reported and left where they are.

  A fold, a recovered episode and a published temp are written to the ledger as
  `session.recovered`, and `--check` prints the plan with its evidence and writes nothing.
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
