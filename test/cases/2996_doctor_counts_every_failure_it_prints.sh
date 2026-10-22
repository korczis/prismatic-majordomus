# majordomus-covers: none
# majordomus-negative: doctor
# The number a reporter prints, and the exit code it returns, account for every finding it
# printed.
#
# A finding counts itself: mj_finding prints the line and increments MJ_FINDINGS, and
# MJ_FAILS for a FAIL or a DRIFT (lib/common.sh). `doctor: N failure(s)` and doctor's exit
# code are read from those counters. A loop fed by a pipeline runs in a subshell, so a
# reporter called from one prints its line and counts it in a shell that is gone by the time
# anybody reads the count. An adopting repository saw exactly that on 2026-10-13: ten FAIL
# lines, `doctor: 8 failure(s)` — the two it lost were the findings of `reasoning check`,
# reported from `jq … | while read …; do mj_fail reasoning …; done`. With only those two left
# doctor would have printed FAIL twice and exited 0, which a pre-commit hook reads as
# permission (docs/ADOPTION_SECOND_RUN.md, G02).
#
# Two halves. The structural one refuses the shape anywhere in the shell tool, so a new
# reporter cannot reintroduce it at a site the behavioural half does not walk. The
# behavioural one holds what a reader sees, at the three places a lost count was visible:
# doctor's reasoning findings, doctor's dependency-graph findings, and `check --overlap`,
# which said "no other worktree claims an overlapping path" under the overlaps it had just
# listed because the count it asked was the parent's.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- structural
# Which functions count: mj_finding, and every function that reaches it through calls (the
# level wrappers, mj_doctrine_fail, and whatever a later change wraps them in — derived from
# the sources, never listed here). Where they may not be called from: the body of a loop a
# pipeline feeds (`… | while …; do … done`, the pipe on the loop's line or ending the line
# before it), and the receiving end of a pipe (`… | mj_reporter`). A loop whose `done` is
# not found before its function ends is reported as unread rather than passed over.
#
# What this does not read: a reporter inside `( … )` or `$( … )`. Both are written on
# purpose where they occur — a captured reporter is one whose output the caller wanted as a
# value — and neither is the shape that lost a count.
cat > "$T/subshell-reporters.awk" <<'AWK'
function strip(s) {
  gsub(/\\./, "", s); gsub(/"[^"]*"/, "\"\"", s); gsub(/'[^']*'/, "", s)
  sub(/(^|[ \t])#.*$/, "", s); return s
}
{
  file[NR] = FILENAME; line[NR] = $0; fnr[NR] = FNR
  if (match($0, /^[a-z_][a-z0-9_]*\(\)/)) cur = substr($0, RSTART, RLENGTH - 2)
  if (cur != "") {
    s = $0
    while (match(s, /mj_[a-z0-9_]+/)) { calls[cur, substr(s, RSTART, RLENGTH)] = 1; s = substr(s, RSTART + RLENGTH) }
  }
  if ($0 ~ /^}/ || $0 ~ /^[a-z_][a-z0-9_]*\(\)[ \t]*\{.*\}[ \t]*$/) cur = ""
}
END {
  reach["mj_finding"] = 1
  do {
    grew = 0
    for (k in calls) { split(k, p, SUBSEP); if (!(p[1] in reach) && (p[2] in reach)) { reach[p[1]] = 1; grew = 1 } }
  } while (grew)
  for (i = 1; i <= NR; i++) {
    s = strip(line[i])
    # the receiving end of a pipe
    t = s; gsub(/\|\|/, " ", t)
    while (match(t, /\|[ \t]*mj_[a-z0-9_]+/)) {
      name = substr(t, RSTART, RLENGTH); sub(/^\|[ \t]*/, "", name)
      if (name in reach) printf "%s:%d: %s reads a pipe, so what it reports is counted in a subshell\n", file[i], fnr[i], name
      t = substr(t, RSTART + RLENGTH)
    }
    # a loop a pipeline feeds
    same = (s ~ /\|[ \t]*(while|for|until)[ \t]/)
    prev = (i > 1 && file[i-1] == file[i] && strip(line[i-1]) ~ /[^|]\|[ \t]*\\?[ \t]*$/ && s ~ /^[ \t]*(while|for|until)[ \t]/)
    if (!same && !prev) continue
    if (same) { match(s, /\|[ \t]*(while|for|until)[ \t]/); s = substr(s, RSTART + 1) }
    depth = 0; opened = 0; closed = 0; body = ""
    for (j = i; j <= NR && file[j] == file[i]; j++) {
      t = (j == i) ? s : strip(line[j])
      if (j > i && line[j] ~ /^}/) break
      n = split(t, w, /[ \t;()|&<>]+/)
      for (k = 1; k <= n; k++) {
        if (w[k] == "do") { depth++; opened = 1 }
        else if (w[k] == "done") { depth--; if (opened && depth == 0) { closed = 1; break } }
        body = body " " w[k]
      }
      if (closed) break
    }
    if (!closed) { printf "%s:%d: a loop fed by a pipeline whose end this check could not find\n", file[i], fnr[i]; continue }
    hit = ""
    m = split(body, w, /[^a-z0-9_]+/)
    for (k = 1; k <= m; k++) if ((w[k] in reach) && !(w[k] in once)) { hit = hit " " w[k]; once[w[k]] = 1 }
    for (k in once) delete once[k]
    if (hit != "") printf "%s:%d: a loop fed by a pipeline calls%s, so what it reports is counted in a subshell\n", file[i], fnr[i], hit
  }
}
AWK
scan() { awk -f "$T/subshell-reporters.awk" "$@"; }

# The check is itself checked first: every shape planted is named, and the shapes that keep
# the count are left alone. A scanner that finds nothing in a file full of the defect would
# pass the real tree for the wrong reason.
cat > "$T/planted.sh" <<'SH'
mj_finding() { MJ_FAILS=$((MJ_FAILS + 1)); }
mj_fail()  { mj_finding FAIL "$@"; }
mj_info()  { mj_finding INFO "$@"; }
mj_planted_wrapper() {
  mj_fail planted "$1" "through a wrapper"
}
mj_planted_quiet() { printf '%s\n' "$1"; }
mj_planted_same_line() {
  printf '%s\n' "$1" | while read -r l; do mj_fail planted "$l" "same line"; done
}
mj_planted_continued() {
  printf '%s' "$1" | jq -r '.findings[]' | \
    while read -r l; do
      mj_info planted "$l" "continued"
    done
}
mj_planted_wrapped() {
  printf '%s\n' "$1" | sort | while read -r l; do
    for x in $l; do printf '%s\n' "$x"; done
    mj_planted_wrapper "$l"
  done
}
mj_planted_receiver() {
  printf '%s\n' "$1" | mj_planted_wrapper x
}
mj_planted_kept() {
  while read -r l; do mj_fail planted "$l" "counted"; done < <(printf '%s\n' "$1")
  printf '%s\n' "$1" | while read -r l; do mj_planted_quiet "$l"; done
  [ -n "$1" ] || mj_fail planted "-" "an or-list is not a pipe"
}
SH
planted="$(scan "$T/planted.sh")"
for want in 'planted.sh:9: .* calls mj_fail,' 'planted.sh:13: .* calls mj_info,' \
            'planted.sh:18: .* calls mj_planted_wrapper,' 'planted.sh:24: mj_planted_wrapper reads a pipe'; do
  printf '%s\n' "$planted" | grep -qE "$want" || { printf '    the structural check misses a planted defect (/%s/); it said:\n%s\n' "$want" "$planted"; exit 1; }
done
[ "$(printf '%s\n' "$planted" | grep -c .)" = 4 ] || { printf '    the structural check names something that keeps its count; it said:\n%s\n' "$planted"; exit 1; }

found="$(cd "$ROOT" && scan bin/majordomus lib/*.sh)"
if [ -n "$found" ]; then
  printf '    a reporter that counts in a subshell prints a finding nobody counts. Feed the loop\n'
  printf '    by process substitution instead — `while …; do …; done < <(producer)`:\n%s\n' "$found"
  exit 1
fi

# ---------------------------------------------------------------- behavioural
command -v jq >/dev/null 2>&1 || skip_case "no jq, so doctor never reads the reasoning check and the counts were not measured"

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
# neither declared hook is there in a fresh checkout, and their absence would be a failure of
# its own beside the ones this case counts
mkdir -p .git/hooks
printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push
PATH="$(dirname "$MJ"):$PATH"; export PATH

# The executable, as far as doctor's reasoning report needs one: what `reasoning check`
# answers is a fixture here, because the subject is what the shell does with the answer.
# test/cases/736_reasoning_check_refuses_drift.sh holds the answer itself.
STUB="$T/stub-cli"
cat > "$STUB" <<'STUB'
#!/bin/sh
case "$*" in
  'reasoning advisors'*) echo '{"mode":{"mode":"stub","source":"case"},"available":0,"advisors":[{"id":"one","status":"unavailable","reason":"executable_not_found"}]}' ;;
  'reasoning check'*)
    [ -f "$MJ_CASE_FINDINGS" ] || { echo '{"ok":true,"checks":["ci"],"findings":[]}'; exit 0; }
    cat "$MJ_CASE_FINDINGS"; exit 10 ;;
esac
exit 0
STUB
chmod +x "$STUB"
MJ_CASE_FINDINGS="$T/findings.json"; export MJ_CASE_FINDINGS
doctor() { MAJORDOMUS_BIN="$STUB" "$MJ" doctor; }
# every FAIL line doctor printed, against the number its last line states
counted() {
  local lines stated
  lines="$(printf '%s\n' "$LAST_OUT" | grep -c '^FAIL' || true)"
  stated="$(printf '%s\n' "$LAST_OUT" | sed -n 's/^doctor: \([0-9][0-9]*\) failure(s)$/\1/p')"
  [ "$lines" = "$1" ] || { printf '    expected %s FAIL line(s), doctor printed %s:\n%s\n' "$1" "$lines" "$LAST_OUT"; return 1; }
  [ "$stated" = "$lines" ] || { printf '    doctor printed %s FAIL line(s) and stated "%s":\n%s\n' "$lines" "$stated" "$LAST_OUT"; return 1; }
}

# healthy first, so that what follows is the only thing failing
expect_exit 0 doctor
counted 0
expect_grep '^OK   reasoning   - — reasoning check: ci — no finding'

# --- only reasoning findings: each is a failure, and doctor exits on them
cat > "$MJ_CASE_FINDINGS" <<'JSON'
{"ok":false,"checks":["ci","claims"],"findings":[
  {"check":"ci","subject":".github/workflows/review.yml","message":"CI names the model credential OPENAI_API_KEY; no gate may depend on a live model"},
  {"check":"claims","subject":"docs/REVIEW.md","message":"claims that consensus guarantees correctness"}]}
JSON
expect_exit 10 doctor
expect_grep '^FAIL reasoning   \.github/workflows/review\.yml — ci: CI names the model credential'
expect_grep '^FAIL reasoning   docs/REVIEW\.md — claims: '
counted 2
expect_no_grep '^FAIL (wiring|projection|bootstrap|project|dag)'
rm "$MJ_CASE_FINDINGS"

# --- the dependency graph: every finding is one failure, with no line added to stand for them
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0003 M000 I0004
pj_issue I0004 M000 I0003
expect_exit 10 doctor
expect_grep '^FAIL dag '
expect_no_grep '^FAIL (wiring|projection|bootstrap|reasoning)'
counted "$(printf '%s\n' "$LAST_OUT" | grep -c '^FAIL' || true)"
expect_no_grep 'dependency-graph failure\(s\)'
# watch counts the same findings as drift
expect_exit 11 "$MJ" watch
[ "$(printf '%s\n' "$LAST_OUT" | grep -c '^DRIFT' || true)" = "$(printf '%s\n' "$LAST_OUT" | sed -n 's/^watch: \([0-9][0-9]*\) drift finding(s)$/\1/p')" ] \
  || { printf '    watch states a number that is not its DRIFT lines:\n%s\n' "$LAST_OUT"; exit 1; }
rm .ai/repo/project/issues/I0003.yaml .ai/repo/project/issues/I0004.yaml
expect_exit 0 doctor
counted 0

# --- check --overlap: an overlap that was listed is not followed by "none"
git add -A >/dev/null 2>&1; git commit -qm "model" >/dev/null 2>&1 || true
expect_exit 0 "$MJ" start "mine" --scope lib/auth
expect_exit 0 "$MJ" check --overlap
expect_grep 'no other worktree claims an overlapping path'
wt="$T-wt-2996"; rm -rf "$wt"
trap 'git worktree remove --force "$wt" >/dev/null 2>&1; rm -rf "$wt"' EXIT
git worktree add -q "$wt" -b other
( cd "$wt" && "$MJ" start "theirs" --scope lib/auth/sub --owner other >/dev/null )
expect_exit 0 "$MJ" check --overlap
expect_grep 'INFO overlap +.* — claims lib/auth/sub — contained by your lib/auth'
expect_no_grep 'no other worktree claims an overlapping path'

echo "    ok: every finding printed is a finding counted, and no reporter in the shell tool counts in a subshell"
