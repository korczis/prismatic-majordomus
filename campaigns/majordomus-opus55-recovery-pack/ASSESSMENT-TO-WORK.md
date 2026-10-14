# Independent audit finding → remediation

| Audit finding at `3ede593c0e` | Milestones | Proof demanded |
|---|---|---|
| Local scope in gitignored editable state | M01 M03 M05 | CI recomputes trusted contract against PR diff |
| Worker chooses vacuous verifier; output discarded | M02 M03 | Pinned verifier + retained, bound outputs + rejected `ls` |
| Hooks bypassable via `--no-verify` | M03 M06 | Required forge status independently verified; native hooks are defense in depth |
| `doctor` misses `|| exit 0` mutation | M04 | Actual mutation is killed by effect-proof |
| YAML parser accepts duplicate keys/quotes quirks | M05 | Parser differential tests at all commands |
| Scope misreads reverted changes | M05 | Net-diff replay tests |
| README contradicted by optional services/runners | M00 M10 M13 | Accurate generated claims and truthful defaults |
| 0/206 recorded guarantees evidenced in baseline | M10 | Recomputed current-HEAD coverage with receipts and ratchet |
| 98-file init and slow hooks | M07 | Real fresh-repo measured path; hooks measured |
| Rust/shell split-brain | M08 | Single transition owner with compatibility tests |
| MCP 166 tools / 1.8 MB | M09 | Measured, semantically compatible discovery compression |
| Provider-specific scoped rules not emitted | M06 | Native denial/Stop tests, provider matrix |
| Intents only about intent | M11 | One independent real product outcome satisfied by CI evidence |
| No external adoption | M12 | Five actual external users or **blocked**, never fictional |
| Mesh/cockpit/economics/integration scope growth | M13 | Isolation and opt-in based on measured user value |

The audit contains both observed results and inference. Treat each row as a lead, not a timeless fact. In particular, ecosystem claims about October 2026 must be confirmed against provider documentation at implementation time.
