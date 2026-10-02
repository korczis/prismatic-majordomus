# Open decisions and conservative defaults

Do not block the transformation for these unless implementation evidence forces
a choice. Use the conservative default below and record deviations.

## 1. Rule override semantics

**Decision for v1:** no override mechanism.

Baseline + project rules are additive.

Design metadata so a future explicit override policy is possible.

## 2. Multiple rule vendors

The directory shape permits:

```text
vendor/<name>/
```

but only Majordomus vendor support is required now.

Do not implement arbitrary remote vendor federation.

## 3. Worktree shared store

**Default:** do not add one.

Use each worktree's `.ai/local/` plus Git worktree discovery.

Only introduce machine-global state if a behavioral invariant demonstrably
cannot be preserved.

## 4. Skills/workflows

Create protocol directories/readmes only if useful for the initial skeleton.

Do not invent a large skill engine merely to populate the tree.

Move existing behavior there only when semantics are obvious.

## 5. Prompt parameterization

Reusable tracked prompts may remain compatible with the current renderer first.

A richer typed parameter schema is desirable later, but not required for
ownership migration.

Do not break current prompt rendering to chase a new templating language.

## 6. ADR format

Introduce `.ai/repo/adrs/` and a minimal README.

Do not mass-convert ordinary docs into ADRs.

If no current ADR records exist, do not fabricate architectural history.

## 7. Claims/responsibilities location

Keep:

```text
docs/CLAIMS.yaml
docs/RESPONSIBILITIES.yaml
```

where they are.

They are product/domain evidence sources, not automatically `.ai` configuration.

Knowledge can reference/index them.

## 8. Projection fingerprints

Do not weaken behavior.

Prefer self-describing target/region provenance if simple and robust.

Otherwise keep minimal tracked derived provenance under a well-documented
`.ai/repo` metadata location and schedule later simplification.

## 9. Policy decomposition

Do NOT split `.ai/repo/policy.yaml` during the first ownership migration.

Move it 1:1, prove parity, then decide whether a later data-driven split
actually removes complexity.

## 10. Tool packaging

Keep source distribution conventional (`bin/lib/share/...`).

The design must permit submodule/symlink/PATH/deb installs but this
transformation does not need to build a Debian package.
