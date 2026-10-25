# Acceptance matrix / proof rules

| ID | Test | Required real proof |
|---|---|---|
| E2E-01 | local serve, select intent/issue, click Implement | browser + real server + canonical ticket + worker change and test |
| E2E-02 | double-click/retry/reload | exactly one mutating ticket; stable ID |
| E2E-03 | browser disconnect/reconnect | cursor replay, correct order, no fabricated progress |
| E2E-04 | server kill while execution running | persisted state and safe recovery, no duplicate mutation |
| E2E-05 | two workers overlapping file scope | one claim, other refusal/queued, fenced lease |
| E2E-06 | disabled/no worker | explicit waiting/blocked reason, no fake executor |
| E2E-07 | hub down, local intact | local end-to-end succeeds and status says unavailable |
| E2E-08 | signed two-node remote execution | trust enforced, authenticated peer, actual remote task, no public secrets |
| E2E-09 | anonymous remote HTTP/MCP request | no private topology, mutations denied |
| E2E-10 | Gigalixir restart | stable identity, correct advertised HTTPS endpoint, reconnection |
| E2E-11 | Gigalixir provider deploy | successful workflow AND actual /live SHA AND authorized rendezvous |
| E2E-12 | Verify & Deliver | exact-head green required CI, release artifact smoke, served SHA |
| E2E-13 | failed CI/unavailable provider | precise failed/blocked state, no success badge |
| E2E-14 | public GH Pages | static-safe snapshot, language parity, links and version provenance |
| E2E-15 | mobile 320px & keyboard | accessible controls, no browser errors, no overflow |
| E2E-16 | evidence freshness | same canonical verdict with provenance across all surfaces |
| E2E-17 | DNS multi-address, TLS failure, slow peer | bounded deadlines, safe address fallback, no cleartext leak |
| E2E-18 | installation on foreign repo | binaries/releases work without source toolchain and with safe repo isolation |

Each result must include test command, exact SHA, environment, timestamp, duration, result PASS/FAIL/SKIP, logs and artifacts/screenshots. Required SKIP is not a PASS. CI pass cannot compensate for missing provider live checks. Use real browser, not only static HTML assertions.
