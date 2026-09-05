# Installed, then a section added under the layer the way a person adds one: a directory
# with its contract, and a directory below it with data and no contract at all. Discovery
# is over the tracked tree, so the commit is what puts both in front of validation.
. "$FIXTURE_SETUP/installed.sh"
mkdir -p .ai/repo/zones/deep
cat > .ai/repo/zones/README.md <<'DOC'
---
schema: context/v1
id: ai.repo.zones
kind: context
title: Zones
description: What a zone is and what belongs in one.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Zones

A zone is this repository's own kind; the directory says what belongs in it.
DOC
printf 'name: one\n' > .ai/repo/zones/deep/one.yaml
git add . && git commit -qm "zones: a section with a contract, and one directory without"
