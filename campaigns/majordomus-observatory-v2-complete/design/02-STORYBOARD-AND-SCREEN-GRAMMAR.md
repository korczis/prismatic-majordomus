# Art direction V2 — physical installation storyboard

## Screen composition
Large-screen hierarchy: 70–80% generative visual field; 10–20% restrained narrative context; optional status strip for source freshness, LIVE / RECORDED REPLAY / NO SIGNAL and scene title. Avoid small dense text. Display modes 1920×1080 and 3840×2160, plus portrait adaptability if evidence supports it. Art is art; operational semantics remain inspectable.

## Scene sketches (conceptual, not fake screenshots)
```
MESH COSMOS                    EVENT RIVER
       o       o              >>>>  . .  >>>
   o -- O -- o                 ╲═══════╱
        |   o                    event lanes
        o                     Count / window / sources

INTENT NEBULA                  SYSTEM PULSE
  Intent                      .--.  --.-  .---
   ├ Milestone                  /  \/    \/
   │  └ Issue                 health / latency / events
   └ Evidence                labels + units + freshness

DELIVERY TREE                  COOP CHOREOGRAPHY
    release                       agent A
    /    \                         │ confirmed event
 tests  evidence                   ▼
    |       |                      agent B
 commits  gates             no inferred communications
```
**Do not show edges or metrics without records.** Scene may animate idle geometry only if operational meaning is explicitly absent.

## Screen grammar
- Colors convey semantics by canonical design token: normal activity, degraded, critical, unavailable, replay. No fresh semantic palette in JS.
- Motion is layered: ambient (decorative), operational (source-backed), attention (warning), transition (director). Never use warning color randomly as decoration.
- Every main scene has a fixed camera-safe title/legend zone and optional inspector; rotate/burn-in shift gently without moving status offscreen.
- Director crossfade 0.8–2.0s initial design hypothesis, tune on reference hardware. Interruptions prioritize readability over spectacle.
- Legibility walk test: observer at 3–5 meters identifies scene, LIVE vs REPLAY, mesh health in ≤30s, and never mistakes no-signal for normal.
- Respect reduced-motion and WCAG-relevant controls; provide non-canvas data alternative from same projection.

## Art review gates
Capture actual running screenshots AND 15–30 second recordings of live, idle, partial/stale, recorded replay, source outage. Review at hall distance on realistic 4K and 1080p setup, including night brightness. Record actual notes and corrective changes. Source audit and visual review are separate gates; attractive pixels cannot rescue dishonest data.
