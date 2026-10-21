# majordomus-covers: none
# majordomus-skip-runs-in: site
# The plan page renders within the audit budget at every width (I3838).
#
# On 2026-10-21 /plan/ (served at /docs/plan/) timed the UI audit out at 1536 px: 1.9 MB,
# 35,516 elements live, axe 14.4 s against 0.3 s on the home page. Half of it was the findings
# table, one row per finding, and 2,774 of its 2,797 rows were scope_conflict, which `plan
# validate` reports once per pair of issues sharing a path — a table that grows with the
# square of the backlog. The page now shows one row per (level, code), counted, with three
# examples, and the whole list stays in plan.json. Asserted here:
#
#   1. the data: findings_by_code partitions findings exactly — every finding counted once,
#      one entry per (level, code), at most three examples, each one a finding of its group
#   2. the template prints the groups, never one row per finding
#   3. the built page: its findings table has one row per group, and the whole page stays
#      under ELEMENT_CEILING elements, the bound the audit's per-visit deadline needs
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || skip "jq absent"
command -v python3 >/dev/null || skip "python3 absent"

# The ceiling, on the elements the HTML carries. axe, the audit's slowest pass, measured about
# 0.4 ms an element on a loaded machine (14.4 s over 35,516 before this case, 4.6 s over 21,589
# after it, both at 1536 px on 2026-10-21). The browser's DOM is larger than the HTML: the plan
# graph is drawn by mermaid from the page and adds about 12,700 SVG elements (8,894 in the HTML,
# 21,589 live). 15,000 in the HTML keeps the live page near 28,000 and axe near 11 s of the
# audit's 60 s per visit (scripts/lib/ui-audit.mjs VISIT_DEADLINE_MS), with every other pass and
# a CI runner's slower cores on top. A page that needs more is a page to split, not a ceiling
# to raise.
ELEMENT_CEILING=15000
PLAN="$ROOT/site/data/generated/plan.json"
TEMPLATE="$ROOT/site/templates/plan-section.html"

# ---------------------------------------------------------------- 1. the data
[ -f "$PLAN" ] || { echo "    $PLAN is missing; run scripts/generate-site-data"; exit 1; }
jq -e '.findings_by_code | type == "array"' "$PLAN" >/dev/null \
  || { echo "    plan.json carries no findings_by_code; the page has nothing bounded to render"; exit 1; }
verdict="$(jq -r '
  . as $p
  | (.findings | length) as $n
  | (.findings_by_code | map(.count) | add // 0) as $counted
  | (.findings_by_code | map([.level, .code]) | unique | length) as $kinds
  | (.findings | map([.level, .code]) | unique | length) as $declared
  | ([.findings_by_code[] | . as $g | .examples[] | select(.level != $g.level or .code != $g.code)] | length) as $strays
  | ([.findings_by_code[] | select((.examples | length) > 3 or (.examples | length) > .count)] | length) as $long
  | if $counted != $n then "the groups count \($counted) finding(s) and there are \($n)"
    elif $kinds != ($p.findings_by_code | length) then "a (level, code) appears in more than one group"
    elif $kinds != $declared then "\($declared) kind(s) of finding and \($kinds) group(s)"
    elif $strays > 0 then "\($strays) example(s) filed under a group they do not belong to"
    elif $long > 0 then "\($long) group(s) carry more than three examples, or more than they count"
    else "ok" end' "$PLAN")"
[ "$verdict" = ok ] || { echo "    findings_by_code does not partition findings: $verdict"; exit 1; }

# ---------------------------------------------------------------- 2. the template
grep -qE '\{%-? *for [a-z_]+ in plan\.findings *-?%\}' "$TEMPLATE" \
  && { echo "    $TEMPLATE prints one row per finding again; it is the table that timed the audit out"; exit 1; }
grep -qE 'for [a-z_]+ in plan\.findings_by_code' "$TEMPLATE" \
  || { echo "    $TEMPLATE no longer renders plan.findings_by_code; this case is guarding nothing"; exit 1; }

# ---------------------------------------------------------------- 3. the built page
PAGE="$ROOT/site/public/plan/index.html"
[ -f "$PAGE" ] || skip "site not built; the rendered page cannot be measured"
# The counter, held to a page whose answer is known before it is trusted with the real one. It
# counts start tags, so a "<" in text is no element and a tag inside a comment is one: it errs
# high, never low, which is the safe side for a ceiling.
count_elements() {
  python3 -I -c 'import re,sys; print(len(re.findall(r"<[A-Za-z][A-Za-z0-9-]*", open(sys.argv[1], encoding="utf-8").read())))' "$1"
}
probe="$(mktemp "${TMPDIR:-/tmp}/mj-3848.XXXXXX")"
trap 'rm -f "$probe"' EXIT
printf '<!doctype html><html><body><p>a <b>b</b> 1 < 2</p><!-- <i> --></body></html>\n' > "$probe"
[ "$(count_elements "$probe")" = 5 ] || { echo "    the element counter miscounts a known page ($(count_elements "$probe"), expected 5)"; exit 1; }

elements="$(count_elements "$PAGE")"
[ "$elements" -le "$ELEMENT_CEILING" ] || {
  echo "    /plan/ has $elements elements, over the ceiling of $ELEMENT_CEILING the audit's per-visit deadline needs"
  exit 1
}
groups="$(jq '.findings_by_code | length' "$PLAN")"
rows="$(python3 -I -c '
import re, sys
s = open(sys.argv[1], encoding="utf-8").read()
i = s.find(">Findings</h2>")
if i < 0: print(-1); sys.exit()
body = re.search(r"<tbody>(.*?)</tbody>", s[i:], re.S)
print(body.group(1).count("<tr") if body else -1)' "$PAGE")"
if [ "$(jq '.findings | length' "$PLAN")" -gt 0 ]; then
  [ "$rows" = "$groups" ] || { echo "    the findings table has $rows row(s) for $groups group(s)"; exit 1; }
fi
