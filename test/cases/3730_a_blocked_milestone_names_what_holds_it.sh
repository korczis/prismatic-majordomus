# majordomus-covers: plan
# A BLOCKED milestone names what holds it (I3757). A milestone is BLOCKED when none of its
# issues is ready; when every milestone it requires is DONE its blocked_by is empty, and the
# reason lies in its issues. Both engines follow each BLOCKED issue's blocked_by until an issue
# that is not BLOCKED, or a milestone gate, and name what they reach as waits_on:
#
#   M001  every open issue waits on I0001, READY in M000      waits_on I0001, named once
#   M002  waits on I0003 (BLOCKED, outside), which waits on    waits_on I0002, not I0003
#         I0002 (ACTIVE)
#   M003  gated by M000, which is not DONE                     blocked_by M000, waits_on empty
#   M004  one READY issue                                      not BLOCKED, waits_on empty
#   M005  two issues waiting only on each other               blocked_without_reason, both
#                                                              engines, beside the cycle
#
# The shell (`plan roadmap --json`, lib/project.awk) and the executable (/api/v1/plan/roadmap,
# plan.rs) are compared field for field; the /ui inspector draws the holding issue as a link,
# and so does /roadmap/ when Zola is here to render it, in one sentence translated once.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST
fail() { printf '    %s\n' "$*"; exit 1; }

"$MJ" init >/dev/null
pj_init
n=0; for m in M000 M001 M002 M003 M004; do pj_milestone "$m" "$n"; n=$((n + 1)); done
printf 'depends_on:\n  - M000\n' >> .ai/repo/project/milestones/M003.yaml
pj_issue I0001 M000
pj_issue I0002 M000
pj_issue I0003 M000 I0002
pj_issue I0004 M001 I0001
pj_issue I0005 M001 I0001 I0004
pj_issue I0006 M002 I0003
pj_issue I0007 M003
pj_issue I0008 M004
pj_issue I0009 M004 I0001
git add -A >/dev/null && git commit -qm "a plan held by its issues"
"$MJ" plan start I0002 >/dev/null || fail "plan start I0002 refused"
git add -A >/dev/null && git commit -qm "I0002 starts"
expect_exit 0 "$MJ" plan validate || exit 1

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 90 "$U$1"; }

# ---------------------------------------------------------------- both engines
"$MJ" plan roadmap --json | jq -S -c '.milestones[] | {id, status, blocked_by, waits_on}' \
  | LC_ALL=C sort > sh.roadmap
get /api/v1/plan/roadmap | jq -S -c '.milestones[] | {id, status, blocked_by, waits_on}' \
  | LC_ALL=C sort > rs.roadmap
[ -s sh.roadmap ] || fail "the shell's roadmap is empty"
cmp -s sh.roadmap rs.roadmap || { diff sh.roadmap rs.roadmap | sed 's/^/    | /'; fail "the engines disagree on waits_on"; }
want() { # <milestone> <jq predicate over the row>
  jq -e --arg m "$1" "select(.id == \$m) | $2" sh.roadmap >/dev/null \
    || fail "$1: $(jq -c --arg m "$1" 'select(.id == $m)' sh.roadmap), expected $2"
}
# MUTATION: stop at the first entry of blocked_by without following a BLOCKED one -> M002 names I0003
want M001 '.status == "BLOCKED" and .blocked_by == [] and .waits_on == ["I0001"]'
want M002 '.status == "BLOCKED" and .waits_on == ["I0002"]'
# MUTATION: walk a gated milestone too -> M003 names milestone M003's own gate
want M003 '.status == "BLOCKED" and .blocked_by == ["M000"] and .waits_on == []'
want M004 '.status != "BLOCKED" and .waits_on == []'
want M000 '.waits_on == []'

# ---------------------------------------------------------------- the /ui inspector
page="$(get '/ui/roadmap?milestone=M002')"
printf '%s' "$page" | grep -qF 'data-held-by="I0002"' || fail "the M002 inspector draws no Held by I0002"
printf '%s' "$page" | grep -qF '?issue=I0002#issue"' || fail "the M002 inspector does not link I0002"
get '/ui/roadmap?milestone=M003' | grep -qF 'Held by' && fail "a gated milestone's inspector draws Held by"

# ---------------------------------------------------------------- /roadmap/
# The page of a site copy whose roadmap is this fixture's: the holding issue is a link inside
# the one element the translation leaves alone, and a milestone with an empty waits_on says
# nothing of it. The sentence carries no id, and the Czech memory carries it.
if mj_zola_ready "$ROOT" && mj_runs node; then
  mkdir -p S
  ( cd "$ROOT/site" && tar cf - --exclude ./public . ) | ( cd S && tar xf - )
  mkdir -p S/static/js S/static/graphs
  : > S/static/app.css; : > S/static/kit.css
  for js in "$ROOT"/site/*.js; do cp "$js" S/static/js/; done
  "$MJ" plan roadmap --json > S/data/generated/roadmap.json
  ( cd S && zola build --force --output-dir ../out ) > zola.log 2>&1 \
    || { tail -20 zola.log; fail "zola could not render the site copy"; }
  R=out/roadmap/index.html
  node --input-type=module - "$ROOT/scripts/lib/site-i18n.mjs" "$R" "$ROOT/site/i18n/cs/memory" <<'EOF' || exit 1
const [lib, page, memdir] = process.argv.slice(2);
const fs = await import('node:fs');
const m = await import(lib);
const html = fs.readFileSync(page, 'utf8');
const card = (id) => (html.match(new RegExp(`<li id="milestone-${id}"[\\s\\S]*?(?=<li id="milestone-|</ol>)`)) ?? [''])[0];
const bad = [];
const held = card('M002').match(/data-milestone-held-by="([^"]*)"[^>]*>([\s\S]*?)<\/p>/);
if (!held) bad.push('M002 has no held-by sentence');
else {
  if (held[1] !== 'I0002') bad.push(`M002 names ${held[1]}`);
  if (!/<span translate="no"><a href="[^"]*\/plan\/i0002\/"[^>]*>I0002<\/a><\/span>/.test(held[2]))
    bad.push(`I0002 is not a link inside the untranslated element: ${held[2]}`);
}
for (const id of ['M003', 'M004', 'M000'])
  if (/data-milestone-held-by/.test(card(id))) bad.push(`${id} carries a held-by sentence`);
const mem = m.loadMemory(memdir);
const segs = m.segments(html).filter((s) => /^Every open issue of this milestone waits on/.test(s.source));
if (!segs.length) bad.push('no segment for the held-by sentence');
for (const s of segs) {
  if (/I\d{4}/.test(s.source)) bad.push(`an id in the segment: ${s.source}`);
  if (!mem.has(s.key)) bad.push(`no Czech for: ${s.source}`);
}
if (bad.length) { console.log('    ' + bad.join('\n    ')); process.exit(1); }
EOF
else
  echo "    /roadmap/ not rendered here: $(mj_zola_why "$ROOT")"
fi

# ---------------------------------------------------------------- a cycle reaches nothing
serve_down
pj_milestone M005 5
pj_issue I0011 M005 I0012
pj_issue I0012 M005 I0011
git add -A >/dev/null && git commit -qm "two issues waiting on each other"
"$MJ" plan validate --json > sh.validate 2>/dev/null || true
jq -e -s 'any(.[]; .category == "blocked_without_reason" and .subject == "M005")' sh.validate >/dev/null \
  || fail "the shell does not refuse M005 as blocked_without_reason"
jq -e -s 'any(.[]; .category == "cycle")' sh.validate >/dev/null || fail "the shell lost the cycle finding"
serve_up serve.out serve.err || exit 1
get /api/v1/plan/validate > rs.validate
jq -e '[.findings[] | select(.code == "blocked_without_reason" and .subject == "M005" and .level == "FAIL")] | length == 1' \
  rs.validate >/dev/null || fail "the executable does not refuse M005 as blocked_without_reason"
jq -e 'any(.findings[]; .code == "cycle")' rs.validate >/dev/null || fail "the executable lost the cycle finding"
get /api/v1/plan/roadmap | jq -e '.milestones[] | select(.id == "M005") | .status == "BLOCKED" and .waits_on == []' >/dev/null \
  || fail "M005 is not BLOCKED with an empty waits_on"
echo "    waits_on agrees in both engines, links on both surfaces, and a cycle is refused"
