# majordomus-covers: session checkpoint handover
# majordomus-negative: session
# `changed_files` names the work, not the tool's exhaust.
#
# Every record Majordomus writes carried `git status --porcelain` verbatim. That is the
# dirty tree and not what the episode did: running the tool dirties the tree. The record at
# .ai/repo/sessions/20260910T103933Z--s-20260909152316-024f--… carries 122 changed files,
# of which the great majority are docs/generated/, site/data/generated/ and
# site/content/ — and three of them are that episode's own sibling records, and one is its
# own site projection. A record naming itself as its own work product is what this case
# exists to make impossible.
#
# The classifier invents no list. It reads the declarations that already say what is
# derived, and this case sets each of them in turn and proves that the record follows it —
# so a repository that declares nothing loses nothing, and one that adds a declaration gets
# the effect without anybody editing lib/changed.sh.
. "$ROOT/test/lib.sh"

must() { local why="$1"; shift; "$@" || { printf '    %s (failed: %s)\n' "$why" "$*"; return 1; }; }
# The changed_files block of a record, one path per line.
listed() { awk '/^changed_files:$/ { f = 1; next } f && /^  - / { sub(/^  - /, ""); print; next } f { exit }' "$1"; }

"$MJ" init >/dev/null; "$MJ" update >/dev/null

# Four declarations of the five, each set here so that it is proved rather than assumed
# present (the fifth, the record stores, needs no fixture: the tool writes them). The
# skeleton ships `out.generated.paths: []`, because the tool cannot know where a repository
# puts its generated trees — which is exactly why the classifier reads the declaration
# instead of carrying a list.
#   scope.yaml out.generated.paths   declared below: docs/generated/
#   scope.yaml out.paths             .ai/local/, already in the skeleton
#   policy projections[].target      CLAUDE.md and AGENTS.md, written by `update`
#   .gitattributes merge=derived     declared below: site/data/generated/**
sed -i.bak "s|^    paths: \[\]$|    paths: ['docs/generated/']|" .ai/repo/scope.yaml
rm -f .ai/repo/scope.yaml.bak
grep -q "docs/generated/" .ai/repo/scope.yaml || { echo "    the scope fixture did not apply"; exit 1; }
printf 'site/data/generated/**            merge=derived\n' > .gitattributes

mkdir -p docs/generated site/data/generated site/content lib
printf 'seed\n' | tee docs/generated/registry.json site/data/generated/a.json \
  site/content/page.md lib/work.sh > /dev/null
git add -A && git commit -qm base

# One file of each kind, dirtied together.
printf 'x\n' >> lib/work.sh              # real work
printf 'x\n' >> site/content/page.md     # real work: nothing declares site/content/ derived
printf 'x\n' >> docs/generated/registry.json   # scope.yaml out.generated.paths
printf 'x\n' >> site/data/generated/a.json     # .gitattributes merge=derived
printf 'x\n' >> CLAUDE.md                # a projection target
printf 'x\n' >> AGENTS.md                # a projection target

# ---------------------------------------------------------------- a checkpoint
printf 'progress\n' | "$MJ" start "classify" --scope lib >/dev/null
printf 'a note\n' | "$MJ" checkpoint >/dev/null
CP="$(find .ai/local/state/checkpoints -name '*.md' | head -n 1)"
expect_file "$CP"
must "the work is named"            grep -q '^  - lib/work.sh$' "$CP"
must "and so is undeclared content" grep -q '^  - site/content/page.md$' "$CP"
must "a generated tree is not"      [ "$(listed "$CP" | grep -c '^docs/generated/')" = 0 ]
must "nor one the merge driver declares" [ "$(listed "$CP" | grep -c '^site/data/generated/')" = 0 ]
must "nor a provider projection"    [ "$(listed "$CP" | grep -c '^CLAUDE.md$')" = 0 ]
must "nor another one"              [ "$(listed "$CP" | grep -c '^AGENTS.md$')" = 0 ]
must "nor this checkout's own state" [ "$(listed "$CP" | grep -c '^\.ai/local/')" = 0 ]

# ---------------------------------------------------------------- a handover
printf '# Objective\nfix it\n# Current State\nhalf done\n# Next Action\nfinish it\n' | "$MJ" handover >/dev/null
HO="$(find .ai/local/state/handovers -name '*.md' | head -n 1)"
expect_file "$HO"
must "the handover names the work"        grep -q '^  - lib/work.sh$' "$HO"
must "and not the generated trees"        [ "$(listed "$HO" | grep -c 'generated/')" = 0 ]
must "and not the checkpoint it followed" [ "$(listed "$HO" | grep -c '/checkpoints/')" = 0 ]

# ---------------------------------------------------------------- a closed session
"$MJ" session start >/dev/null
printf 'summary\n' | "$MJ" session close >/dev/null
REC="$(grep -l '^session_id: ' .ai/repo/sessions/*.md | head -n 1)"
expect_file "$REC"
must "the record names the work"    grep -q '^  - lib/work.sh$' "$REC"
must "and the undeclared content"   grep -q '^  - site/content/page.md$' "$REC"
must "and not the generated trees"  [ "$(listed "$REC" | grep -c 'generated/')" = 0 ]
must "and not the projections"      [ "$(listed "$REC" | grep -c '^CLAUDE.md$\|^AGENTS.md$')" = 0 ]
# the finding that named this case: a record must never name its own store
must "and never a session record"   [ "$(listed "$REC" | grep -c '\.ai/repo/sessions/')" = 0 ]
must "nor a checkpoint"             [ "$(listed "$REC" | grep -c '/checkpoints/')" = 0 ]
must "nor a handover"               [ "$(listed "$REC" | grep -c '/handovers/')" = 0 ]

# ---------------------------------------------------------------- declaration-driven
# The whole point: the classifier has no list of its own. Remove the declaration and the
# path comes back, which is what proves it was the declaration that excluded it and not a
# constant in lib/changed.sh.
: > .gitattributes
printf 'x\n' >> site/data/generated/a.json
"$MJ" session start --provider-session second >/dev/null
printf 'summary\n' | "$MJ" session close --provider-session second >/dev/null
REC2="$(grep -rl '^title: "Session' .ai/repo/sessions/*.md | while IFS= read -r f; do
  [ "$f" = "$REC" ] || printf '%s\n' "$f"; done | head -n 1)"
expect_file "$REC2"
must "an undeclared generated tree is named like any other work" \
  grep -q '^  - site/data/generated/a.json$' "$REC2"
must "while one the scope file still declares stays out" \
  [ "$(listed "$REC2" | grep -c '^docs/generated/')" = 0 ]

echo "  136 ok"
