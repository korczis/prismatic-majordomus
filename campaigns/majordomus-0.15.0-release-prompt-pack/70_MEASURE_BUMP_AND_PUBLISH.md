# Milestone 7 — Measure, bump and publish

Goal: release through Majordomus' own measured-version architecture.

Run current equivalents:

```bash
majordomus release analyze
majordomus release analyze --explain
majordomus release analyze --format json
scripts/ci/version-matches-surface
```

Capture previous release, required bump, public additions/removals/narrowings/widenings and breaking markers.

Expected likely result: minor -> 0.15.0. Do not force it.

If PATCH, investigate why intended public surface change is not measured. If MAJOR/breaking, inspect exact compatibility movement and do not hide it.

Run canonical writer:

```bash
majordomus release bump
```

Do not hand-edit version declarations to obtain desired number.

Then derive/regenerate as required and rerun release gate/tests/pre-push checks.

Integrate release bump through current release workflow if separate branch/PR is required.

Use canonical tag/publication pipeline. Expected general flow: tag triggers release workflow, artifacts are built/published, hashes are produced, release record derives from real published artifacts, changelog is generated rather than manually authored.

Monitor release workflow to terminal success. Inspect failed job logs and fix root cause rather than re-running blindly.

Acceptance: measured bump explained, canonical writer used, tag/release exists, publication succeeds, artifacts published, release record/changelog projections current.
