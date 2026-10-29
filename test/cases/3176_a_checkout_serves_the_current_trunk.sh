# majordomus-covers: none
# A checkout says how far it is from the remote trunk, and a stale one fails doctor
# (project.a-checkout-serves-the-current-trunk).
#
# On 2026-10-16 the primary checkout was 779 commits behind origin/master and its shared
# server answered every session from that tree; its local master was the base new branches
# were made from. Every reading said ready, none said "not the trunk".
#
# A fixture remote moves six commits ahead of its clone, under a policy declaring
# checkout.trunk_distance warn_behind 2, fail_behind 5:
#
#   measured   `worktree status` reports the checkout and the local trunk each 6 behind and
#              0 ahead of origin/master, on the trunk, as of the last fetch: stale
#   doctor     FAILs clone/trunk with the fast-forward as the remedy; `worktree trunk`
#              gives the same reading alone and exits 10
#   server     server.status carries the same reading as `trunk`, and so does the MCP tool
#              majordomus_worktree_trunk asked by one tools/call
#   lagging    with fail_behind raised past the distance it WARNs and does not fail
#   feature    a linked worktree on a feature branch is judged on its own HEAD; the local
#              trunk behind it is a WARN (clone/local-trunk), never its failure
#   merging    a merge of the remote trunk in progress counts what it brings in: the
#              commit that makes the branch current is not refused for the tree it replaces
#   current    once fast-forwarded, current and OK
#   unmeasured a clone with no remote is unmeasured, never current, and doctor says so
#
# Nothing is fetched by any reading: the remote-tracking refs are byte-identical before and
# after.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; MAJORDOMUS_BIN="$RB"
export MAJORDOMUS_SHARE MAJORDOMUS_LOG MAJORDOMUS_BIN
W="$(mktemp -d "${TMPDIR:-/tmp}/mj3176.XXXXXX")"; trap 'rm -rf "$W"' EXIT
W="$(cd "$W" && pwd -P)"

"$MJ" init >/dev/null
trunk="$(git symbolic-ref --short HEAD)"
set_thresholds() { # warn fail
  local p=.ai/repo/policy.yaml
  sed -i.bak '/^checkout:/,/^    fail_behind:/d' "$p" && rm -f "$p.bak"
  printf 'checkout:\n  trunk_distance:\n    warn_behind: %s\n    fail_behind: %s\n' "$1" "$2" >> "$p"
}
set_thresholds 2 5
git add -A >/dev/null && git commit -qm "the layer"

# the remote, and six commits on it the clone has not got
git init -q --bare "$W/origin.git"
git remote add origin "$W/origin.git"
git push -q -u origin "$trunk" 2>/dev/null
git clone -q "$W/origin.git" "$W/other" 2>/dev/null
for i in 1 2 3 4 5 6; do
  git -C "$W/other" -c user.name=o -c user.email=o@o commit -q --allow-empty -m "upstream $i"
done
git -C "$W/other" push -q origin "$trunk" 2>/dev/null
git fetch -q origin
refs_before="$(git for-each-ref refs/remotes)"

status() { "$RB" worktree status --repo "${1:-$T}" --format json 2>/dev/null || true; }
field() { printf '%s' "$S" | jq -r "$1"; }

# ---------------------------------------------------------------- measured
S="$(status)"
[ -n "$S" ] || { echo "    worktree status answered nothing"; exit 1; }
got="$(field '.trunk_distance | [.standing, .remote_trunk, .checkout.behind, .checkout.ahead, .local_trunk.behind, .local_trunk.ahead, .on_trunk, .thresholds.warn_behind, .thresholds.fail_behind] | @tsv')"
want="$(printf 'stale\torigin/%s\t6\t0\t6\t0\ttrue\t2\t5' "$trunk")"
[ "$got" = "$want" ] || { echo "    the distance is not measured as it stands:"; echo "      got  $got"; echo "      want $want"; exit 1; }
[ "$(field '.trunk_distance.remote_head')" = "$(git rev-parse "origin/$trunk")" ] \
  || { echo "    remote_head is not the remote trunk's commit"; exit 1; }
[ "$(field '.trunk_distance.fetched_at | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T")')" = true ] \
  || { echo "    the reading does not say when the repository last fetched"; exit 1; }
field '.trunk_distance.remedy' | grep -qF "merge --ff-only origin/$trunk" \
  || { echo "    the remedy does not fast-forward the trunk checkout: $(field '.trunk_distance.remedy')"; exit 1; }
"$RB" worktree status --repo "$T" 2>/dev/null | grep -qE '^currency +!! +stale: this checkout is 6 commit\(s\) behind' \
  || { echo "    the text rendering does not name the distance:"; "$RB" worktree status --repo "$T"; exit 1; }

# the reading alone: the same answer, and exit 10 while stale
expect_exit 10 "$RB" worktree trunk --repo "$T" --format json
[ "$(printf '%s' "$LAST_OUT" | jq -r '"\(.standing) \(.checkout.behind)"')" = "stale 6" ] \
  || { echo "    worktree trunk does not give the status's reading:"; printf '%s\n' "$LAST_OUT"; exit 1; }

# the same reading as a client asks for it: one MCP session, one tools/call
mcp_frame="$({ printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3176","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_worktree_trunk","arguments":{}}}\n'
} | "$RB" mcp --standalone --repo "$T" 2>/dev/null | sed -n 2p)"
[ "$(printf '%s' "$mcp_frame" | jq -r '.result.structuredContent | "\(.standing) \(.checkout.behind)"' 2>/dev/null)" = "stale 6" ] \
  || { echo "    majordomus_worktree_trunk does not answer the reading:"; printf '%s\n' "$mcp_frame" | head -c 600; echo; exit 1; }

# ---------------------------------------------------------------- doctor fails a stale checkout
# (the fixture has other findings — no hooks, no projections — so the trunk line is read,
# and the exit is only required to be a failure)
rc=0; LAST_OUT="$("$MJ" doctor 2>&1)" || rc=$?
[ "$rc" -ne 0 ] || { echo "    doctor passed a stale checkout"; exit 1; }
expect_grep "^FAIL clone +trunk — this checkout is 6 commit\\(s\\) behind and 0 ahead of origin/$trunk; stale: at or past checkout.trunk_distance.fail_behind 5"
expect_grep "\\[reproduce: git fetch; commit or rescue uncommitted work, then: git -C .* merge --ff-only origin/$trunk\\]"

# ---------------------------------------------------------------- the server says it too
srv="$("$RB" serve status --repo "$T" --checkouts this --format json 2>/dev/null || true)"
[ "$(printf '%s' "$srv" | jq -r '.trunk | "\(.standing) \(.checkout.behind)"')" = "stale 6" ] \
  || { echo "    server.status does not carry the reading:"; printf '%s\n' "$srv" | jq '.trunk'; exit 1; }

# ---------------------------------------------------------------- lagging warns, does not fail
set_thresholds 2 50
S="$(status)"
[ "$(field '.trunk_distance.standing')" = lagging ] || { echo "    6 behind against 2/50 is not lagging: $(field '.trunk_distance.standing')"; exit 1; }
"$MJ" doctor > "$W/doctor.txt" 2>&1 || true
grep -qE "^WARN clone +trunk — this checkout is 6 commit" "$W/doctor.txt" \
  || { echo "    a lagging checkout is not a warning:"; grep 'clone' "$W/doctor.txt"; exit 1; }
if grep -qE '^FAIL clone +trunk' "$W/doctor.txt"; then echo "    a lagging checkout failed doctor"; exit 1; fi
set_thresholds 2 5

# ---------------------------------------------------------------- a feature branch in a linked worktree
git worktree add -q -b feature/x "$W/x"
S="$(status "$W/x")"
[ "$(field '.trunk_distance | "\(.on_trunk) \(.standing) \(.local_trunk_standing) \(.checkout.behind)"')" = "false stale stale 6" ] \
  || { echo "    the feature worktree is not judged on its own HEAD:"; field '.trunk_distance'; exit 1; }
field '.trunk_distance.remedy' | grep -qF "merge origin/$trunk" \
  || { echo "    a feature branch's remedy is not a merge of the remote trunk"; exit 1; }
# merging the remote trunk: the commit in progress holds what it brings in
git -C "$W/x" -c user.name=t -c user.email=t@t merge -q --no-ff --no-commit "origin/$trunk" 2>/dev/null || true
git -C "$W/x" rev-parse -q --verify MERGE_HEAD >/dev/null || { echo "    the fixture merge is not in progress"; exit 1; }
S="$(status "$W/x")"
[ "$(field '.trunk_distance | "\(.standing) \(.checkout.behind) \(.local_trunk_standing)"')" = "current 0 stale" ] \
  || { echo "    a merge in progress is not counted as present:"; field '.trunk_distance'; exit 1; }
( cd "$W/x" && "$MJ" doctor ) > "$W/x.txt" 2>&1 || true
grep -qE '^OK +clone +trunk' "$W/x.txt" || { echo "    doctor in the merging worktree is not OK on its own distance:"; grep clone "$W/x.txt"; exit 1; }
grep -qE "^WARN clone +local-trunk — the local $trunk is 6 commit\\(s\\) behind origin/$trunk \\(stale\\)" "$W/x.txt" \
  || { echo "    the stale local trunk is not a warning in the linked worktree:"; grep clone "$W/x.txt"; exit 1; }
if grep -qE '^FAIL clone' "$W/x.txt"; then echo "    the linked worktree failed for the local trunk"; grep clone "$W/x.txt"; exit 1; fi
git -C "$W/x" merge --abort

# ---------------------------------------------------------------- current once fast-forwarded
git merge -q --ff-only "origin/$trunk"
S="$(status)"
[ "$(field '.trunk_distance | "\(.standing) \(.checkout.behind) \(.local_trunk.behind) \(.remedy)"')" = "current 0 0 null" ] \
  || { echo "    a fast-forwarded checkout is not current:"; field '.trunk_distance'; exit 1; }
"$MJ" doctor > "$W/current.txt" 2>&1 || true
grep -qE "^OK +clone +trunk — this checkout is 0 commit\\(s\\) behind and 0 ahead of origin/$trunk; current" "$W/current.txt" \
  || { echo "    doctor does not pass a current checkout:"; grep clone "$W/current.txt"; exit 1; }

# ---------------------------------------------------------------- nothing was fetched
[ "$(git for-each-ref refs/remotes)" = "$refs_before" ] \
  || { echo "    a reading moved a remote-tracking ref"; exit 1; }

# ---------------------------------------------------------------- no remote: unmeasured
mkdir "$W/lonely" && git -C "$W/lonely" init -q
( cd "$W/lonely" && "$MJ" init >/dev/null && git add -A >/dev/null \
  && git -c user.name=t -c user.email=t@t commit -qm layer )
S="$(status "$W/lonely")"
[ "$(field '.trunk_distance.standing')" = unmeasured ] \
  || { echo "    a clone with no remote is not unmeasured: $(field '.trunk_distance.standing')"; exit 1; }
( cd "$W/lonely" && "$MJ" doctor ) > "$W/lonely.txt" 2>&1 || true
grep -qE "^INFO clone +trunk — no remote-tracking branch, so this checkout's distance from the remote trunk cannot be measured" "$W/lonely.txt" \
  || { echo "    doctor does not say a clone with no remote is unmeasured:"; grep clone "$W/lonely.txt"; exit 1; }
exit 0
