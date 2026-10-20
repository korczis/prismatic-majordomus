# majordomus-covers: none
# majordomus-skip-runs-in: ui
# A split UI audit reports what one runner reports (I3604). The same few pages of the real UI
# are audited twice against the executable the ui job built: once by one runner, once as two
# shards (UI_AUDIT_SHARD=1/2 and 2/2) merged by scripts/ui merge. The two documents must agree on
# everything a reader of the report is told — pages, visits, rules and every finding's page,
# width and rule — and differ only in when, how fast and at which port they ran. Case 3604 holds
# the merge's refusals on fixtures; this is the claim against a real run.
#
# Needs the executable, a built site and a browser; the suite runners have none, so the case
# defers to the ui job (--runs-in ui), where a skip is a failure.
. "$ROOT/test/lib.sh"
if [ -n "${CI:-}" ] && [ "${MAJORDOMUS_RUNS_IN:-}" != ui ]; then
  skip "a split audit is compared with a single one in the ui job (--runs-in ui), not in this CI run"
fi
command -v node >/dev/null 2>&1 || skip "no node"
command -v jq >/dev/null 2>&1 || skip "no jq"
[ -d "$ROOT/node_modules/playwright" ] || skip "playwright is not installed (npm ci)"
[ -f "$ROOT/site/public/index.html" ] && [ -d "$ROOT/target/web/docs" ] || skip "the site and the documentation it serves are not built (the ui job builds both before its cases)"
UI="$ROOT/scripts/ui"
SEL="${MJ_SPLIT_AUDIT_ONLY:-/ui/}"

rc=0; "$UI" audit --only "$SEL" --limit 6 --port 8797 --no-build --out "$PWD/single.json" > single.out 2>&1 || rc=$?
if grep -q '^SKIP ui audit' single.out; then skip "no browser for the audit"; fi
[ "$rc" = 0 ] || [ "$rc" = 10 ] || { echo "    FAIL: the single audit exited $rc"; tail -20 single.out; exit 1; }
for k in 1 2; do
  mkdir -p "parts/$k"
  rc=0; UI_AUDIT_SHARD="$k/2" UI_AUDIT_REPORT="$PWD/parts/$k/ui-audit-routes.txt" \
    "$UI" audit --only "$SEL" --limit 6 --port 8797 --no-build --out "$PWD/parts/$k/run-ui.json" > "part.$k.out" 2>&1 || rc=$?
  [ "$rc" = 0 ] || [ "$rc" = 10 ] || { echo "    FAIL: shard $k/2 exited $rc"; tail -20 "part.$k.out"; exit 1; }
done
rc=0; "$UI" merge --reports parts --out "$PWD/merged.json" > merge.out 2>&1 || rc=$?
[ "$rc" = 0 ] || [ "$rc" = 10 ] || { echo "    FAIL: the merge exited $rc"; cat merge.out; exit 1; }

meaning='{pages, visits, rules, routes: ([.findings[].route] | unique), findings: ([.findings[] | [.route, .width, .rule]] | sort)}'
single="$(jq -cS "$meaning" single.json)"; merged="$(jq -cS "$meaning" merged.json)"
[ "$(jq -r .pages single.json)" -ge 2 ] || { echo "    FAIL: the selection holds $(jq -r .pages single.json) page(s); two shards need at least two"; exit 1; }
[ "$single" = "$merged" ] || { echo "    FAIL: the split audit reports otherwise than one runner"; echo "    single: $single"; echo "    merged: $merged"; exit 1; }
[ "$(jq -r .complete merged.json)" = true ] || { echo "    FAIL: the merged document does not say it is complete"; exit 1; }
echo "    ok: $(jq -r .pages single.json) page(s), $(jq -r .visits single.json) visit(s) of $SEL: two shards merged report what one runner reports"
