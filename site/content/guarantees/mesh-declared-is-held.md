+++
title = "An enabled mesh declaration whose shared server did not activate the mesh is a failed `runtime` check of `mesh doctor` (exit 10) carrying the server's reason, and a `Mesh: DECLARED ENABLED BUT NOT ACTIVE` line in the session-start briefing — never a quiet off; and this repository's committed declaration is tracked, enabled, allowlists the fleet and names a hub"
description = "The mesh has two truths that can drift apart: what the repository declares, and what the"
weight = 209
[extra]
claim_id = "mesh-declared-is-held"
status = "guaranteed"
source = "docs/claims/mesh-declared-is-held.md"
+++
{% raw %}

## What it means

The mesh has two truths that can drift apart: what the repository declares, and what the
shared server did with it. On 2026-09-12 three machines each ran an uncommitted
`enabled: true` while master said `false`; the checkout that was reset to master started a
server whose mesh was, correctly, off — and nothing told anyone. This claim is that the drift
is loud in the two places a worker already looks.

`majordomus mesh doctor` ends with a `runtime` check that reads the server's own decision —
activated, or declined with a reason — and fails, exit 10, when the declaration is enabled and
the mesh is not active, naming the reason and the restart that follows fixing it. The briefing
the provider's start event injects carries a `Mesh:` line under the `Shared server:` line
whenever the repository tracks a declaration: `active — …` with what the server sees,
`off, as declared`, or `DECLARED ENABLED BUT NOT ACTIVE — …` with the same reason. And this
repository's own declaration is held: a tree whose committed declaration is untracked,
disabled, not `deny_unknown`, allowlists fewer keys than `docs/MESH.md` names machines, or names
no rendezvous hub is refused.

## How it works

The mesh runtime keeps one bit beside its state: whether a shared server decided anything
about it (`MeshRuntime::decided`, `apps/majordomus-cli/src/mesh/manager.rs`). A server sets it by
activating the mesh or by declining with a reason; in the command line's process nothing sets
it, so an undecided runtime is an absence and not a verdict. The `mesh.doctor` capability
passes the runtime's status to `doctor_at` (`apps/majordomus-cli/src/mesh/doctor.rs`) only when
the bit is set, and the `runtime` check judges it against the declaration: enabled and not
active is the one failure, beside an active mesh whose every declared transport failed.

`majordomus mesh doctor` asks this checkout's running server for the report
(`GET /api/v1/mesh/doctor`) when one serves it, so the verdict is the server's, and runs in its
own process only when none does. The hook library (`lib/capture.sh`, `mj_capture_mesh_line`)
runs that command after the start event has ensured the server and renders the report into the
one briefing line; it sends no request of its own, so SECURITY.md's single declared request
stays single.

## How to see it

```
majordomus mesh doctor                      # ok/FAIL per check; `runtime` is the server's verdict
majordomus mesh doctor --format json        # the same report, for scripts; exit 10 on a failed check
majordomus run mesh.doctor --format json    # this process's self-check, never a server's
bash test/run.sh 494_the_mesh_is_declared_and_held
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test mesh
```

A server whose identity cannot be written, under an enabled declaration, briefs:

```
Shared server: ready http://127.0.0.1:8741 pid 123 version 0.10.0 (started by this call)
Mesh: DECLARED ENABLED BUT NOT ACTIVE — identity: /srv/state/majordomus/node.json: Is a directory (os error 21)
```

`an_enabled_declaration_the_server_could_not_activate_fails_the_doctor_and_names_why` in
`apps/majordomus-cli/tests/mesh.rs` proves the failed verdict over a real server, beside the
active and off-as-declared verdicts of the two tests before it;
`test/cases/494_the_mesh_is_declared_and_held.sh` reads this repository's committed declaration
and drives a disposable repository through the start event for every briefing answer.

## What it does not cover

It does not make the mesh reach anybody: an active mesh whose hubs are unreachable holds, and
shows it as a `present` count of zero, not as a failed check. It does not start, supervise or
install the hubs — they are servers started by hand beyond loopback, and an installer is owed
as a typed capability (ADR 0059). The briefing line is absent when no server is ready (the
`Shared server:` line says why) or the executable cannot answer. It does not hold the skeleton,
which ships no declaration (`mesh-off-by-default`). And the trust verdict labels; it grants
nothing (`mesh-observation-not-authority`).

## Why it exists

A declaration that says on and a server that is off is the one state in which every other
mesh guarantee holds and the operator is still alone. Making that state a failed check and a
briefing line is what turns "the mesh was quietly off for a day" into "the first lines of the
first session said so".
{% endraw %}
