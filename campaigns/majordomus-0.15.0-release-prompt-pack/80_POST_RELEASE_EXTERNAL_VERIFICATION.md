# Milestone 8 — External verification

Goal: prove the release from a user's perspective, not the development worktree.

Prefer fresh temp directory/clean checkout/no local build PATH override.

Verify:
- tag/release metadata;
- expected target artifacts;
- checksums/metadata;
- release record commit/artifact linkage;
- public installer resolves new version;
- isolated install succeeds;
- installed binary reports released version;
- minimal supported flow works (`init`, `doctor`, `update` or current equivalents);
- public website/install/docs/changelog/release surfaces are reachable and current;
- intent docs no longer contradict shipped state;
- intent validate/preflight/verdict smoke works using repository-supported fixture/schema.

Do not use locally built binary as public-install proof.

Acceptance: fresh install works, version correct, public docs current, release record/changelog visible, intent smoke behaves as documented, no local release-only state is required.

Provide commands and summarized observed outputs.
