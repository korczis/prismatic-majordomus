# majordomus-covers: none
# majordomus-timeout: 900
# `majordomus prs repair --apply` is an act of the integrator and follows the drain's rules
# (ADR 0101; rule project.land-and-publish clause 2): it holds the base branch's integration
# lease for the whole act, writes the act to the trail before it reaches the remote, brings
# master in as a merge commit with the derived driver and a fresh derive, and pushes a
# fast-forward of the head it observed with a plain push, never a forced one, only while
# origin still serves that head. It never merges into master, and a branch somebody pushed
# to meanwhile is refused, never overwritten.
#
# Against a scripted forge and a local bare origin. The repository's scripts/derive is the
# moment between the attempt and the push, so the fixture's derive regenerates the derived
# file and, while it runs, writes down what the trail said, and starts a rival `prs drain`,
# which must be refused by the lease. Three pull requests:
#
#   #1  behind master, conflicting with it only on the derived gen.txt  -> repaired
#   #2  conflicting with master on the authored a.txt                    -> refused, nothing pushed
#   #4  behind master like #1, and its author pushes while the derive runs
#                                                                        -> refused as stale, the
#                                                                           author's push stands
#   #5  the same, but its author rewinds the branch to master's ancestor while the derive runs:
#       the repair's merge would then be a fast-forward of what the branch holds, and only the
#       check that origin still serves the observed head refuses it
#                                                      -> refused as stale, the rewind stands
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-912.git"; W="$T/../work-912"; STATE="$T/../forge-912"; BIN="$T/../bin-912"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
git config user.email t@example.com; git config user.name t
common="$(git rev-parse --path-format=absolute --git-common-dir)"
TRAIL="$common/majordomus/integration/events.jsonl"
mkdir -p scripts
cp "$ROOT/scripts/merge-derived" scripts/merge-derived
# the derive: gen.txt is a fingerprint of the authored tree, as the repository's projections
# are. While it runs it is the act in progress, so it records what the trail said then, and
# a rival executor tries to start; with $STATE/move present, #4's author pushes meanwhile.
cat > scripts/derive <<EOF
#!/bin/sh
git ls-files | grep -v '^gen.txt\$' | LC_ALL=C sort | tr '\n' ' ' > gen.txt
n=\$(ls "$STATE"/at-derive-* 2>/dev/null | wc -l | tr -d ' ')
cp "$TRAIL" "$STATE/at-derive-\$n"
"$RB" prs --repo "$W" drain --max 1 > "$STATE/rival-\$n.out" 2>&1; echo \$? > "$STATE/rival-\$n.rc"
if [ -e "$STATE/move" ]; then read -r br sha < "$STATE/move"; git -C "$W" push -q -f origin "\$sha:refs/heads/\$br"; fi
exit 0
EOF
chmod +x scripts/derive scripts/merge-derived
printf 'gen.txt merge=derived\n' > .gitattributes
echo base > a.txt; echo 'gen 0' > gen.txt
gitq add -A; gitq commit -qm base; gitq push -q origin HEAD:master
M0="$(git rev-parse HEAD)"
branch() {   # <name> <from> <file> <content> [<file> <content>]: one commit, pushed
  gitq checkout -q -b "$1" "$2"; echo "$4" > "$3"
  [ -z "${5:-}" ] || echo "$6" > "$5"
  gitq add -A; gitq commit -qm "$1"; gitq push -q origin "HEAD:refs/heads/$1"
}
branch feature/1 "$M0" one.txt one gen.txt 'gen 1'
branch feature/2 "$M0" a.txt two
branch feature/4 "$M0" four.txt four gen.txt 'gen 4'
branch feature/5 "$M0" five.txt five gen.txt 'gen 5'
gitq checkout -q feature/4
# what #4's author will push while the repair of it runs, made now and pushed then
echo more > more.txt; gitq add more.txt; gitq commit -qm 'the author again'
H4B="$(git rev-parse HEAD)"
gitq checkout -q master
echo master > a.txt; echo 'gen M' > gen.txt; gitq commit -qam 'master moves'; gitq push -q origin HEAD:master
M1="$(git rev-parse HEAD)"
H1="$(git rev-parse feature/1)"; H2="$(git rev-parse feature/2)"; H4="$(git rev-parse 'feature/4~1')"
git config merge.derived.name derived
git config merge.derived.driver "$W/scripts/merge-derived %O %A %B %P"
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }

printf '%s\t%s\n' 1 feature/1  2 feature/2  4 feature/4  5 feature/5 > "$STATE/prs.tsv"
cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    case " \$* " in *" --state closed "*) echo '[]'; exit 0 ;; esac
    printf '['; sep=''
    while IFS='	' read -r n br; do
      h="\$(git -C "$ORIGIN" rev-parse "refs/heads/\$br")"
      git -C "$ORIGIN" update-ref "refs/pull/\$n/head" "\$h"
      printf '%s{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-0%sT00:00:00Z","updatedAt":"2026-09-0%sT00:00:00Z","body":"","statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}' \
        "\$sep" "\$n" "\$n" "\$br" "\$h" "\$n" "\$n"
      sep=','
    done < "$STATE/prs.tsv"
    printf ']\n' ;;
  *) echo "WRITE-OR-UNEXPECTED \$*" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
remote() { git -C "$ORIGIN" rev-parse "refs/heads/$1"; }
actions() { jq -r .action "$TRAIL" | tr '\n' ' '; }
prs refresh >/dev/null 2>"$STATE/refresh.err" || { echo "    the refresh failed:"; cat "$STATE/refresh.err"; exit 1; }

# ---------------------------------------------------------------- #1: repaired
: > "$TRAIL"
expect_exit 0 prs repair 1 --apply --format json
r="$LAST_OUT"
[ "$(printf '%s' "$r" | jq -r '.dry_run, .outcome.outcome' | tr '\n' ' ')" = "false repaired " ] \
  || { echo "    #1 was not repaired: $r"; tail -5 "$STATE/log"; exit 1; }
N1="$(printf '%s' "$r" | jq -r .outcome.head_after)"
[ "$(remote feature/1)" = "$N1" ] || { echo "    origin's feature/1 is $(remote feature/1), not the reported $N1"; exit 1; }
# a merge commit of the observed head and the master decided on: a fast-forward, never a rewrite
[ "$(git -C "$ORIGIN" rev-parse "$N1^1" "$N1^2" | tr '\n' ' ')" = "$H1 $M1 " ] \
  || { echo "    $N1 is not the merge of $H1 and $M1"; git -C "$ORIGIN" log --oneline -3 "$N1"; exit 1; }
# the derived file was regenerated, not resolved by hand
[ "$(git -C "$ORIGIN" show "$N1:gen.txt")" = '.gitattributes a.txt one.txt scripts/derive scripts/merge-derived ' ] \
  || { echo "    gen.txt at $N1 was not regenerated: $(git -C "$ORIGIN" show "$N1:gen.txt")"; exit 1; }
# master is untouched, and the forge was never asked to merge or to do anything else
[ "$(remote master)" = "$M1" ] || { echo "    a repair moved master"; exit 1; }
if grep -q 'WRITE-OR-UNEXPECTED\|pr merge' "$STATE/log"; then echo "    the forge was asked to act:"; grep 'WRITE\|merge' "$STATE/log"; exit 1; fi
# the trail: the lease, a fresh observation, the selection and the attempt before the act, the
# outcome after it
[ "$(actions)" = "lease_acquired observed repair_selected repair_attempted repaired lease_released " ] \
  || { echo "    the trail of #1 is: $(actions)"; exit 1; }
[ "$(jq -r 'select(.action == "repaired") | .head_after' "$TRAIL")" = "$N1" ] || { echo "    repaired does not name the head it pushed"; exit 1; }
# while the derive ran — after the attempt, before the push — the attempt was on the trail and
# the lease was held: the rival drain was refused by it
[ "$(jq -r .action "$STATE/at-derive-0" | tail -1)" = repair_attempted ] \
  || { echo "    the act ran before its attempt was recorded: $(jq -r .action "$STATE/at-derive-0" | tr '\n' ' ')"; exit 1; }
[ "$(cat "$STATE/rival-0.rc")" = 12 ] || { echo "    a rival drain was not refused while the repair held the lease (exit $(cat "$STATE/rival-0.rc")):"; cat "$STATE/rival-0.out"; exit 1; }
grep -qE 'another integration executor holds .*integration-master\.lock' "$STATE/rival-0.out" \
  || { echo "    the rival's refusal does not name the lease:"; cat "$STATE/rival-0.out"; exit 1; }

# ---------------------------------------------------------------- #2: an authored conflict
: > "$TRAIL"
expect_exit 10 prs repair 2 --apply
expect_grep 'REFUSE #2 \(feature/2\): merging master conflicts on 1 authored file'
expect_grep '^       a\.txt$'
[ "$(remote feature/2)" = "$H2" ] || { echo "    #2's branch moved"; exit 1; }
[ "$(actions)" = "lease_acquired observed repair_refused lease_released " ] || { echo "    the trail of #2 is: $(actions)"; exit 1; }
[ "$(jq -r 'select(.action == "repair_refused") | .class' "$TRAIL")" = conflict ] || { echo "    #2's refusal is not classed a conflict"; exit 1; }
[ ! -e "$STATE/at-derive-1" ] || { echo "    the refused repair derived anyway"; exit 1; }

# ---------------------------------------------------------------- #4: the branch moved meanwhile
: > "$TRAIL"
echo "feature/4 $H4B" > "$STATE/move"
expect_exit 10 prs repair 4 --apply --format json
r="$LAST_OUT"
[ "$(printf '%s' "$r" | jq -r '.head_sha, .outcome.refusal.kind, .outcome.refusal.class' | tr '\n' ' ')" = "$H4 act_failed stale " ] \
  || { echo "    the moved branch was not refused as stale: $r"; exit 1; }
[ "$(remote feature/4)" = "$H4B" ] || { echo "    the author's push was overwritten: feature/4 is $(remote feature/4), not $H4B"; exit 1; }
[ "$(actions)" = "lease_acquired observed repair_selected repair_attempted repair_refused lease_released " ] \
  || { echo "    the trail of #4 is: $(actions)"; exit 1; }
[ "$(remote master)" = "$M1" ] || { echo "    a repair moved master"; exit 1; }
# no scratch worktree outlives an act
[ "$(git -C "$W" worktree list | wc -l | tr -d ' ')" = 1 ] || { echo "    a scratch worktree was left behind:"; git -C "$W" worktree list; exit 1; }

# ---------------------------------------------------------------- #5: rewound meanwhile
# A plain push would take this: the repair's merge descends from the rewound tip. The check
# that origin still serves the head that was observed (git ls-remote) is what refuses it.
: > "$TRAIL"
H5="$(remote feature/5)"
echo "feature/5 $M0" > "$STATE/move"
expect_exit 10 prs repair 5 --apply --format json
r="$LAST_OUT"
[ "$(printf '%s' "$r" | jq -r '.head_sha, .outcome.refusal.kind, .outcome.refusal.class' | tr '\n' ' ')" = "$H5 act_failed stale " ] \
  || { echo "    the rewound branch was not refused as stale: $r"; exit 1; }
[ "$(remote feature/5)" = "$M0" ] || { echo "    the author's rewind was overwritten: feature/5 is $(remote feature/5), not $M0"; exit 1; }
rm "$STATE/move"
[ "$(git -C "$W" worktree list | wc -l | tr -d ' ')" = 1 ] || { echo "    a scratch worktree was left behind:"; git -C "$W" worktree list; exit 1; }
