# Acceptance matrix
| Claim | Required evidence |
|---|---|
| Web deployed | Public URL + deployed SHA + timestamp + external fetch |
| Gigalixir deployed | Release SHA + successful runtime health + deploy log |
| CI green | Run ID, tested SHA, required checks passing |
| Docs current | Generated from current canonical state, publish SHA and link checks |
| Cockpit live | Real backend event delivered to browser via documented transport |
| Mesh working | Authenticated peer, capability discovery, task completion and trace |
| No drift | Source, artifact and deployment SHA mapping explained |
| Secure | Required security checks and no secrets in published output |
| Rollback ready | Documented and validated rollback target/procedure |

Failure of a gate means NOT VERIFIED, not 'mostly done'.
