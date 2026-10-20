# majordomus-covers: none
# A served surface says which checkout it is serving, and never where it sits (I1031).
#
# Several sessions share one machine and each may bind a server of its own. A Swagger page
# held from a server of another session's worktree read exactly like this tree's, and a
# primary checkout 779 commits behind master answered plan questions over MCP for half an
# hour with nothing on any surface saying so. So every surface that renders the process —
# the home page, its JSON projection and the API shell — names the checkout that answered:
# its identity and its git repository's (crate::repository, ADR 0035), and the branch and
# commit the index read. How far that commit is behind the trunk is not this case's: it is
# the rule project.a-checkout-serves-the-current-trunk, with a case of its own.
#
# What is proved, against two roots served at once — a primary checkout and a linked
# worktree of it, on two different commits:
#  1. each JSON index names its own checkout, one git repository between them, and its own
#     branch and head;
#  2. each home page and each API shell carries its own checkout identity and not the
#     other's, and the one repository identity both share;
#  3. none of those answers, nor the OpenAPI document nor the route listing, carries an
#     absolute path: not the fixture's, not this tree's, not $HOME, and nothing the
#     published-output gate (scripts/ci/no-machine-paths, from
#     apps/majordomus-cli/src/quality/leak-patterns.tsv) calls a machine path; and the
#     published tree itself passes that gate.
#
# Every server is started with `--port 0`: the default port belongs to whatever already
# serves this machine, and a case may never take it.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq not installed"
command -v curl >/dev/null 2>&1 || skip "curl not installed"

MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# no MAJORDOMUS_LOG=warn here: serve_up waits for the server's listening line, which is
# logged at info, and a quieter level leaves it waiting for a line that never comes
# serve_up waits for a loopback listening line; a host a developer's shell exports for its
# own server (MAJORDOMUS_HTTP_HOST=0.0.0.0) is not this case's to inherit
unset MAJORDOMUS_HTTP_HOST

# Nested one level down, so that the worktree container ($R-wt) stays inside $T; resolved,
# because git holds the resolved path and macOS's $TMPDIR is a symlink.
mkdir -p "$T/wt"; W="$(cd "$T/wt" && pwd -P)"; R="$W/repo"
mkdir -p "$R"
g() { git -C "$1" -c core.hooksPath=/dev/null -c user.email=t@example.com -c user.name=t "${@:2}"; }
g "$R" init -q .
( cd "$R" && "$MJ" init >/dev/null )
g "$R" add -A >/dev/null && g "$R" commit -qm base
trunk="$(g "$R" symbolic-ref --short HEAD)"
A="$R-wt/feature/alpha"
g "$R" worktree add -q -b feature/alpha "$A" "$trunk"
g "$A" commit -q --allow-empty -m alpha
head_r="$(g "$R" rev-parse HEAD)"; head_a="$(g "$A" rev-parse HEAD)"

SRV=""; SRV_R=""; SRV_A=""
# SRV too: a server serve_up started and gave up on is still running, and the case must
# not leave it behind
trap 'kill $SRV $SRV_R $SRV_A 2>/dev/null || true; wait $SRV $SRV_R $SRV_A 2>/dev/null || true' EXIT
cd "$R" && { serve_up "$T/r.out" "$T/r.err" || exit 1; }; SRV_R="$SRV"; U_R="$U"; SRV=""
cd "$A" && { serve_up "$T/a.out" "$T/a.err" || exit 1; }; SRV_A="$SRV"; U_A="$U"; SRV=""
cd "$T" || exit 1
[ "$U_R" != "$U_A" ] || { echo "    the two roots share one address: $U_R"; exit 1; }

get() { curl -sS -m 30 "$@"; }
fetch() { # fetch <name> <url>: every surface that renders the process
  get "$2/" > "$T/$1.json"
  get -H 'Accept: text/html' "$2/" > "$T/$1.home.html"
  get "$2/swagger" > "$T/$1.swagger.html"
  get "$2/openapi.json" > "$T/$1.openapi.json"
  get "$2/api/v1/" > "$T/$1.routes.json"
}
fetch r "$U_R"; fetch a "$U_A"

# ---------------------------------------------------------------- 1. each index names its own
id_r="$(jq -r '.repository_id' "$T/r.json")"; id_a="$(jq -r '.repository_id' "$T/a.json")"
[ -n "$id_r" ] && [ "$id_r" != null ] || { echo "    the index names no checkout:"; cat "$T/r.json"; exit 1; }
[ "$id_r" != "$id_a" ] || { echo "    two checkouts answer as one: $id_r"; exit 1; }
git_id="$(jq -r '.git_repository_id' "$T/r.json")"
[ -n "$git_id" ] && [ "$git_id" != null ] && [ "$git_id" = "$(jq -r '.git_repository_id' "$T/a.json")" ] \
  || { echo "    two worktrees of one repository do not name one git repository"; exit 1; }
[ "$head_r" != "$head_a" ] || { echo "    the fixture's two checkouts are on one commit"; exit 1; }
jq -e --arg h "$head_r" --arg b "$trunk" '.linked_worktree == false and .head == $h and .branch == $b' \
  "$T/r.json" >/dev/null || { echo "    the primary's index does not name $trunk at $head_r:"; cat "$T/r.json"; exit 1; }
jq -e --arg h "$head_a" '.linked_worktree == true and .head == $h and .branch == "feature/alpha"' \
  "$T/a.json" >/dev/null || { echo "    the worktree's index does not name feature/alpha at $head_a:"; cat "$T/a.json"; exit 1; }

# ---------------------------------------------------------------- 2. each page names its own
for page in home swagger; do
  expect_grep "data-mj-checkout=\"$id_r\"" "$T/r.$page.html"
  expect_no_grep "$id_a" "$T/r.$page.html"
  expect_grep "data-mj-checkout=\"$id_a\"" "$T/a.$page.html"
  expect_no_grep "$id_r" "$T/a.$page.html"
  expect_grep "${head_r:0:12}" "$T/r.$page.html"
  expect_grep "${head_a:0:12}" "$T/a.$page.html"
  expect_grep "feature/alpha" "$T/a.$page.html"
  expect_grep "linked worktree" "$T/a.$page.html"
  expect_grep "primary checkout" "$T/r.$page.html"
  # and one git repository behind both
  expect_grep "data-mj-repository=\"$git_id\"" "$T/r.$page.html"
  expect_grep "data-mj-repository=\"$git_id\"" "$T/a.$page.html"
done
# the shell is still the shell: one specification, loaded from the server that served it
expect_grep 'url: "/openapi.json"' "$T/r.swagger.html"

# ---------------------------------------------------------------- 3. no answer says where anything sits
patterns=()
while IFS= read -r p; do [ -n "$p" ] && patterns+=(-e "$p"); done \
  < <(awk -F'\t' '$1 == "machine-path" { print $3 }' "$ROOT/apps/majordomus-cli/src/quality/leak-patterns.tsv")
[ "${#patterns[@]}" -gt 0 ] || { echo "    no machine-path pattern could be read; refusing to call the answers clean"; exit 1; }
for f in "$T"/r.json "$T"/a.json "$T"/*.html "$T"/*.openapi.json "$T"/*.routes.json; do
  [ -s "$f" ] || { echo "    $f is empty: the surface answered nothing to look at"; exit 1; }
  for literal in "$W" "$T" "$ROOT" "$HOME/"; do
    if grep -qF -- "$literal" "$f"; then
      echo "    ${f#"$T"/} names a host path ($literal):"; grep -nF -- "$literal" "$f" | head -3; exit 1
    fi
  done
  if grep -qE "${patterns[@]}" "$f"; then
    echo "    ${f#"$T"/} carries a machine path:"; grep -nE "${patterns[@]}" "$f" | head -3; exit 1
  fi
done
# and what is published passes the gate that refuses a machine path in the tracked tree
expect_exit 0 "$ROOT/scripts/ci/no-machine-paths" "$ROOT"
