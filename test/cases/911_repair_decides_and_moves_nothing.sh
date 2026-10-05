# majordomus-covers: none
# majordomus-timeout: 900
# `majordomus prs repair` decides from the integrator's own classification, and its default —
# the dry run — is a read that moves nothing (rule project.land-and-publish clause 2; ADR
# 0101). It is the gesture scripts/unblock performed outside the integrator: bring master
# into a pull request whose only conflict with it is over `merge=derived` files.
#
# Against a scripted forge and a local bare origin, with the derived driver declared as
# `just derive-merge-driver` declares it, five pull requests:
#
#   #1  behind master; master and it both rewrote the derived gen.txt  -> eligible
#   #2  behind master; master and it both rewrote the authored a.txt   -> refused, a.txt named
#   #3  branched from the current master                               -> nothing to repair
#   #4  behind master, its head in a fork                              -> refused
#   #9  not open                                                       -> refused
#
# After one `prs refresh`, the forge is made unreachable — the GitHub CLI fails and logs any
# call, and the origin is moved away — and every dry run still answers: it decides on the
# last recorded observation, as `prs status` does. Before and after, origin's refs, the local
# refs, the audit trail and the lease directory are byte-identical, no worktree was added,
# and the forge was asked nothing.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-911.git"; W="$T/../work-911"; STATE="$T/../forge-911"; BIN="$T/../bin-911"
rm -rf "$ORIGIN" "$ORIGIN.away" "$W" "$STATE" "$BIN"; mkdir -p "$STATE" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
cd "$W" || exit 1
mkdir -p scripts
cp "$ROOT/scripts/merge-derived" scripts/merge-derived
printf '#!/bin/sh\nexit 0\n' > scripts/derive; chmod +x scripts/derive scripts/merge-derived
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
gitq checkout -q master
echo master > a.txt; echo 'gen M' > gen.txt; gitq commit -qam 'master moves'; gitq push -q origin HEAD:master
M1="$(git rev-parse HEAD)"
branch feature/3 "$M1" three.txt three
gitq checkout -q master
# the driver, as the derive-merge-driver recipe declares it: per clone, by absolute path
git config merge.derived.name derived
git config merge.derived.driver "$W/scripts/merge-derived %O %A %B %P"
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed:"; tail -5 "$STATE/init.log"; exit 1; }

printf '%s\t%s\t%s\n' 1 feature/1 false  2 feature/2 false  3 feature/3 false  4 feature/4 true > "$STATE/prs.tsv"
cat > "$BIN/gh" <<EOF
#!/bin/sh
echo "\$*" >> "$STATE/log"
[ -e "$STATE/unreachable" ] && { echo "FORGE-ASKED" >> "$STATE/log"; exit 1; }
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    case " \$* " in *" --state closed "*) echo '[]'; exit 0 ;; esac
    printf '['; sep=''
    while IFS='	' read -r n br fork; do
      h="\$(git -C "$ORIGIN" rev-parse "refs/heads/\$br")"
      git -C "$ORIGIN" update-ref "refs/pull/\$n/head" "\$h"
      printf '%s{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-0%sT00:00:00Z","updatedAt":"2026-09-0%sT00:00:00Z","body":"","statusCheckRollup":[{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":%s}' \
        "\$sep" "\$n" "\$n" "\$br" "\$h" "\$n" "\$n" "\$fork"
      sep=','
    done < "$STATE/prs.tsv"
    printf ']\n' ;;
  *) echo "WRITE-OR-UNEXPECTED" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }

prs refresh >/dev/null 2>"$STATE/refresh.err" || { echo "    the refresh failed:"; cat "$STATE/refresh.err"; tail -5 "$STATE/log"; exit 1; }
# the classification the repair reads is the queue's: #1 needs master, #2 conflicts on a.txt
q="$(prs status --format json || true)"
disp() { printf '%s' "$q" | jq -r --argjson n "$1" '.assessments[] | select(.number == $n) | .disposition'; }
[ "$(disp 1)" = needs_refresh ] || { echo "    #1 is $(disp 1), not needs_refresh"; exit 1; }
[ "$(disp 2)" = conflicting ] || { echo "    #2 is $(disp 2), not conflicting"; exit 1; }

# ---------------------------------------------------------------- the forge is gone; the dry run is not
common="$(git rev-parse --path-format=absolute --git-common-dir)"
snapshot() {
  {
    echo "== origin"; git -C "${1:-$ORIGIN}" for-each-ref --format='%(refname) %(objectname)'
    echo "== local"; git -C "$W" for-each-ref --format='%(refname) %(objectname)'
    echo "== trail"; cat "$common/majordomus/integration/events.jsonl" 2>/dev/null || true
    echo "== locks"; ls -la "$common/majordomus/locks" 2>/dev/null | awk '{print $1, $5, $NF}' || true
    echo "== worktrees"; git -C "$W" worktree list --porcelain
  } > "$2"
}
snapshot "$ORIGIN" "$STATE/before"
: > "$STATE/log"
touch "$STATE/unreachable"
mv "$ORIGIN" "$ORIGIN.away"

expect_exit 0 prs repair 1
expect_grep 'dry run: would merge master'
expect_grep '--apply does'
expect_exit 0 prs repair 1 --dry-run
expect_exit 0 prs repair 1 --format json
r="$LAST_OUT"
[ "$(printf '%s' "$r" | jq -r .outcome.outcome)" = would_repair ] || { echo "    #1 is not eligible: $r"; exit 1; }
[ "$(printf '%s' "$r" | jq -r '.dry_run, .pr, .branch, .master_sha, .relation.kind' | tr '\n' ' ')" = "true 1 feature/1 $M1 behind " ] \
  || { echo "    the dry run of #1 does not say what it was decided on: $r"; exit 1; }
# by its head branch, as scripts/unblock took it
expect_exit 0 prs repair feature/1 --format json
[ "$(printf '%s' "$LAST_OUT" | jq -r '.pr, .outcome.outcome' | tr '\n' ' ')" = "1 would_repair " ] \
  || { echo "    feature/1 does not name #1: $LAST_OUT"; exit 1; }

# an authored conflict is refused, and the files are named
expect_exit 10 prs repair 2
expect_grep '^REFUSE #2 \(feature/2\): merging master conflicts on 1 authored file'
expect_grep '^       a\.txt$'
expect_exit 10 prs repair 2 --format json
[ "$(printf '%s' "$LAST_OUT" | jq -r '.outcome.refusal.kind, (.outcome.refusal.paths | join(","))' | tr '\n' ' ')" = "authored_conflict a.txt " ] \
  || { echo "    #2's refusal is not typed with its paths: $LAST_OUT"; exit 1; }

expect_exit 0 prs repair 3
expect_grep 'nothing to repair: its head already contains master'
expect_exit 10 prs repair 4
expect_grep 'fork'
expect_exit 10 prs repair 9
expect_grep 'not an open pull request'

# applying and dry-running at once is a usage error, before anything is read
expect_exit 2 prs repair 1 --apply --dry-run

mv "$ORIGIN.away" "$ORIGIN"
rm "$STATE/unreachable"
snapshot "$ORIGIN" "$STATE/after"
if ! diff -u "$STATE/before" "$STATE/after" > "$STATE/moved"; then
  echo "    a dry run moved something:"; sed 's/^/      /' "$STATE/moved"; exit 1
fi
if [ -s "$STATE/log" ]; then
  echo "    a dry run asked the forge:"; sed 's/^/      /' "$STATE/log"; exit 1
fi

# a dry run over an observation the clone has moved past is a reading nobody can vouch for:
# it still says what it would do, names the diagnostic, and exits 10 as status and plan do
m="$(git -C "$W" rev-parse refs/remotes/origin/master)"
later="$(git -C "$W" commit-tree -p "$m" -m "master moved on since the observation" "$m^{tree}")"
git -C "$W" update-ref refs/remotes/origin/master "$later"
expect_exit 10 prs repair 1
expect_grep 'stale observation'
expect_exit 10 prs repair 1 --format json
[ "$(printf '%s' "$LAST_OUT" | jq -r '.diagnostics | length')" -ge 1 ] \
  || { echo "    the json answer carries no diagnostic: $LAST_OUT"; exit 1; }
