# majordomus-covers: plan
# The choice ranks by what it unblocks (I2973, ADR 0177 decision 2).
#
# Among READY issues equal in milestone, wave and priority, the one whose taking unblocks
# more unfinished issues is handed out first, by the shell engine (`majordomus plan next`)
# and the Rust one (plan.next, proceed.plan) alike. Proved on a fixture:
#   1. I0002 unblocks three issues through a diamond (I0003, I0004 and I0008, which depends
#      on both, counted once) and I0001 unblocks none: both engines name I0002, and
#      proceed.plan's reason says it unblocks 3;
#   2. adding a dependency on I0001 to four issues changes the answer to I0001 in both
#      engines, with no list edited;
#   3. of two issues equal in everything but their ids, both engines take the same one: an id
#      is compared by its number, not as text (I9999 before I10000), equal numbers by their
#      spelling (I00000 before I0000), and a number too long for a float exactly
#      (Codex's reviews of I2972 and of 7308a418be0);
#   4. a decision over a source that did not load is provisional (exit 11, no commands): a
#      mesh declaration the index refuses is not an undeclared mesh, and a plan with a
#      record that did not load is partial; the same plan with neither exits 0.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_BIN="$RB"
"$MJ" init >/dev/null
git add -A >/dev/null 2>&1; git commit -qm base >/dev/null 2>&1 || true
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000
pj_issue I0003 M000 I0002
pj_issue I0004 M000 I0002
pj_issue I0008 M000 I0003 I0004
git add -A >/dev/null && git commit -qm plan >/dev/null

shell_next() { "$MJ" plan next 2>/dev/null | grep -oE 'I[0-9]+' | head -n 1; }
rust_next() { "$RB" proceed plan --format json 2>/dev/null | jq -r '.selected.issue // empty'; }

# ---------------------------------------------------------------- 1. the one that unblocks more
[ "$(shell_next)" = I0002 ] || { echo "    the shell engine does not hand out I0002 first: $(shell_next)"; exit 1; }
[ "$(rust_next)" = I0002 ] || { echo "    proceed.plan does not select I0002: $(rust_next)"; exit 1; }
"$RB" proceed plan --format json > "$T/d.json" 2>/dev/null || true
jq -e '.selected.terms.unblocks == ["I0003", "I0004", "I0008"]
  and (.reason | contains("it unblocks 3 issue(s)"))' "$T/d.json" >/dev/null \
  || { echo "    the diamond is not counted once:"; cat "$T/d.json"; exit 1; }

# ---------------------------------------------------------------- 2. a dependency changes the order
for id in I0005 I0006 I0007 I0009; do pj_issue "$id" M000 I0001; done
git add -A >/dev/null && git commit -qm "four issues wait on I0001" >/dev/null
[ "$(shell_next)" = I0001 ] || { echo "    the shell engine did not follow the new dependencies: $(shell_next)"; exit 1; }
[ "$(rust_next)" = I0001 ] || { echo "    proceed.plan did not follow the new dependencies: $(rust_next)"; exit 1; }

# ---------------------------------------------------------------- 3. an id by its number
rm .ai/repo/project/issues/I00*.yaml
pair() { # <id> <id> <expected>: two issues equal in everything else, both engines take <expected>
  rm -f .ai/repo/project/issues/I*.yaml
  pj_issue "$1" M000
  pj_issue "$2" M000
  git add -A >/dev/null && git commit -qm "$1 and $2" >/dev/null
  [ "$(shell_next)" = "$3" ] || { echo "    of $1 and $2 the shell engine takes $(shell_next), not $3"; exit 1; }
  [ "$(rust_next)" = "$3" ] || { echo "    of $1 and $2 proceed.plan takes $(rust_next), not $3"; exit 1; }
}
pair I10000 I9999 I9999
pair I0000 I00000 I00000
pair I10000000000000000 I9999999999999999 I9999999999999999

# ---------------------------------------------------------------- 4. a source that did not load
plan_rc() { rc=0; "$RB" proceed plan --format json > "$T/d.json" 2>/dev/null || rc=$?; }
plan_rc
[ "$rc" = 0 ] && jq -e '.provisional == false and (.sources[] | select(.source == "mesh") | .state == "undeclared")
  and (.sources[] | select(.source == "plan") | .state == "read")' "$T/d.json" >/dev/null   || { echo "    with no declaration and a whole plan the decision is not final ($rc):"; cat "$T/d.json"; exit 1; }
mkdir -p .ai/repo/mesh
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: bananas\n' > .ai/repo/mesh/majordomus.yaml
git add -A >/dev/null && git commit -qm "a declaration the schema refuses" >/dev/null
plan_rc
[ "$rc" = 11 ] && jq -e '.provisional and .next == [] and (.sources[] | select(.source == "mesh") | .state == "unread")' "$T/d.json" >/dev/null \
  || { echo "    a refused declaration is read as no mesh ($rc):"; cat "$T/d.json"; exit 1; }
git rm -q .ai/repo/mesh/majordomus.yaml && git commit -qm "no declaration" >/dev/null
printf 'id: I0042\nmilestone: M000\ntitle: 7\npriority: urgent\n' > .ai/repo/project/issues/I0042.yaml
git add -A >/dev/null && git commit -qm "a record that does not load" >/dev/null
plan_rc
[ "$rc" = 11 ] && jq -e '.provisional and .next == [] and (.sources[] | select(.source == "plan") | .state == "partial"
  and (.summary | contains("I0042")))' "$T/d.json" >/dev/null \
  || { echo "    a plan with a record that did not load is read as whole ($rc):"; cat "$T/d.json"; exit 1; }
exit 0
