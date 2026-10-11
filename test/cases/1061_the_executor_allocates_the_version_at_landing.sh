# majordomus-covers: none
# The integration executor allocates the version where it already refreshes a pull request
# (ADR 0134, slice B), through the real command line against a scripted forge, as case 850
# drives it:
#
#   1. allocation off: a head whose manifest version conflicts with master's is a person's
#      to resolve, as it always was, and nothing is refreshed
#   2. allocation at landing, committed on master: the same head needs a refresh, because a
#      conflict on the version line alone is the executor's; a working tree that says landing
#      while master does not changes nothing
#   3. the refresh resolves that line to master's, keeps the branch's own change beside it,
#      writes master's version raised by the policy floor (0.5.0 -> 0.6.0), and says so in a
#      Version-Allocated trailer on the commit it pushes
#   4. master moves on to 0.6.0 by another landing: the next refresh recomputes from the new
#      master (0.7.0), never raising the number the head already carries a second time
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
RB="$(rust_bin)" || rust_bin_exit $?

gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
ORIGIN="$T/../origin-1061.git"; W="$T/../work-1061"; STATE="$T/../forge-1061"; BIN="$T/../bin-1061"
rm -rf "$ORIGIN" "$W" "$STATE" "$BIN"; mkdir -p "$STATE/ci" "$BIN"
gitq init -q --bare -b master "$ORIGIN"
gitq clone -q "$ORIGIN" "$W" 2>/dev/null
git -C "$W" config user.email t@example.com; git -C "$W" config user.name t
cd "$W" || exit 1
MANIFEST=apps/majordomus-cli/Cargo.toml
manifest() {   # <version> [extra dependency line]
  printf '[package]\nname = "fixture"\nversion = "%s"\nedition = "2021"\n\n[dependencies]\na = "1"\n%s' \
    "$1" "${2:+$2
}" > "$MANIFEST"
}
mkdir -p scripts apps/majordomus-cli docs/generated
printf '#!/bin/sh\nexit 0\n' > scripts/derive; chmod +x scripts/derive
cp "$ROOT/docs/generated/registry.json" docs/generated/registry.json
manifest 0.4.0
gitq add -A; gitq commit -qm base; gitq push -q origin HEAD:master
M0="$(git rev-parse HEAD)"
# the pull request: a fix that also raised the version by hand, as every branch did
gitq checkout -q -b feature/1 "$M0"
echo one > one.txt; manifest 0.4.1 'b = "1"'
gitq add -A; gitq commit -qm "fix: one"; gitq push -q origin HEAD:refs/heads/feature/1
H1="$(git rev-parse HEAD)"
# master moves on: another landing took 0.5.0
gitq checkout -q master; manifest 0.5.0; echo more > a.txt
gitq add -A; gitq commit -qm "feat: another landing"; gitq push -q origin HEAD:master
"$MJ" init >"$STATE/init.log" 2>&1 || { echo "    majordomus init failed in the scratch clone:"; tail -5 "$STATE/init.log"; exit 1; }
# the policy a landing obeys is the one master commits, never the executor's working tree
release_policy() {   # <allocation>: committed to master and pushed
  awk '/^release:/{skip=1; next} skip && /^[^ ]/{skip=0} !skip' .ai/repo/policy.yaml > "$STATE/policy"
  cp "$STATE/policy" .ai/repo/policy.yaml
  printf '\nrelease:\n  allocation: %s\n  floor: minor\n' "$1" >> .ai/repo/policy.yaml
  gitq add -A .ai; gitq commit -qm "chore: release allocation $1"; gitq push -q origin HEAD:master
}
release_policy off

printf '%s\t%s\t%s\t%s\n' 1 feature/1 2026-09-01T00:00:00Z "" > "$STATE/prs.tsv"
echo SUCCESS > "$STATE/ci/$H1"

cat > "$BIN/gh" <<EOF
#!/bin/sh
[ -n "\${DECLARATIONS:-}" ] || echo "\$*" >> "$STATE/log"
case " \$* " in *" --admin "*) echo "ADMIN" >> "$STATE/log"; exit 1 ;; esac
case "\$1 \$2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "\$(git -C "$ORIGIN" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    printf '['; sep=''
    while IFS='	' read -r n br created body; do
      h="\$(git -C "$ORIGIN" rev-parse "refs/heads/\$br")"
      git -C "$ORIGIN" update-ref "refs/pull/\$n/head" "\$h"
      if [ -f "$STATE/ci/\$h" ]; then
        roll="[{\"__typename\":\"CheckRun\",\"name\":\"ci\",\"status\":\"COMPLETED\",\"conclusion\":\"\$(cat "$STATE/ci/\$h")\"}]"
      else roll='[]'; fi
      printf '%s{"number":%s,"title":"change %s","author":{"login":"someone"},"headRefName":"%s","headRefOid":"%s","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"%s","updatedAt":"%s","body":"%s","statusCheckRollup":%s,"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}' \
        "\$sep" "\$n" "\$n" "\$br" "\$h" "\$created" "\$created" "\$body" "\$roll"
      sep=','
    done < "$STATE/prs.tsv"
    printf ']\n' ;;
  "pr view") echo OPEN ;;
  "api graphql")
    nodes="\$(DECLARATIONS=1 "\$0" pr list --state open | grep -o '"number":[0-9]*' | sed 's/.*/{&,"authorAssociation":"OWNER","isCrossRepository":false,"timelineItems":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[]}}/' | paste -sd, -)"
    printf '{"data":{"repository":{"pullRequests":{"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[%s]}}}}\n' "\$nodes" ;;
  *) echo "UNEXPECTED \$*" >> "$STATE/log"; exit 1 ;;
esac
EOF
chmod +x "$BIN/gh"
export PATH="$BIN:$PATH"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
prs() { "$RB" prs --repo "$W" "$@"; }
: > "$STATE/log"
disp() { printf '%s' "$q" | jq -r '.assessments[] | select(.number == 1) | .disposition'; }
why() { printf '%s' "$q" | jq -c '.assessments[] | select(.number == 1) | {disposition, reasons}' | sed 's/^/      /'; }
at() { git -C "$ORIGIN" show "refs/heads/feature/1:$MANIFEST"; }
declared() { at | sed -n 's/^version = "\(.*\)"$/\1/p'; }

# ---------------------------------------------------------------- 1. allocation off
prs refresh >/dev/null || { echo "    the first observation failed"; tail -5 "$STATE/log"; exit 1; }
q="$(prs status --format json)"
case "$(disp)" in
  needs_refresh|ready) echo "    with allocation off a version conflict was taken as the executor's: $(disp)"; why; exit 1 ;;
esac
prs drain --refresh >/dev/null 2>&1
[ "$(git -C "$ORIGIN" rev-parse refs/heads/feature/1)" = "$H1" ] \
  || { echo "    with allocation off the executor moved the branch"; exit 1; }

# ---------------------------------------------------------------- 2. allocation at landing
# only the working tree says landing: the trunk does not, and nothing changes
printf '\nrelease:\n  allocation: landing\n' >> .ai/repo/policy.yaml
prs refresh >/dev/null; q="$(prs status --format json)"
[ "$(disp)" != needs_refresh ] || { echo "    the executor's working tree, not master, decided the allocation"; exit 1; }
git checkout -q -- .ai/repo/policy.yaml
release_policy landing
prs refresh >/dev/null
q="$(prs status --format json)"
[ "$(disp)" = needs_refresh ] || { echo "    under landing a version-only conflict is not the executor's:"; why; exit 1; }

# ---------------------------------------------------------------- 3. the refresh allocates
out="$(prs drain --refresh 2>&1)" || { echo "    the refresh drain failed: $out"; tail -5 "$STATE/log"; exit 1; }
H1b="$(git -C "$ORIGIN" rev-parse refs/heads/feature/1)"
[ "$H1b" != "$H1" ] || { echo "    the refresh pushed nothing: $out"; exit 1; }
git -C "$ORIGIN" merge-base --is-ancestor master "$H1b" || { echo "    the new head does not contain master"; exit 1; }
[ "$(declared)" = 0.6.0 ] || { echo "    the refresh allocated $(declared), not 0.6.0:"; at | sed 's/^/      /'; exit 1; }
at | grep -q '^b = "1"$' || { echo "    the branch's own dependency was lost in the resolution"; exit 1; }
git -C "$ORIGIN" log -1 --format=%B "$H1b" | grep -q '^Version-Allocated: 0.6.0 (minor from 0.5.0)$' \
  || { echo "    the refresh commit carries no allocation trailer:"; git -C "$ORIGIN" log -1 --format=%B "$H1b" | sed 's/^/      /'; exit 1; }

# ---------------------------------------------------------------- 4. master moves; recomputed
git fetch -q origin; gitq checkout -q -B master origin/master
manifest 0.6.0; echo again >> a.txt
gitq add "$MANIFEST" a.txt; gitq commit -qm "feat: a third landing"; gitq push -q origin HEAD:master
echo SUCCESS > "$STATE/ci/$H1b"
prs refresh >/dev/null
q="$(prs status --format json)"
[ "$(disp)" = needs_refresh ] || { echo "    after master moved #1 is not behind:"; why; exit 1; }
out="$(prs drain --refresh 2>&1)" || { echo "    the second refresh drain failed: $out"; exit 1; }
[ "$(declared)" = 0.7.0 ] || { echo "    the second refresh allocated $(declared), not 0.7.0 from the new master"; exit 1; }
! grep -q '^ADMIN\|^UNEXPECTED' "$STATE/log" || { echo "    the forge was asked something it should not be:"; grep '^ADMIN\|^UNEXPECTED' "$STATE/log"; exit 1; }

echo "    the executor allocates the version at landing: resolved, floored, trailed, recomputed"
