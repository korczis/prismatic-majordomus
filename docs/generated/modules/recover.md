<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `recover` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.6.0 -->
# Module `recover` — Recovery of the record stores

The stray files a killed publish, an interrupted rename or an unfinished site build leave in the record stores, classified by reading the clock and the content, and swept exactly once. This is the capability that backs `majordomus recover` (ADR 0040): the command no longer decides what a stray is, it asks.

Stability: behaviorally_verified. Capabilities: 1.

## `recover.orphans` — Classify every stray file of the record stores, and sweep the ones that are nobody's

Walk the publish temps of every record store, the rename temps under the layer's local half and the tracked sessions section, and the site generator's staging directories at the repository root; give each a verdict by reading its age and then its content; and, unless the call is a check, remove the ones the verdict says nothing is holding. Nothing is deleted for being unrecognised: a temp holding the only copy of a record is reported as `incomplete` and left for the caller to publish, content this version cannot classify is `foreign` and left exactly where it is, a candidate whose age this platform cannot read is `unmeasurable` and never a candidate, and no directory is ever removed. The threshold is required and has no default, because a sweep measuring against an absent threshold would find every file stale.

| | |
|---|---|
| kind | command |
| stability | behaviorally_verified |
| MCP tool | `majordomus_recover_orphans` |
| HTTP | `POST /api/v1/recover/orphans` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::recover |
| tags | recover, sessions, maintenance, lifecycle |

| input | type | required | description |
|---|---|---|---|
| `older_than_seconds` | integer | yes | How long a stray file must have been untouched before it is a candidate at all, in
seconds. Required, and refused at zero: the shell tool resolves it from
`session.stranded_after` with `mj_pol_req`, which fails closed, and nothing here
invents one. |
| `check` | boolean | no | Classify and report, and write nothing. The default, because the alternative deletes
files. |

Output: `RecoverOrphansResult`.

