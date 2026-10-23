# Unattended Hall LCD / Kiosk Operations

## Mode hierarchy
- `/ui/observatory`: full interactive Observatory with inspector and links.
- `/ui/kiosk`: same renderer, automatic scheduling, UI auto-hide, high-legibility composition, recoverable operator controls.
- `/ui/screensaver`: compatibility redirect only.
- `/ui/obsrvatory`: optional typo redirect; only if easy and tests pass.

## Installation constraints
Support 1920x1080 and 3840x2160 landscape, dynamic DPR caps, typography readable at 3–5 m, safe areas, overscan, large status glyphs, and low-noise branding. Avoid flashing and high brightness defaults. Add low-risk slow composition offsets to reduce persistent LCD image retention; do not promise burn-in prevention. Browser fullscreen requires gesture or genuine managed Chromium kiosk flags; do not spoof browser permissions.

## Session resiliency
- No busy loops or unbounded retry storms.
- Distinguish offline, disconnected, stale, no events and healthy idle.
- Implement streaming -> bounded cursor-poll fallback when canonical transport unavailable.
- Re-authenticate after token/policy changes; fail closed for unauthorized.
- Automatically recover from backend restart, network flap, browser sleep, WebGL context loss, scene exception, tab visibility changes and resizing.
- Display small, readable connection state, last verified observation and ongoing degraded status.
- Keep service worker/cache and browser asset updates coherent with repository conventions. No arbitrary self-reload that destroys capture state.

## Ops and DX
Document Linux systemd/Chromium kiosk and macOS launch options only as justified by existing packaging and supported environment. Prefer thin launch command in majordomus binary when supported; no new daemon solely for Observatory. Provide startup prerequisites, auth provisioning, privacy checklist, crash recovery, hardware sizing, measured resource budgets, and runbook with observable diagnostics.

## Art quality review
Capture real UI screenshots at 1080p and 4K and verify readable labels from a scaled-down thumbnail, non-overlapping overlays, stable composition, quiet-state aesthetics, no epileptogenic flashing, legible missing-data states and clear LIVE/REPLAY status. Art review is human check, not fake machine assertion.
