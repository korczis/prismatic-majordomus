# majordomus-covers: init update checkpoint session doctor
# A record names the work, and the declaration it is filtered by is read whole.
#
# Case 136 proved that `changed_files` excludes what the five declarations call derived. It
# proved it with declarations written in three shapes — a plain path, a trailing `/`, a
# trailing `/**` — and those were the only three shapes the reader could match. Anything
# else it compared literally, so it excluded nothing at all, silently, and three
# declarations in this repository are written in shapes it could not read:
#
#   site/content/*.md                  a glob in the last segment (.gitattributes)
#   .ai/repo/benchmarks/**/baseline.*  a `**` in the middle (scope.yaml out.generated.paths)
#   **/target/ and eight siblings      a `**/` prefix (scope.yaml out.paths)
#
# The first two name tracked files that a generator rewrites — the site's own index page and
# a ratchet baseline — so a record written after a derive claimed both as the episode's
# work. This case declares each of the three shapes and proves the record follows it, which
# is the part a shape-by-shape reader cannot pass.
#
# It also holds the two halves that make the filter honest: a path nothing declares is still
# named, and the number of paths left out is reported rather than silently dropped.
. "$ROOT/test/lib.sh"

must() { local why="$1"; shift; "$@" || { printf '    %s (failed: %s)\n' "$why" "$*"; return 1; }; }
listed() { awk '/^changed_files:$/ { f = 1; next } f && /^  - / { sub(/^  - /, ""); print; next } f { exit }' "$1"; }

"$MJ" init >/dev/null; "$MJ" update >/dev/null

# The three shapes, one per declaration that carries it. None of these trees is ignored:
# git never reports an ignored path as changed, so an ignored fixture would pass this case
# without the classifier doing anything at all.
sed -i.bak "s|^    paths: \[\]$|    paths: ['bench/**/baseline.*']|" .ai/repo/scope.yaml
rm -f .ai/repo/scope.yaml.bak
grep -q "bench/\*\*/baseline" .ai/repo/scope.yaml || { echo "    the scope fixture did not apply"; exit 1; }
printf 'gen/*.json          merge=derived\n**/stamp.txt        merge=derived\n' > .gitattributes

mkdir -p gen bench/run pkg/sub lib
printf 'seed\n' | tee gen/a.json bench/run/baseline.json pkg/sub/stamp.txt lib/work.sh >/dev/null
git add -A && git commit -qm base

dirty() {
  printf 'x\n' >> lib/work.sh            # work: nothing declares it
  printf 'x\n' >> gen/a.json             # a glob in the last segment
  printf 'x\n' >> bench/run/baseline.json # a ** in the middle
  printf 'x\n' >> pkg/sub/stamp.txt      # a **/ prefix
}
dirty

# ---------------------------------------------------------------- a checkpoint
printf 'progress\n' | "$MJ" start "classify" --scope lib >/dev/null
# The path comes from the command, never from `find | sort | tail`: three checkpoints of
# one case can share a timestamp to the second, and then name order is the digest's and the
# case reads whichever record it happens to pick. That flakiness cost a mutation verdict
# before it was noticed.
CP="$(printf 'a note\n' | "$MJ" checkpoint)"
expect_file "$CP"
must "the work is named"                    grep -q '^  - lib/work.sh$' "$CP"
must "a last-segment glob is not"           [ "$(listed "$CP" | grep -c '^gen/')" = 0 ]
must "nor a ** in the middle"               [ "$(listed "$CP" | grep -c '^bench/')" = 0 ]
must "nor a **/ prefix"                     [ "$(listed "$CP" | grep -c 'stamp.txt$')" = 0 ]

# ---------------------------------------------------------------- a closed session
"$MJ" session start >/dev/null
printf 'summary\n' | "$MJ" session close >/dev/null
REC="$(grep -l '^session_id: ' .ai/repo/sessions/*.md | head -n 1)"
expect_file "$REC"
must "the record names the work"            grep -q '^  - lib/work.sh$' "$REC"
must "and not a last-segment glob"          [ "$(listed "$REC" | grep -c '^gen/')" = 0 ]
must "nor a ** in the middle"               [ "$(listed "$REC" | grep -c '^bench/')" = 0 ]
must "nor a **/ prefix"                     [ "$(listed "$REC" | grep -c 'stamp.txt$')" = 0 ]

# ---------------------------------------------------------------- said out loud
# A filter that shortens a list without saying so is indistinguishable from a tree that was
# cleaner than it was. The count goes to stderr, never into the record's front matter.
dirty
CP2="$(printf 'a second note\n' | "$MJ" checkpoint 2>note.err)"
expect_grep 'derived or checkout-local path\(s\) classified out of changed_files' note.err
expect_file "$CP2"
must "and the count is not smuggled into the record" \
  [ "$(grep -c 'classified out' "$CP2")" = 0 ]

# ---------------------------------------------------------------- declaration-driven
# The reader has no list of its own: remove the declaration and the path is work again.
: > .gitattributes
dirty
CP3="$(printf 'a third note\n' | "$MJ" checkpoint 2>/dev/null)"
expect_file "$CP3"
must "an undeclared path is named like any other work" \
  grep -q '^  - gen/a.json$' "$CP3"
must "while one the scope file still declares stays out" \
  [ "$(listed "$CP3" | grep -c '^bench/')" = 0 ]

# ---------------------------------------------------------------- absence is not damage
# The two record stores are checkout-local and created on demand, by their writers and by
# `majordomus update`. Reporting their absence as a finding gave every new worktree two
# standing warnings that no action clears; the condition that matters — episodes opened and
# the writers stopped — is `mj_validate_lifecycle`'s, and is conditioned on activity.
rm -rf .ai/local/state/handovers
"$MJ" doctor > doctor.out 2>&1 || true
must "an absent checkout-local store is not a finding" \
  [ "$(grep -c 'WARN .*layout .*state/handovers' doctor.out)" = 0 ]
expect_grep 'layout +.*state/handovers .* absent' doctor.out
must "a present one says how much it holds" \
  grep -qE 'layout +.*state/checkpoints .* present, [0-9]+ record' doctor.out

# The tracked section keeps its finding: the manifest names it and nothing creates it.
rm -rf .ai/repo/prompts
"$MJ" doctor > doctor2.out 2>&1 || true
expect_grep 'WARN +layout +.*prompts .* missing' doctor2.out

echo "  284 ok"
