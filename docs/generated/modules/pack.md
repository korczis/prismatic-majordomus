<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `pack` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.19.1 -->
# Module `pack` — Source pack

The tracked tree as a few token-bounded Markdown shards a language model's file search can index, with an index that orients the reader and a manifest that proves afterwards what left the machine: the git index's blobs only, never the working tree, and never a binary, a gitlink or worktree, a symbolic link, build output or a credential, whatever the index holds.

Stability: behaviorally_verified. Capabilities: 2.

## `pack.plan` — Plan a source pack

What a profile of share/archive.yaml would pack: the files carried and their bytes and o200k_base tokens, every file left out by reason (worktree, link, artifact, binary, derived, excluded; derived counted, the rest listed), the shards it is cut into within the profile's token budget and file count, and every finding that refuses the build — a file over the budget, too many shards, a machine path or credential in a selected file, an empty selection, a profile without limits. Reads git's index and blobs only; writes nothing.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_pack_plan` |
| HTTP | `GET /api/v1/pack/plan` |
| CLI | `majordomus pack plan` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::pack |
| tags | pack, archive, review, privacy |

| input | type | required | description |
|---|---|---|---|
| `profile` | string or null | no | A profile of `share/archive.yaml`; its `default` when absent. |

Output: `PackPlan`.

## `pack.verify` — Verify a written source pack

Read a pack directory inside the repository against its pack.json: every file it names is present with its digest and nothing else is, every carried file's content has the digest its marker records, no carried path is a binary, an artifact, a link or a gitlink under the profile the manifest names, no file is over the profile's token budget or holds a NUL byte or a leak, and there are no more files than the profile allows. An unreadable manifest is unmeasured, never a pass.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_pack_verify` |
| HTTP | `GET /api/v1/pack/verify` |
| CLI | `majordomus pack verify` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::pack |
| tags | pack, archive, privacy, evidence |

| input | type | required | description |
|---|---|---|---|
| `dir` | string | yes | The pack's directory, inside the repository: relative to its root, or absolute
under it. |

Output: `PackVerdict`.

