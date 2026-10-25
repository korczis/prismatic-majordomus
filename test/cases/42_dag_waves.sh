# majordomus-covers: plan
# majordomus-negative: plan
# claims: dag-validation, execution-waves
# The dependency graph: what it refuses, what it derives, and what it draws.
#
# A dependency list without validation is a suggestion. Each negative case below is a
# graph that must be rejected by name rather than silently producing an empty ready set.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
pj_init
pj_milestone M000

# --- a diamond: two independent middles that both wait on the root and both feed the tip
pj_issue I0001 M000
pj_issue I0002 M000 I0001
pj_issue I0003 M000 I0001
pj_issue I0004 M000 I0002 I0003
expect_exit 0 "$MJ" plan validate

# waves place an issue one layer past its deepest dependency
waves="$("$MJ" plan waves)"
printf '%s' "$waves" | grep -qE '^Wave 0'   || { echo "    no wave 0"; exit 1; }
w() { "$MJ" plan list | awk -v i="$1" '$1==i{print $3}'; }
[ "$(w I0001)" = 0 ] || { echo "    I0001 wave $(w I0001), expected 0"; exit 1; }
[ "$(w I0002)" = 1 ] || { echo "    I0002 wave $(w I0002), expected 1"; exit 1; }
[ "$(w I0003)" = 1 ] || { echo "    I0003 wave $(w I0003), expected 1"; exit 1; }
[ "$(w I0004)" = 2 ] || { echo "    I0004 wave $(w I0004), expected 2"; exit 1; }

# --- Mermaid is generated from the same edges, and every edge in it is an edge in the model
g="$("$MJ" plan graph)"
printf '%s' "$g" | grep -q '^flowchart LR' || { echo "    plan graph is not a Mermaid flowchart"; exit 1; }
for e in "I0001 --> I0002" "I0001 --> I0003" "I0002 --> I0004" "I0003 --> I0004"; do
  printf '%s' "$g" | grep -q -- "$e" || { echo "    missing edge: $e"; exit 1; }
done
# no edge the model does not have
[ "$(printf '%s\n' "$g" | grep -c -- '-->')" = 4 ] || { echo "    the diagram has edges the model does not"; exit 1; }
# node styling carries the derived status
printf '%s' "$g" | grep -q 'I0001\[.*\]:::ready' || { echo "    node styling does not reflect status"; exit 1; }

# --- an unknown dependency is refused by name
pj_issue I0009 M000 I9999
expect_exit 10 "$MJ" plan validate
expect_grep 'depends on I9999, which is not an issue'
rm .ai/repo/project/issues/I0009.yaml

# --- a self-dependency is a distinct finding, not a cycle report
pj_issue I0008 M000 I0008
expect_exit 10 "$MJ" plan validate
expect_grep 'I0008 — depends on itself'
rm .ai/repo/project/issues/I0008.yaml

# --- a two-node cycle is detected and both members are named
pj_issue I0006 M000 I0007
pj_issue I0007 M000 I0006
expect_exit 10 "$MJ" plan validate
expect_grep 'dependency cycle'
expect_grep 'I0006'
expect_grep 'I0007'

# --- a longer cycle is detected too
pj_issue I0006 M000 I0007
pj_issue I0007 M000 I0010
pj_issue I0010 M000 I0006
expect_exit 10 "$MJ" plan validate
expect_grep 'dependency cycle'
rm .ai/repo/project/issues/I0006.yaml .ai/repo/project/issues/I0007.yaml .ai/repo/project/issues/I0010.yaml
expect_exit 0 "$MJ" plan validate

# --- naming the same dependency twice is reported rather than counted twice
pj_issue I0011 M000 I0001 I0001
expect_exit 0 "$MJ" plan validate
expect_grep 'names I0001 more than once'
rm .ai/repo/project/issues/I0011.yaml

# --- two issues that could run together but touch the same path are serialised, conservatively
pj_issue I0012 M000
pj_issue I0013 M000
sed 's#^  - src/I0012$#  - src/shared#' .ai/repo/project/issues/I0012.yaml > /tmp/a.$$ && mv /tmp/a.$$ .ai/repo/project/issues/I0012.yaml
sed 's#^  - src/I0013$#  - src/shared/inner#' .ai/repo/project/issues/I0013.yaml > /tmp/b.$$ && mv /tmp/b.$$ .ai/repo/project/issues/I0013.yaml
expect_exit 0 "$MJ" plan validate
expect_grep 'shares src/shared with I0013'
expect_exit 0 "$MJ" plan waves
expect_grep 'serialised by scope overlap'

# --- only issues that can run are paired, each pair once, in record order, and a scope's
#     leading ./ and trailing / do not hide an overlap (I0820)
pj_issue I0014 M000
sed 's#^  - src/I0014$#  - ./src/shared/#' .ai/repo/project/issues/I0014.yaml > /tmp/c.$$ && mv /tmp/c.$$ .ai/repo/project/issues/I0014.yaml
# I0002 waits on I0001, so it is not READY and shares nothing with anybody
sed 's#^  - src/I0002$#  - src/shared#' .ai/repo/project/issues/I0002.yaml > /tmp/d.$$ && mv /tmp/d.$$ .ai/repo/project/issues/I0002.yaml
expect_exit 0 "$MJ" plan validate
got="$(printf '%s\n' "$LAST_OUT" | grep -oE 'I00[0-9]+ — shares [^ ]+ with I00[0-9]+')"
want="I0012 — shares src/shared with I0013
I0012 — shares src/shared with I0014
I0013 — shares src/shared/inner with I0014"
[ "$got" = "$want" ] || { printf '    scope conflicts:\n%s\n    expected:\n%s\n' "$got" "$want"; exit 1; }

# the doctor prints the model's findings in one pass, as the lines mj_finding prints. The
# fixture has no hooks and no projections, so doctor fails for its own reasons; only the dag
# line is asked about.
LAST_OUT="$("$MJ" doctor --json 2>&1)" || true
expect_grep '^\{"level":"WARN","category":"dag","subject":"I0012","message":"shares src/shared with I0013; they may not run concurrently","reproduce":"majordomus plan show I0012"\}$'
