# Installed, with one skill written the way a repository writes its own: a directory under
# the skills section holding SKILL.md and one example, both tracked. Nothing registers it;
# the source class `skill` in the knowledge sources is what makes it exist.
. "$FIXTURE_SETUP/installed.sh"
mkdir -p .ai/repo/skills/release-notes/examples
cat > .ai/repo/skills/release-notes/SKILL.md <<'S'
---
schema: skill/v1
id: release-notes
version: 1
title: Release notes
description: Write the release notes for a tagged version from the ledger and the closed issues, never from memory.
status: active
tags: [release, documentation]
inputs:
  - the tag being released and the previous tag
outputs:
  - a release-notes section per closed issue, each naming its evidence
---
# Purpose

Turn what the ledger and the plan record into the notes a reader of the tag needs.

# Procedure

1. List the issues closed between the two tags with `majordomus plan list`.
2. For each, quote its outcome and the evidence attached to it.
3. Refuse to describe a change that no closed issue records.

# Output

One section per issue: title, what changed, the evidence, in plan order.
S
cat > .ai/repo/skills/release-notes/examples/notes-for-a-tag.md <<'S'
# Notes for one tag

```text
Apply the release-notes skill to the range v0.1.0..v0.2.0.
```
S
git add .ai/repo/skills && git commit -qm "a skill" >/dev/null
