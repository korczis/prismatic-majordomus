# Acceptance test blueprint

| Scenario | Required observable outcome |
|---|---|
| UI submits intent twice | one logical intent or explicit version/idempotency conflict |
| Plan re-materialized | no duplicate canonical milestone/issue |
| Blocked dependency | issue is not scheduled |
| Unauthorized action | rejected by shared policy, audited |
| Browser socket drops | reconnect with resume or gap/snapshot reconciliation |
| Duplicate/out-of-order event | projection stays consistent with authoritative snapshot |
| Worker lease expires | stale worker fenced, state reconciled |
| Two workers claim issue | one owner at a time, or well-defined parallel semantics |
| Provider unavailable | explicit unavailable, no fake zero cost or green success |
| Execution succeeded but tests fail | issue remains unverified |
| PR green for old commit | merge blocked pending current head checks |
| Deployment not externally observed | marked unverified |
| New capability introduced | generated CLI/API/MCP/UI discoverable without second manual registry |
| New governance violation fixture | required validator fails with useful diagnostic |
| Inherited `.ai` rule weakened | context integrity check rejects change |
| Huge event stream and graph | bounded memory/lag, predictable degraded behavior |
| UI untrusted log payload | escaped rendering, no XSS |
| Mobile and keyboard controls | usable actionable state, not just screenshots |
