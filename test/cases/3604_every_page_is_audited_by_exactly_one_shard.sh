# majordomus-covers: none
# The UI audit runs as N shards side by side (I3604), cut by the stride scripts/site-probe
# --partition defines for every browser gate (I2841, case 3602). The audit itself is held by
# case 85 and, split against a single runner on the real executable, by case 3605. This case
# holds what needs no browser:
#
#   wired      the workflow runs ui-audit-shard 1..N, each told UI_AUDIT_SHARD=<shard>/N; the
#              ui-audit job merges them with scripts/ui merge; the ui job audits nothing itself
#   whole      scripts/ui merge of every shard's document and route report is one complete
#              document, with the shape a single runner writes
#   refused    a missing shard, a route two shards audited, and shards planned at other widths
#              are each refused (exit 10) and nothing is written
#   malformed  UI_AUDIT_SHARD=4/3 is refused (exit 2) before a server or browser is started
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || skip "no node"
command -v jq >/dev/null 2>&1 || skip "no jq"
UI="$ROOT/scripts/ui"
WF="$ROOT/.github/workflows/validate.yml"

job_of() { awk -v j="  $1:" '$0 == j {f=1; next} f && /^  [a-z][a-z-]*:$/{exit} f' "$WF"; }
job="$(job_of ui-audit-shard)"
[ -n "$job" ] || { echo "    FAIL: validate.yml has no ui-audit-shard job"; exit 1; }
shards="$(printf '%s\n' "$job" | sed -n 's/^ *shard: *\[\(.*\)\] *$/\1/p' | tr -d ' ')"
n="$(printf '%s\n' "$shards" | tr ',' '\n' | grep -c .)"
[ "$shards" = "$(seq 1 "$n" | paste -sd, -)" ] || { echo "    FAIL: the ui-audit-shard matrix is [$shards], not 1..$n"; exit 1; }
printf '%s\n' "$job" | grep -qF "UI_AUDIT_SHARD: \${{ matrix.shard }}/$n" \
  || { echo "    FAIL: the shard job does not hand each shard UI_AUDIT_SHARD=<shard>/$n"; exit 1; }
job_of ui-audit | grep -q 'scripts/ui merge --reports' || { echo "    FAIL: the ui-audit job does not merge the shards"; exit 1; }
! job_of ui | grep -q '^ *run:.*scripts/ui audit' || { echo "    FAIL: the ui job still audits every page beside the shards"; exit 1; }
echo "    ok: the workflow runs shards 1..$n, each told UI_AUDIT_SHARD=<shard>/$n; ui-audit merges them; the ui job audits nothing"

# three shards' documents and route reports, as the shard jobs leave them
part() { # $1 dir, $2 K, $3 N, $4 viewports json, $5... routes
  local d="$1" k="$2" n="$3" vp="$4"; shift 4
  mkdir -p "$d/$k"
  { printf '# shard %s/%s of 6 routes\n' "$k" "$n"; printf '%s\n' "$@"; } > "$d/$k/ui-audit-routes.txt"
  jq -n --arg k "$k/$n" --argjson vp "$vp" --args '{schema: "ui-audit/v1", origin: "http://127.0.0.1:8790",
    complete: false, source: {pages: "fixture"}, breakpoints: [640], viewports: $vp, surfaces: [{id: "app"}],
    pages: ($ARGS.positional | length), visits: (($ARGS.positional | length) * 2), seconds: 5, concurrency: 2,
    rules: {}, foreign: [], shard: $k, routes: $ARGS.positional,
    findings: [$ARGS.positional[] | select(. == "/b/") | {route: ., width: 320, tier: "full", rule: "overflow.document", detail: "x"}]}' \
    "$@" > "$d/$k/run-ui.json"
}
part whole 1 3 '[320,1280]' /a/ /d/
part whole 2 3 '[320,1280]' /b/ /e/
part whole 3 3 '[320,1280]' /c/ /f/
rc=0; MAJORDOMUS_BIN=/usr/bin/true "$UI" merge --reports whole --out "$PWD/merged.json" > out 2>&1 || rc=$?
[ "$rc" = 10 ] || { echo "    FAIL: a merge holding one finding exited $rc, not 10"; cat out; exit 1; }
got="$(jq -c '{schema, complete, pages, visits, rules, n: (.findings | length), shard, routes}' merged.json)"
[ "$got" = '{"schema":"ui-audit/v1","complete":true,"pages":6,"visits":12,"rules":{"overflow.document":1},"n":1,"shard":null,"routes":null}' ] \
  || { echo "    FAIL: the merged document is $got"; exit 1; }
echo "    ok: three shards merge into one complete document of 6 pages and 12 visits, its finding kept"

refuse() { # $1 dir, $2 what, $3 what the refusal names
  rc=0; MAJORDOMUS_BIN=/usr/bin/true "$UI" merge --reports "$1" --out "$PWD/$1.json" > out 2>&1 || rc=$?
  [ "$rc" = 10 ] && [ ! -e "$PWD/$1.json" ] && grep -q "$3" out || { echo "    FAIL: $2 was merged (exit $rc)"; cat out; exit 1; }
}
cp -R whole missing; rm -r missing/2
refuse missing 'a missing shard' 'not one whole sweep'
mkdir twice; part twice 1 2 '[320,1280]' /a/ /b/ /c/; part twice 2 2 '[320,1280]' /c/ /d/ /e/ /f/
refuse twice 'a route two shards audited' 'not one whole sweep'
cp -R whole widths; part widths 3 3 '[320]' /c/ /f/
refuse widths 'shards planned at other widths' 'disagree on viewports'
echo "    ok: a missing shard, a route audited twice and shards planned at other widths are each refused, nothing written"

rc=0; UI_AUDIT_SHARD=4/3 MAJORDOMUS_BIN=/usr/bin/true "$UI" audit > out 2>&1 || rc=$?
[ "$rc" = 2 ] && grep -q 'UI_AUDIT_SHARD must be K/N' out || { echo "    FAIL: UI_AUDIT_SHARD=4/3 exited $rc"; cat out; exit 1; }
echo "    ok: a malformed shard is refused before a server or a browser starts"
