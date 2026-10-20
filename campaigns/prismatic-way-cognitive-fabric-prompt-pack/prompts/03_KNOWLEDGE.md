# WAVE 03 — Continuous learning using EXISTING evolve and knowledge facilities

Inspect `majordomus knowledge observe`, `derive --dry-run`, `candidates`, `promote`, `reject`, `check`, episode-end `session.knowledge_on_end`, and `.ai/repo/skills/evolve/SKILL.md`. Preserve current semantics: an observation belongs to an episode; candidates are unreviewed; curated knowledge is reviewed; `evolve` is the workflow to place lessons into rule/skill/test/ADR/curated locations, not a second knowledge store.

Extend existing episode lifecycle with bounded signals from completed tasks, successful verification, repeated friction, code review, and incidents. Detect duplicates and contradictions against existing curated content and rules before making candidates. Treat LLM-generated knowledge as proposed with evidence refs; do not promote merely because multiple models agree. Preserve promotion/rejection rationale and original source. Never automatically publish local state.

Optional auto-review is permitted only for explicit low-risk classes that the *current repository policy* allows, and only after deterministic evidence gates and audit/rollback. Otherwise produce review-ready batches and call the existing evolve workflow during closure. Build cycle protections: knowledge promotion must not re-trigger an endless candidate cascade.

Verify: same observation twice, replay, contradicting evidence, superseded ADR, near-duplicate candidate, unreviewed candidate, partial task, missing evidence, no active episode, and multi-peer race. Test current CLI commands and site projections remain canonical.
