# majordomus-covers: knowledge check
# majordomus-negative: knowledge check
# claims: knowledge-integrity
# A record's evidence is what was true when it was written. The file it cites may since have
# been deleted, and the record is no less true for that: what it names is in the history. So
# `knowledge check` asks git before it calls a `file:` or `test:` reference dangling, and
# says which references it resolved that way.
#
# What it must not become:
#
#   a check that cannot fail. A path no commit ever held is a typo or an invention; it is
#   refused exactly as before, bare or pinned to a commit that never held it.
#
#   a silent pass. A reference resolved through the history is reported as historical, with
#   the commit, so a reader knows to look with git and not with ls.
#
#   a second opinion. The Rust executable resolves the same references the same way.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH
CAND=.ai/repo/knowledge/candidates
STATE=.ai/local/state
mkdir -p "$CAND" tests

# two files a record will cite, committed, and the commit that held them
echo 'engine' > tests/engine.cjs; echo 'smoke' > tests/smoke.cjs; echo 'kept' > tests/kept.cjs
git add -A >/dev/null; git commit -qm "the layer and three test files" >/dev/null
HELD="$(git rev-parse HEAD)"

"$MJ" session start >/dev/null
SID="$(sed -n 's/^session_id: //p' "$STATE/session-current.yaml" | head -n 1)"
[ -n "$SID" ] || { echo "    no open episode to reference"; exit 1; }

# record ID REF... — one candidate whose derived_from names the episode and each REF
record() {
  local id="$1" r; shift
  {
    printf -- '---\nschema: knowledge/v1\nid: %s\nkind: knowledge\nclass: convention\n' "$id"
    printf 'title: "The engine is covered by two files"\ndescription: "Hand-made for the provenance check."\n'
    printf 'status: candidate\nepistemics: decided\ndate: 2026-10-13\ntags:\n  - derived\n  - decision\n'
    printf 'provenance:\n  origin: extracted\n  derived_from:\n    - session:%s\n' "$SID"
    for r in "$@"; do printf '    - %s\n' "$r"; done
    printf -- '---\n\n# The engine is covered by two files\n\nHand-made for the provenance check.\n'
  } > "$CAND/$id.md"
}
clear_store() { find "$CAND" -maxdepth 1 -name '*.md' ! -name README.md -delete; git add -A .ai/repo/knowledge >/dev/null; }

# ---------------------------------------------------------------- while the files exist: nothing historical
record cites-files file:tests/engine.cjs test:tests/smoke.cjs file:tests/kept.cjs
git add -A >/dev/null
expect_exit 0 "$MJ" knowledge check
expect_grep '^knowledge check: 1 record\(s\), 0 failing$'
expect_no_grep 'historical'

# ---------------------------------------------------------------- the files are deleted: historical, not dangling
git rm -q tests/engine.cjs tests/smoke.cjs; git commit -qm "V2 removes the old tests" >/dev/null
GONE="$(git rev-parse --short=10 HEAD)"
expect_exit 0 "$MJ" knowledge check
expect_grep '^OK +knowledge +.*cites-files\.md'
expect_grep "^INFO +knowledge +.*cites-files\.md — provenance\.derived_from\.1 names \`file:tests/engine\.cjs\`, which the tree no longer holds and commit $GONE does: historical, not dangling"
expect_grep "^INFO +knowledge +.*names \`test:tests/smoke\.cjs\`, which the tree no longer holds and commit $GONE does"
expect_no_grep 'tests/kept\.cjs'
expect_grep '^knowledge check: 1 record\(s\), 0 failing, 2 reference\(s\) resolved in history$'
# the same, for a program
out="$("$MJ" knowledge check --json)"
[ "$(jq -r '.failing' <<<"$out")" = 0 ] || { echo "    --json reports a failing record: $out"; exit 1; }
[ "$(jq -r '[.historical[] | .reference] | sort | join(" ")' <<<"$out")" = "file:tests/engine.cjs test:tests/smoke.cjs" ] \
  || { echo "    --json does not list the historical references: $out"; exit 1; }
[ "$(jq -r '.historical[0].commit' <<<"$out")" = "$GONE" ] || { echo "    --json names another commit: $out"; exit 1; }
clear_store

# ---------------------------------------------------------------- pinned to the commit that held it
record cites-a-commit "file:tests/engine.cjs@$HELD" "file:tests/engine.cjs@${HELD:0:10}"
git add -A >/dev/null
expect_exit 0 "$MJ" knowledge check
expect_grep "names \`file:tests/engine\.cjs@$HELD\`, which the tree no longer holds and commit $HELD does"
expect_grep '0 failing, 2 reference\(s\) resolved in history$'
clear_store

# ---------------------------------------------------------------- what never existed still fails
record never-existed file:tests/no-such-file.cjs
git add -A >/dev/null
expect_exit 10 "$MJ" knowledge check
expect_grep 'FAIL +knowledge .*never-existed.*`file:tests/no-such-file\.cjs`, which resolves to nothing'
clear_store
# pinned to a commit that never held the path: the pin is not a pass
record wrong-commit "file:tests/no-such-file.cjs@$HELD"
git add -A >/dev/null
expect_exit 10 "$MJ" knowledge check
expect_grep 'FAIL +knowledge .*wrong-commit.*which resolves to nothing'
clear_store
# a path that climbs out of the repository is never asked of git
record climbs-out file:../outside.txt
git add -A >/dev/null
expect_exit 10 "$MJ" knowledge check
expect_grep 'FAIL +knowledge .*climbs-out'
clear_store
# a file that lives only on another branch does not vouch for a record here
git checkout -q -b elsewhere; echo x > tests/only-elsewhere.cjs; git add -A >/dev/null; git commit -qm elsewhere >/dev/null
git checkout -q -
record cites-another-branch file:tests/only-elsewhere.cjs
git add -A >/dev/null
expect_exit 10 "$MJ" knowledge check
expect_grep 'FAIL +knowledge .*cites-another-branch.*which resolves to nothing'
clear_store

# ---------------------------------------------------------------- the Rust executable agrees
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
record cites-files file:tests/engine.cjs file:tests/no-such-file.cjs "file:tests/smoke.cjs@$HELD"
git add -A >/dev/null; git commit -qm "a record for both programs" >/dev/null
out="$("$RB" knowledge record cites-files --format json 2>/dev/null)" \
  || { echo "    the executable could not read the record"; "$RB" knowledge record cites-files 2>&1 | tail -3; exit 1; }
res() { jq -r --arg r "$1" '[.. | objects | select(.reference? == $r) | .resolution] | first // "absent"' <<<"$out"; }
[ "$(res file:tests/engine.cjs)" = external ] || { echo "    the executable calls a deleted file $(res file:tests/engine.cjs)"; exit 1; }
[ "$(res "file:tests/smoke.cjs@$HELD")" = external ] || { echo "    the executable calls a pinned file $(res "file:tests/smoke.cjs@$HELD")"; exit 1; }
[ "$(res file:tests/no-such-file.cjs)" = missing ] || { echo "    the executable calls a file that never existed $(res file:tests/no-such-file.cjs)"; exit 1; }
exit 0
