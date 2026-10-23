# Deterministic Director + scene lifecycle contract

## Algorithm, not a suggestion
For each eligible scene compute `priority = base_weight + bounded_signal_relevance + fairness_debt - repetition_penalty - resource_penalty`. Exact coefficients belong in canonical versioned policy, not scattered UI constants. Critical operational interrupt is a separate priority class and does not compete with normal art rotation. Weighting is explainable and subject to deterministic tests.

Eligibility = required source available **and** its data fit-for-purpose; `connected` alone insufficient. Scenes can be `FULL`, `PARTIAL`, `AMBIENT_ONLY`, `UNAVAILABLE`. If NONE has valid operational signals, select explicitly labeled ambient/no-signal state.

Minimum dwell, maximum dwell, signal cooldown and recovery hysteresis protect against flashing. Ensure fair rotation: test that any continuously eligible scene is shown at least once within a bounded scheduling horizon under bounded normal event activity. Critical alerts may preempt, but their visible diagnostic status remains even as gallery continues.

## Lifecycle invariants
At most one active expensive renderer per viewport, transition double-buffer only within memory budget. `prepare → mount → activate → suspend → dispose` with idempotent cleanup, cancellation of subscriptions, timers and GPU allocations. Never create a backend stream per scene. Repeated rotations must not increase listeners/subscriptions/memory asymptotically.

## Inspector and manual control
Keyboard/manual lock overrides normal rotation but not mandatory safety notices. Director `explain` returns eligibility, weights, cause for switch, last shown, next candidates and source quality. Persist only appropriate non-sensitive local preferences.

## Deterministic test cases
Same sequence/capability snapshots/seed => identical schedule and scene transforms; replay with distinct refresh rate => identical semantic event counts; unavailable source => no fake activity; repeated incident => bounded interruption; scene failure => isolated failure and fallback; all available scenes => no starvation; quality reduction never alters semantic counters.
