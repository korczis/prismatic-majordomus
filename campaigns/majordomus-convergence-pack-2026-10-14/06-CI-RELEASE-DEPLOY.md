# Mission 06 — deterministic green trunk, CI throughput and release evidence

Map live workflow YAML and scripts; referenced issues: I2709 #1144, I2662 #1105, I2660 #1103, I2661 #1104, I2663 #1106, I2282 #1001. Historical fixes: #1145 release grace and #1143 branch cancellation; preserve semantics.

### CI lanes

Define cheap pre-push gates, focused PR tests, full trunk verification, release artifact validation, publication and downstream installation verification. Use measured wall time/queue times and cache hit statistics, not optimism. Scope only safe independent checks. Never make a cancelled/dispatch result count as required PR green. Branch protection must require correct workflow identity/sha/event context. Trunk failures designate single repair carrier; no unrelated merge except narrowly specified policy.

### Runner capacity

Reserve release and publication capacity; prevent PR validation from consuming all runners. Failed PR job can cancel sibling work early per existing policy; never cancel authoritative trunk/release/scheduled runs or hide a failure. New commit cancellation semantics must match current doctrine. Avoid runaway parallel cargo build thrash; use per-host resource limits and tracked caching.

### Release state machine

`candidate → tag → built artifacts → published artifacts → signed/checksummed manifest → release record → installation smoke test → deployment observed`. For each transition, record expected source SHA, binary version, target platform and evidence. Pending state is neither passed nor failed; bound grace clearly and escalate terminally. A missing release record must never permanently freeze unrelated trunk work, but cannot be called verified.

Use canonical distribution model `share/distribution.yaml` and generators; no per-workflow duplicate OS/platform list. Installer must SHA-256 verify artifact and refuse malicious archive contents; preserve previous installation on failure. Run installer smoke in clean target environments as available and label coverage gaps.

### GitHub Pages

Build once, publish its immutable artifact, observe published commit, detect stale/cancelled/deploy failure, show deploy age and reason across site/UI/CLI if supported. No claims until external URL has been verified against deployed artifact; never assume Actions success means live site was updated.

### Benchmarks

Measure p50/p95 feedback latency, queue wait, duration by job, sccache usage, cache misses, runner contention, deployment lead time, false red/green classification. CI policy should enforce budgets based on robust runner-aware sampling; one cold outlier not automatic regression.

Acceptance: a red trunk blocks ordinary landings; release is either fully proven or distinctly pending/failed; no runner starvation under modeled concurrent activity; every published artifact traced to validated source; no skipped required CI checks.
