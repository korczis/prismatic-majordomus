# Acceptance matrix: historical GitHub findings mapped to mission and proof

| Finding | Issue / PR | Priority | Mission | Minimum proof |
|---|---|---|---|---|
| Durable ACK and bounded journal reload | I2706 / #1137 | P0 | 02 | fail-append cannot ACK; reload bounded; fault tests |
| DNS rebinding / UI script policy | I2151 / #996 | P0 | 03 | cross-origin mutation refused; CSP test |
| Live key rotation and revocation | I2705 / #1136, I2134/#989 | P0 | 02 | 3-node rotation/replay tests |
| Mesh event schema/redaction | I2708 / #1139 | P0 | 02,03 | invalid field and synthetic secret tests |
| Dialer bounds/retry | I2704 / #1135 | P1 | 02 | size/time/retry exhaustion test |
| Journal event pagination | I2140 / #993 | P1 | 02 | no gap/duplicate pagination under concurrent append |
| Runtime lease holder | I2700 / #1131 | P1 | 04 | token mismatch refused |
| Stable server address | I2702 / #1133 | P1 | 04 | deterministic restart/collision test |
| Idle server configuration | I2701 / #1132 | P2 | 04 | command/config precedence test |
| Doctor stale runtime | I2707 / #1138 | P1 | 04 | stale lease makes actionable finding |
| Server version compatibility | I2163 / #1000 | P1 | 04 | correct handshake/replacement refusal |
| Red trunk ownership | I2709 / #1144 | P1 | 05,06 | unrelated landing blocked |
| CI release runner contention | I2662 / #1105 | P1 | 06 | dispatch under saturated runners |
| Publish artifact with build | I2660 / #1103 | P1 | 06 | published SHA traced to build |
| Failed publish visible | I2661 / #1104 | P1 | 06 | failed deploy creates visible finding |
| Published UI not rebuilt from nothing | I2663 / #1106 | P2 | 06 | cache + exact artifact proof |
| Branch protection provenance | I2282 / #1001 | P1 | 06 | dispatch status cannot forge required check |
| HTTP stack performance | I2488 / #1075 | P2 | 08 | same-load benchmark decision |
| UI live stream | I2487 / #1074 | P1 | 09 | reconnect+gap+cursor test |
| UI route coverage | I2482 / #1035 | P1 | 09 | all route smoke + links |
| UI server bootstrap | I2484 / #1037 | P2 | 04,09 | fresh repo UI works if supported |
| Knowledge UI projection | I2652 / #1091 | P2 | 07,09 | canonical data with status/freshness |
| Kiosk evidence scenes | PR #1124 | P2 | 09 | source-backed scene models and screenshots |
| Cluster dashboard | PR #1111 | P2 | 09 | CLI/API/UI parity |
| Shared build-cache typed offer | PR #1118 / #1101 | P2 | 08 | selection deterministic; optional cache fallback |
| Landing page / customer narrative | PR #1098 | P2 | 10 | adopter proof + truthful copy |
| Second adopter findings | PR #1089, #1081-94 | P1 | 10 | reproducible tests on foreign-stack fixture |
| Release tag grace | PR #1145 | P1 | 06 | not falsely green, no freeze of other head |
| Board journal duplication | PR #922 | P1 | 02 | no re-projection of unchanged long intent |
| 42 PR merge queue | live query required | P1 | 05 | each PR classified by diff; no lost unique work |
| Canonical rules/claims | DOCTRINE, SITE_CLAIMS | P1 | 07 | violation propagation + cross-surface tests |
| Performance measurements | PERFORMANCE doc | P2 | 08 | controlled before/after evidence |
| Optional lightweight core | README | P1 | 10 | basic lifecycle works without extras |

All statuses in this matrix are *target tasks*, not claims of current issue state. Live reconciliation may mark some as already delivered, superseded or nonexistent, but it must cite proof.
