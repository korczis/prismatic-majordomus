<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `continuity` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.20.0 -->
# Module `continuity` — Continuity

What this checkout's lifecycle is holding: the open episode, the record the next worker would resume from with the label that says how far to trust it, the newest progress note, and what is blocking acceptance. Read from the local half of the layer, which this process serves to the worker in front of it and never publishes.

Stability: behaviorally_verified. Capabilities: 8.

## `continuity.device` — This device's identity, and its label

The device a published handover names: the mesh node key (created when this device has none, in the user's state directory and never in a repository), its node id, its public key — what a trust list admits — and the label a person reads. With a label, renames the device; the key and every signature it made are unchanged.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_continuity_device` |
| HTTP | `POST /api/v1/continuity/device` |
| CLI | `majordomus continuity device` |
| cache | — |
| benchmark | waived (destructive) |
| provenance | builtin majordomus_cli::capability::builtin::continuity |
| tags | continuity, devices |

| input | type | required | description |
|---|---|---|---|
| `label` | string or null | no | A label to give this device (`macbook-pro`, `mac-mini`): letters, digits, `.`, `_`,
`-`. The key, and so the identity, is unchanged. |

Output: `DeviceView`.

## `continuity.plan` — Whether and how a published handover can be resumed here

Writes nothing. Chooses the record (named, or the one other device's handover this checkout has not resumed, on this branch first) and decides: is its signer trusted, does it diverge from the record this checkout stands on, does the local source hold the commit and branch it was written against, was it written in a dirty tree whose uncommitted files never reached a commit, may the other device still be working. The verdict is ready, ready_with_warnings, requires_source_update, conflict, choose_record, nothing_to_resume or refused, with blockers, warnings, the commands that resolve them (recommended, never run), what a resume restores and what this checkout recomputes for itself.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_continuity_plan` |
| HTTP | `GET /api/v1/continuity/plan` |
| CLI | `majordomus continuity plan` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::continuity |
| tags | continuity, handover, devices |

| input | type | required | description |
|---|---|---|---|
| `record` | string or null | no | A record id, or a unique prefix of one. |

Output: `ResumePlan`.

## `continuity.publish` — Publish this checkout's newest handover for another machine

Projects the newest handover record into a portable record — the handover body, the repository and device identity, the episode, the source state (branch, commit, and for a dirty tree the changed paths and a fingerprint, never their content), the task and its decisions, and the record it continues — refuses it when any value carries a credential, a secret environment value or a path of this machine's disk, signs it with the device's mesh key and adds it to refs/majordomus/continuity. Touches no branch, index or working tree, and no network: a sync publishes it.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_continuity_publish` |
| HTTP | `POST /api/v1/continuity/publish` |
| CLI | `majordomus continuity publish` |
| cache | — |
| benchmark | waived (destructive) |
| provenance | builtin majordomus_cli::capability::builtin::continuity |
| tags | continuity, handover, devices |

| input | type | required | description |
|---|---|---|---|
| `handover` | string or null | no | The file name of a handover record under `.ai/local/state/handovers/`; the newest
when absent. |
| `issue` | string or null | no | The issue the work belongs to (`#184`, `I0042`). |
| `milestone` | string or null | no | The milestone it belongs to. |

Output: `Published`.

## `continuity.records` — Every published handover the local store admits

Every record of refs/majordomus/continuity that passed admission — schema, id, signature, repository identity, portability — by line, with each line's heads, and a diagnostic for every file refused and every lineage defect.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_continuity_records` |
| HTTP | `GET /api/v1/continuity/records` |
| CLI | `majordomus continuity records` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::continuity |
| tags | continuity, handover, devices |

Input: none.

Output: `Records`.

## `continuity.resume` — Resume a handover another machine published

Plans exactly as continuity.plan does and acts only on a ready or ready_with_warnings plan: writes the handover into this checkout's handovers (where handover --resolve and the session briefing find it), appends the task's decisions to the decision log once each, and records the record as the one this checkout continues, so that its next publication extends the same line. Runs nothing the record says; the start command for the task is returned as a recommendation.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_continuity_resume` |
| HTTP | `POST /api/v1/continuity/resume` |
| CLI | `majordomus continuity resume` |
| cache | — |
| benchmark | waived (destructive) |
| provenance | builtin majordomus_cli::capability::builtin::continuity |
| tags | continuity, handover, devices |

| input | type | required | description |
|---|---|---|---|
| `record` | string or null | no | A record id, or a unique prefix of one. |

Output: `Resumed`.

## `continuity.state` — What the lifecycle is holding

The open episode, the active task, the handover and checkpoint that resolve for this worktree and branch, each with its divergence label, the unresolved questions that refuse completion, and the record tallies. Selection is two-tiered and never repository-wide: a record from an unrelated worktree or branch is not offered, because a briefing that is quietly about somebody else is worse than none. Absence is reported as absence.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_continuity` |
| MCP resource | `majordomus://continuity` |
| HTTP | `GET /api/v1/continuity` |
| cache | process, 2 entries, 2s |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::continuity |
| tags | continuity, session, handover |

Input: none.

Output: `Continuity`.

## `continuity.status` — Where this device stands in the work published from every device

This device and repository identity, the record this checkout continues on its branch, how the local continuity store stands towards the remote's (from the refs alone, no network), every line of work with its heads, the handovers other devices published that this checkout could resume, and every refused record or broken lineage as a diagnostic. A diverged line — the same work continued twice — is reported; separate lines of work on separate devices are not a conflict.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_continuity_status` |
| HTTP | `GET /api/v1/continuity/status` |
| CLI | `majordomus continuity status` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::continuity |
| tags | continuity, handover, devices |

Input: none.

Output: `Status`.

## `continuity.sync` — Exchange published handovers with a git remote

Fetches the remote's refs/majordomus/continuity, merges it into the local store as the union of both (records are content-addressed, so a name holding different bytes is reported and the local copy kept), and pushes the result, never forced. Reports each line as equal, remote_newer, local_newer, diverged, local_only or remote_only. An unreachable remote changes nothing and leaves what is pending pending.

| | |
|---|---|
| kind | command |
| stability | experimental |
| MCP tool | `majordomus_continuity_sync` |
| HTTP | `POST /api/v1/continuity/sync` |
| CLI | `majordomus continuity sync` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::continuity |
| tags | continuity, handover, devices |

| input | type | required | description |
|---|---|---|---|
| `remote` | string or null | no | The git remote; the current branch's, else `origin`, when absent. |

Output: `Synced`.

