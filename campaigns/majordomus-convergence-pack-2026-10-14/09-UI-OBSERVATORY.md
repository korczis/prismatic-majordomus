# Mission 09 — reliable derived cockpit, p5 Observatory and production UI

Issues I2487 #1074, I2482 #1035, I2484 #1037, I2652 #1091 and PR #1124 kiosk, #1111 dashboard, #1098 landing. Re-check actual merged vs pending state.

- Treat UI as a projection of existing typed capabilities, not a new source of state. Same IDs, same semantic statuses and source citations/links. One collection order defined upstream, explicit user sort/filters local only.
- Confirm route inventory and canonical names (`/ui`, `/ui/kiosk`, `/ui/observatory` etc.) from actual router; no fake route. Make every screen reach its owning capability and reproducible evidence detail.
- Build one event stream contract with per-execution cursor/resume semantics, dedup, reconnect/jitter, backpressure, heartbeats and stale indicator. Never fabricate live data; recover from snapshot on gap and explain freshness.
- UI performance: first render, p95 route read, client memory, long-run 4K full-screen, mobile widths, reduced motion, high contrast and accessible keyboard navigation. Ensure no browser-only business state duplicates Rust/server.
- Observatory display provenance classes explicitly: observed/derived/inferred/planned/stale/unknown with schema compatibility, confidence methodology where inferred. On empty data show unavailable with reason, never demo numbers.
- p5.js must be pinned, lazy loaded from approved local asset derivation, instance-mode isolation, offscreen composition and graceful no-p5 fallback. Don't inject CDN scripts next to state-changing routes. CSP and security take priority over animation.
- Test every scene's count model against exact canonical capability response with generated fixtures. Screenshot tests at 1080p/4K and at target mobile widths; verify reduced-motion and browser resource survival after hours (test period limited to what was actually run).
- Navigation and docs/marketing claims cross-linked to evidence. Do not promise interactive live kiosk on static Pages unless backed by real data or explicitly snapshot-labeled.

Acceptance: no broken routes or links, typed streamed truth, reconnection correctness, predictable mobile/4K layout, no security regression; UI cannot turn unknown into green.
