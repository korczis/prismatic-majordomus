# A repository that already has a hand-written CLAUDE.md people follow, with the two
# enforcements wired as hooks; Majordomus is installed, and the policy projects CLAUDE.md
# in region mode so that the authored text is never overwritten. Nothing is rendered yet.
"$MJ" init >/dev/null
cat > CLAUDE.md <<'MD'
# CLAUDE.md

Hand-written governance that predates Majordomus. Nothing here is generated.

## A rule the repository already had

Keep it.
MD
awk '/^projections:/{exit} {print}' .ai/repo/policy.yaml > policy.new
cat >> policy.new <<'YAML'
projections:
  - provider: agents
    target: AGENTS.md
    always_loaded: true
  - provider: claude-code
    target: CLAUDE.md
    mode: region
YAML
mv policy.new .ai/repo/policy.yaml
git add . && git commit -qm "authored governance"
# the hooks come after the commit: until the region is rendered, doctor refuses a
# CLAUDE.md that does not reach the layer, which is the point of the use case
mkdir -p .githooks
printf '#!/bin/sh\n%s doctor || exit $?\n' "$MJ" > .githooks/pre-commit
printf '#!/bin/sh\n%s finish --check || exit $?\n' "$MJ" > .githooks/pre-push
chmod +x .githooks/pre-commit .githooks/pre-push
git config core.hooksPath .githooks
