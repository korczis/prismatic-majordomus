# Convergence baseline of 2026-10-14

Recorded 2026-10-14T20:19:50Z. Every classification below was made against master `e8505a431d552b9a9d6b7baebb91dfdee4451bbf` (committed
2026-10-14T21:43:41+02:00), 2 commit(s) after `97c4521f91`, the head the pack
`campaigns/majordomus-convergence-pack-2026-10-14` names as its historical baseline. This is a
measurement taken once, kept as evidence; it is replaced by measuring again, never edited.

| file | what it holds |
|---|---|
| `baseline.json` | the head, the counts, the classifier's tallies, the CI runs on the head, the local gates run, every finding's disposition |
| `pr-inventory.tsv` | every open pull request against master: head, behind/ahead, files, conflicts, checks on its head, stacking, classification, claims |
| `issue-coverage.tsv` | every row of the pack's acceptance matrix: its issue's derived plan status, what each `#N` is on the forge, the open pull requests naming the issue, the master commits naming it |

Open pull requests: 43. Classification: blocked-by-failing-check 29, partially-present 1, unique 12, unsafe 1.
A head is blocked by a failing check when the latest run of any check name on it failed; the
required `ci` check is the failing one on 26 of them. 9 head(s) carry no check run at all: #821, #882, #928, #965, #1060, #1080, #1082, #1099, #1101.
No open pull request's patch is already wholly on master by `git cherry` or by an empty merge result.

Dispositions of the acceptance matrix:

| finding | priority | issue | plan status on master | disposition |
|---|---|---|---|---|
| Durable ACK and bounded journal reload | P0 | I2706 | READY | open (ready, no open PR names it) |
| DNS rebinding / UI script policy | P0 | I2151 | READY | open (ready, no open PR names it) |
| Live key rotation and revocation | P0 | I2705 | BLOCKED | open (blocked, no open PR names it) |
| Trust revocation (with rotation) | P0 | I2134 | READY | open (ready, no open PR names it) |
| Mesh event schema/redaction | P0 | I2708 | READY | open (ready, no open PR names it) |
| Dialer bounds/retry | P1 | I2704 | READY | open (ready, no open PR names it) |
| Journal event pagination | P1 | I2140 | READY | open (ready, no open PR names it) |
| Runtime lease holder | P1 | I2700 | READY | open (ready, no open PR names it) |
| Stable server address | P1 | I2702 | READY | open (ready, no open PR names it) |
| Idle server configuration | P2 | I2701 | READY | open (ready, no open PR names it) |
| Doctor stale runtime | P1 | I2707 | READY | open (ready, no open PR names it) |
| Server version compatibility | P1 | I2163 | BLOCKED | open (blocked, no open PR names it) |
| Red trunk ownership | P1 | I2709 | READY | open (ready, no open PR names it) |
| CI release runner contention | P1 | I2662 | READY | in_progress |
| Publish artifact with build | P1 | I2660 | READY | in_progress |
| Failed publish visible | P1 | I2661 | READY | in_progress |
| Published UI not rebuilt from nothing | P2 | I2663 | READY | in_progress |
| Branch protection provenance | P1 | I2282 | READY | open (ready, no open PR names it) |
| HTTP stack performance | P2 | I2488 | READY | open (ready, no open PR names it) |
| UI live stream | P1 | I2487 | READY | in_progress |
| UI route coverage | P1 | I2482 | READY | open (ready, no open PR names it) |
| UI server bootstrap | P2 | I2484 | READY | open (ready, no open PR names it) |
| Knowledge UI projection | P2 | I2652 | READY | in_progress |
| Kiosk evidence scenes | P2 | - | - | in_progress |
| Cluster dashboard | P2 | - | - | in_progress |
| Shared build-cache typed offer | P2 | - | - | in_progress |
| Landing page / customer narrative | P2 | - | - | in_progress |
| Second adopter findings | P1 | - | - | in_progress |
| Release tag grace | P1 | - | - | delivered (PR merged into master) |
| Board journal duplication | P1 | - | - | delivered (PR merged into master) |

Limits of this measurement:

- Measured on a shared Mac at load 80-120; no cargo build or test ran here (not_run), CI on master's head is the code verdict.
- worktree-topology gate timed out (420 s) on the loaded machine; not a verdict.
- Classification is git-derived (rev-list, cherry, merge-tree, ancestry) plus the repository's own classifier; titles were not read for it.
- Ownership is what the board and mesh claims said at recording time; a claim is advisory, not a lock.
