# majordomus-covers: none
# claims: a-batch-names-its-members
# majordomus-timeout: 600
# A composed branch that is not a batch is refused (ADR 0114 D5; rule
# project.integration-follows-the-current-master, version 4).
#
# On 2026-10-07 and 2026-10-08 five `int/batch-*` branches were built by hand: `git merge
# --no-ff` of each member's head, then fixes written on the batch branch itself. Four of the
# five carried such fixes, none recorded which members rode at which heads, and nothing in CI
# could tell one of them from a branch of ordinary work. The previous slice's gate then
# counted a batch by its merges alone, so a batch whose member had moved since it was
# composed merged "fewer than two other open pull requests" and passed as not a batch, with
# a manifest that named a member nothing carried.
#
# What this holds, through the real command line (`majordomus prs batch-check`) in a fixture
# repository with a bare origin and a scripted `gh`:
#
#   1. a branch of ordinary commits is not a batch, and git alone says so: the forge is not
#      asked
#   2. a hand-built branch merging the heads of two open pull requests, with no manifest, is
#      refused (exit 10) naming both members and their heads
#   3. the same branch with the manifest that names those merges, in order, passes
#   4. one authored commit on the batch is refused naming the commit's path
#   5. a manifest that lists the members the other way round is refused naming the line
#   6. a member whose owner pushed since is no longer what the batch carries: the manifest
#      makes the branch a batch to be judged although one member merge is left, and the
#      refusal names the member the manifest still lists
#   7. the gate asks the forge for the open heads and nothing else, and when it cannot read
#      them it cannot run (exit 12, nothing on stdout): never clean because it could not look
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-1000.git"; W="$T/../work-1000"; STATE="$T/../forge-1000"; BIN="$T/../bin-1000"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b main "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
git config user.email t@example.com; git config user.name t
echo base > a.txt
gitq add -A; gitq commit -qm base; gitq push -q origin HEAD:main
M0="$(git rev-parse HEAD)"
branch() {   # <name> <file>: one commit on the base, pushed
  gitq checkout -q -b "$1" "$M0"; echo "$1" > "$2"
  gitq add -A; gitq commit -qm "$1"; gitq push -q origin "HEAD:refs/heads/$1"
}
branch feature/1 one.txt
branch feature/2 two.txt
branch feature/3 three.txt
H1="$(git rev-parse feature/1)"; H2="$(git rev-parse feature/2)"
gitq fetch -q origin

# the scripted forge: the open pull requests are the branches of prs.tsv at the heads origin
# serves; with $STATE/down it answers nothing. Anything but the two reads is logged as such.
printf '%s\t%s\n' 1 feature/1  2 feature/2  3 feature/3 > "$STATE/prs.tsv"
: > "$STATE/log"
cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
if [ -f "$STATE/down" ]; then echo 'HTTP 401: Bad credentials' >&2; exit 1; fi
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"main"}}' ;;
  "pr list")
    printf '['; sep=''
    while IFS='	' read -r n br; do
      h="\$(git -C "$ORIGIN" rev-parse "refs/heads/\$br")"
      printf '%s{"number":%s,"title":"change %s","headRefName":"%s","headRefOid":"%s","baseRefName":"main","isCrossRepository":false}' \
        "\$sep" "\$n" "\$n" "\$br" "\$h"
      sep=','
    done < "$STATE/prs.tsv"
    printf ']\n' ;;
  *) echo "WRITE-OR-UNEXPECTED \$*" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
check() { "$RB" prs --repo "$W" batch-check --base origin/main --head "$@"; }
asked() { grep -c '^pr list' "$STATE/log" || true; }
# a manifest as `prs compose` writes one: <id> then "<number> <head> <merge commit>" lines
manifest() {
  printf "schema: integration-batch/v1\nid: '%s'\nbase: main\nbase_master: '%s'\ncomposed_at: '2026-10-08T10:00:00Z'\nmembers:\n" "$1" "$M0"
  shift
  while [ $# -gt 0 ]; do
    printf "  - number: %s\n    head: '%s'\n    title: 'change %s'\n    merge_commit: '%s'\n" "$1" "$2" "$1" "$3"
    shift 3
  done
}
short() { printf '%s' "$1" | cut -c1-10; }

# ---------------------------------------------------------------- 1. ordinary work
expect_exit 0 check "$H1"
expect_grep '^batch-check: not a batch'
expect_grep 'decided by git alone'
[ "$(asked)" = 0 ] || { echo "    the forge was asked about a branch git alone decides:"; cat "$STATE/log"; exit 1; }

# ---------------------------------------------------------------- 2. built by hand
gitq checkout -q -b int/hand "$M0"
gitq merge -q --no-ff -m 'merge #1' "$H1"; C1="$(git rev-parse HEAD)"
gitq merge -q --no-ff -m 'merge #2' "$H2"; C2="$(git rev-parse HEAD)"
HAND="$C2"
expect_exit 10 check "$HAND"
expect_grep '^batch-check: REFUSED: .* merges #1, #2 and is not a batch'
expect_grep 'no manifest under \.ai/repo/integration/batches/ is added'
expect_grep "#1 at $(short "$H1")"
expect_grep "#2 at $(short "$H2")"
[ "$(asked)" = 1 ] || { echo "    the forge was asked $(asked) time(s) for one judgment, not once"; exit 1; }
expect_exit 10 check "$HAND" --format json
[ "$(printf '%s' "$LAST_OUT" | jq -r '[.verdict, .forge_read, ([.members[].number] | join(",")), .findings[0].kind] | join(" ")')" = "refused true 1,2 manifest_missing" ] \
  || { echo "    the report of the hand-built batch is: $LAST_OUT"; exit 1; }

# ---------------------------------------------------------------- 3. with its manifest
ID="$(short "$M0")-1-2"
MANIFEST=".ai/repo/integration/batches/$ID.yaml"
mkdir -p "$(dirname "$MANIFEST")"
manifest "$ID" 1 "$H1" "$C1" 2 "$H2" "$C2" > "$MANIFEST"
gitq add -A; gitq commit -qm 'the composition'
BATCH="$(git rev-parse HEAD)"
expect_exit 0 check "$BATCH"
expect_grep "^batch-check: ok: .* is the batch $MANIFEST says: #1, #2\$"
expect_exit 0 check "$BATCH" --format json
[ "$(printf '%s' "$LAST_OUT" | jq -r '[.verdict, .manifest, (.findings | length | tostring)] | join(" ")')" = "batch $MANIFEST 0" ] \
  || { echo "    the report of the batch is: $LAST_OUT"; exit 1; }

# ---------------------------------------------------------------- 4. a fix on the batch branch
echo 'a fix no member saw' > fix.txt
gitq add -A; gitq commit -qm 'fix on the batch'
FIX="$(git rev-parse HEAD)"
expect_exit 10 check "$FIX"
expect_grep "commit $(short "$FIX") \(fix on the batch\) changes fix\.txt on the batch branch"
expect_grep "a fix is written on the member's branch"
[ "$(printf '%s\n' "$LAST_OUT" | grep -c '^  ')" = 1 ] || { echo "    one authored path is not one finding:"; printf '%s\n' "$LAST_OUT"; exit 1; }

# ---------------------------------------------------------------- 5. members the other way round
gitq checkout -q -b int/reordered "$HAND"
mkdir -p "$(dirname "$MANIFEST")"
manifest "$ID" 2 "$H2" "$C2" 1 "$H1" "$C1" > "$MANIFEST"
gitq add -A; gitq commit -qm 'the composition, reordered'
expect_exit 10 check "$(git rev-parse HEAD)"
expect_grep "$MANIFEST:7 lists #2 as member 1, and the merge at that position on the first-parent line is #1's"

# ---------------------------------------------------------------- 6. a member moved
# #2's owner pushes. One member merge is left on the batch; the manifest still names two.
gitq checkout -q feature/2
echo more > two-b.txt; gitq add -A; gitq commit -qm 'the owner again'
gitq push -q origin HEAD:refs/heads/feature/2
expect_exit 10 check "$BATCH"
expect_grep "^batch-check: REFUSED: .* adds or changes $MANIFEST, merges #1 and is not the batch that manifest says"
expect_grep "$MANIFEST:11 names member #2, and no merge on the first-parent line has #2's current head"
# and what the stale merge brought is no member's any more
expect_grep 'changes two\.txt on the batch branch'
# the hand-built branch with no manifest is now a stack of one, which is not this gate's
expect_exit 0 check "$HAND"
expect_grep '^batch-check: not a batch'

# ---------------------------------------------------------------- 7. only the heads, and never clean unread
if grep -q 'WRITE-OR-UNEXPECTED' "$STATE/log"; then echo "    the gate asked the forge for more than the open heads:"; grep WRITE "$STATE/log"; exit 1; fi
if grep -v '^pr list --state open ' "$STATE/log" | grep -q .; then echo "    the gate asked the forge for more than the open heads:"; grep -v '^pr list' "$STATE/log"; exit 1; fi
grep -q -- '--json number,headRefOid,isCrossRepository$' "$STATE/log" \
  || { echo "    the list asks for more than the number and the head:"; head -1 "$STATE/log"; exit 1; }
: > "$STATE/down"
got=0; out="$(check "$BATCH" 2>"$STATE/err")" || got=$?
[ "$got" = 12 ] || { echo "    an unreadable forge exited $got, not 12: $out"; cat "$STATE/err"; exit 1; }
[ -z "$out" ] || { echo "    a gate that could not run printed a verdict: $out"; exit 1; }
grep -q 'batch-check cannot run' "$STATE/err" && grep -q 'could not be listed' "$STATE/err" \
  || { echo "    the refusal does not say the gate could not run:"; cat "$STATE/err"; exit 1; }
# git alone still decides what it can, whatever the forge
expect_exit 0 check "$H1"
echo "    a plain branch passes, a hand-built batch is refused naming both members, its manifest makes it one, and a fix, a reordering, a moved member and an unreadable forge are each refused by name"
