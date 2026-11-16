# majordomus-covers: none
# majordomus-skip-runs-in: site
# The plan's graph is drawn by milestone, so /plan/dag/ answers the site probe within its
# budget however large the backlog grows (I3841).
#
# On 2026-10-21 /plan/dag/ did not answer the site probe within 90 s on master 97391192dbb: it
# drew the mermaid graph of every issue (824 nodes, 82,663 characters), printed every issue edge
# (779 rows) and every wave (801 issues) again, 26,881 elements live. Both plan pages now draw
# plan.json's milestone graph, one node per milestone, and each milestone's page draws its
# issues. Asserted here:
#
#   1. the data: milestone_edges counts exactly the issue dependencies that cross milestones,
#      one entry per pair, each naming the issue pairs behind it, and milestone_mermaid has one node per milestone and one arrow per edge
#   2. the templates draw the milestone graph, never the graph of every issue
#   3. the built /plan/dag/ stays under ELEMENT_CEILING elements and draws no issue node
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || skip "jq absent"
mj_runs python3 || skip "$(mj_runs_why python3)"

# The ceiling, on the elements the HTML carries: one row and one link per milestone edge, the
# graph's link list per milestone, the page's frame. 3,000 is twice what 100 milestones and 111
# edges need at the tree this case lands with; a page that needs more is one to split again.
ELEMENT_CEILING=3000
PLAN="$ROOT/site/data/generated/plan.json"

# ---------------------------------------------------------------- 1. the data
[ -f "$PLAN" ] || { echo "    $PLAN is missing; run scripts/generate-site-data"; exit 1; }
verdict="$(jq -r '
  . as $p
  | ($p.issues | map({key: .id, value: .milestone}) | from_entries) as $of
  | ([$p.edges[] | select($of[.from] != null and $of[.to] != null and $of[.from] != $of[.to])] | length) as $crossing
  | ($p.milestone_edges | map(.count) | add // 0) as $counted
  | ($p.milestone_edges | map([.from, .to]) | unique | length) as $pairs
  | ($p.milestone_mermaid | split("\n")) as $lines
  | ([$lines[] | select(test("^    M_[A-Za-z0-9_]+\\["))] | length) as $nodes
  | ([$lines[] | select(test("^    M_[A-Za-z0-9_]+ -->\\|[0-9]+\\| M_"))] | length) as $arrows
  | ([$lines[] | select(test("^    I[0-9]+\\["))] | length) as $issues
  | ([$p.milestone_edges[] | select((.issues | length) != .count)] | length) as $unnamed
  | if ($p.milestone_edges | type) != "array" then "plan.json carries no milestone_edges"
    elif $counted != $crossing then "the milestone edges count \($counted) issue dependencies and \($crossing) cross milestones"
    elif $pairs != ($p.milestone_edges | length) then "a pair of milestones appears in more than one edge"
    elif $nodes != ($p.milestones | length) then "\($nodes) node(s) for \($p.milestones | length) milestones"
    elif $arrows != ($p.milestone_edges | length) then "\($arrows) arrow(s) for \($p.milestone_edges | length) edges"
    elif $issues > 0 then "the milestone graph draws \($issues) issue node(s)"
    elif $unnamed > 0 then "\($unnamed) milestone edge(s) name a different number of issue pairs than they count"
    else "ok" end' "$PLAN")"
[ "$verdict" = ok ] || { echo "    the milestone graph does not match the plan: $verdict"; exit 1; }

# ---------------------------------------------------------------- 2. the templates
for t in dag.html plan-section.html; do
  grep -qF 'mermaid={plan.mermaid}' "$ROOT/site/templates/$t" \
    && { echo "    site/templates/$t draws the graph of every issue again; it is what timed the probe out"; exit 1; }
  grep -qF 'mermaid={plan.milestone_mermaid}' "$ROOT/site/templates/$t" \
    || { echo "    site/templates/$t no longer draws the milestone graph; this case is guarding nothing"; exit 1; }
done

# ---------------------------------------------------------------- 3. the built page
PAGE="$ROOT/site/public/plan/dag/index.html"
[ -f "$PAGE" ] || skip "site not built; the rendered page cannot be measured"
# Start tags are counted, so a "<" in text is no element and a tag in a comment is one: the
# count errs high, never low, the safe side for a ceiling.
measured="$(python3 -I -c '
import re, sys
s = open(sys.argv[1], encoding="utf-8").read()
print(len(re.findall(r"<[A-Za-z][A-Za-z0-9-]*", s)), len(re.findall(r"^\s*I[0-9]+\[", s, re.M)))' "$PAGE")"
elements="${measured% *}"; issue_nodes="${measured#* }"
[ "$elements" -le "$ELEMENT_CEILING" ] || {
  echo "    /plan/dag/ has $elements elements, over the ceiling of $ELEMENT_CEILING the site probe's budget needs"
  exit 1
}
[ "$issue_nodes" = 0 ] || { echo "    /plan/dag/ still draws $issue_nodes issue node(s)"; exit 1; }
