+++
title = "An enabled mesh declaration whose server did not activate the mesh is a failed `runtime` check of `mesh doctor` (exit 10) carrying the server's reason, and a `Mesh: DECLARED ENABLED BUT NOT ACTIVE` line in the session-start briefing — never a quiet off; and this repository's committed declaration is enabled, allowlisted and names a hub"
description = "The mesh has two truths that can drift apart: what the repository declares, and what"
weight = 179
[extra]
claim_id = "mesh-declared-is-held"
status = "guaranteed"
source = "docs/claims/mesh-declared-is-held.md"
+++
{% raw %}

## What it means

The mesh has two truths that can drift apart: what the repository declares, and what
the shared server did with it. On 2026-09-12 three machines each ran an uncommitted
`enabled: true` while master said `false`; the checkout that was reset to master started
a server whose mesh was, correctly, off — and nothing told anyone. This claim is that the
drift is loud in the two places a worker already looks. `majordomus mesh doctor` carries
a `runtime` check that reads the server's own decision — activated, or declined with a
reason — and fails, exit 10, when the declaration is enabled and the mesh is not active,
naming the reason and the restart that follows fixing it. The briefing the provider's
start event injects carries a `Mesh:` line whenever a declaration exists: `active` with
what the server sees, `off, as declared`, or `DECLARED ENABLED BUT NOT ACTIVE` with the
same reason. And the declaration of this repository itself is held: a test refuses a
tree whose committed declaration is disabled, untracked, without an allowlist of at
least the fleet's size, or without a rendezvous hub.

## How it works

The runtime keeps one bit beside its state: whether a shared server decided anything
about it (`MeshRuntime::decided`). In a server it is set by activation or by a decline
with a reason; in the command line's process nothing sets it, so an undecided runtime is
an absence and not a verdict. `mesh.doctor` (`apps/majordomus-cli/src/mesh/doctor.rs`)
appends the `runtime` check from the server's status when the bit is set, and a
`trust` check from the declaration and this machine's own key. `majordomus mesh doctor`
asks this checkout's running server for the report when one serves it, so the verdict
is the server's, and runs in-process only when none does. The hook library
(`lib/capture.sh`) renders the same report into the one briefing line, through the
executable — it sends no request of its own, so SECURITY.md's single declared request
stays single.

## How to see it

```
majordomus mesh doctor                       # ok/FAIL per check; `runtime` is the server's verdict
curl http://127.0.0.1:8741/api/v1/mesh/doctor
bash test/run.sh 290_the_mesh_is_declared_and_held
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test mesh
```

`an_enabled_declaration_the_server_could_not_activate_fails_the_doctor_and_names_why`
proves the failed verdict over a real server whose identity cannot be written;
`test/cases/290_the_mesh_is_declared_and_held.sh` drives a disposable repository
through the start event for the three briefing answers, and reads this repository's
own `.ai/repo/mesh/majordomus.yaml`.

## What it does not cover

It does not make the mesh reach anybody: a mesh that is active with every provider
failed is named as degraded, and a fleet that cannot route to its hubs is a `present`
count of zero, not a failed check. It does not hold the skeleton: a fresh repository
still starts with `enabled: false` and declares no `mesh-declaration` source class
(ADR 0059's open end). And the trust verdict labels; it grants nothing
(`mesh-observation-not-authority`).

## Why it exists

A declaration that says on and a server that is off is the one state in which every
other mesh guarantee holds and the operator is still alone. Making that state a failed
check and a briefing line is what turns "the mesh was quietly disabled for a day" into
"the first line of the first session said so".
{% endraw %}
