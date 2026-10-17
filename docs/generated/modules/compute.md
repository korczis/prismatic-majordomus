<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `compute` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.28.0 -->
# Module `compute` — Browser compute

The server's half of browser compute (ADR 0170): a browser computes with the portable core, the same code this executable links, and returns a claim; the server recomputes it on the same input and says whether they agree. A verification records nothing, so any caller may ask, and agreement among browsers is never proof.

Stability: experimental. Capabilities: 1.

## `compute.verify` — Whether a browser's result is what the server computes

Recompute a claimed result of a portable-core function (today `canonical_digest`: SHA-256 of a JSON document's canonical bytes) on the input the claim carries, with the same core the browser ran as wasm32, and answer `match`, `mismatch`, or `invalid` with the reason when the input is not JSON, exceeds 1 MiB or the result is not the function's shape. It records nothing: no execution, ledger entry or journal event, and the claim never enters a record.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_compute_verify` |
| HTTP | `POST /api/v1/compute/verify` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::compute |
| tags | compute, browser, verification |

| input | type | required | description |
|---|---|---|---|
| `function` | ComputeFunction | no | The function the claim was computed with. |
| `input` | string | yes | The input the browser computed on, as JSON text, at most `MAX_INPUT_BYTES`. |
| `claimed` | string | yes | The result the browser claims: for `canonical_digest`, 64 lowercase hex characters. |

Output: `ComputeVerification`.

