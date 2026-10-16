# Mission 10 — external adoption proof, zero optional-service dependency

Ground in actual `docs/ADOPTION_SECOND_RUN.md` if present on live branch; prior audit found it in PR #1089 but fetch on master was 404. Consult PR branch if not merged; don't assert it is master. Follow reported fixes #1082 (personal init collision), #1083 (scope baseline), #1081 (health), #1092 (evidence subcommands), #1094 (region projection). Validate latest status before changes.

Build clean-room adoption matrix:
A. minimal bash+git repo;
B. existing Rust repo with managed AGENTS.md/CLAUDE.md;
C. pnpm TypeScript monorepo with hooks and integration branch;
D. Elixir/Phoenix repo if environment available;
E. non-mesh offline repo; F. optional mesh repo. Use disposable fixtures, not real third-party repositories without permission.

Workflow per adopter: install signed/hashed artifact → init with policy/profile inference → update projection without overwriting existing human content → doctor → start scoped task in a pre-dirty repo → change canonical file and derived projections → check negative and positive path → handover/resume → finish verification refused when test red → recover → finish valid → optional UI/restart/update/rollback.

Measure time to first verified outcome, user steps, unavoidable manual config edits, false blockers, crash, new files, generated diffs, cleanup, token overhead (only if actually observable). Issue every failure with exact minimal reproducible case; test/merge actual fixes.

Product positioning: show one concrete 'agent claimed done but verification refused' example. Default workflow should be installable and usable with minimal dependencies, without runtime mesh, minio, NPM, LLM API or network after installation. No hardcoded claim counts in public prose. Add quickstart with verified copy-paste commands and honest limitations.

Acceptance: all supported clean-room cases execute with evidence; any unsupported stack explicitly labeled; no broad beta claim while P0 safety/installation regressions remain unresolved.
