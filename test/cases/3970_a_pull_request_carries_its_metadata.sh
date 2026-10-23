# majordomus-covers: none
# claims: github-projection
# A pull request carries its metadata, projected from the canonical records (I3970).
#
# `scripts/github-sync --prs` derives, for each open pull request, the issues it serves (the
# I-ids of its title and commit subjects, then the issue records its head changes), and from
# the trunk's records its labels (type, priority, train, records), its milestone, its author
# as assignee when nobody is, and a `Refs #<n>` region in its body; a record only the head
# carries is named by its id and lends no priority and no milestone. This drives it against a scripted `gh`
# that keeps the remote's state in files, so an apply is observed as GitHub would hold it and
# the second run is judged against the first one's writes.
#
# Mutations run on 2026-10-22, each failing this case where it says:
#   - `pr_safe` returns its argument unchanged: the title `Fixes #3` reaches the region, the
#     adapter's own guard refuses it (FAIL closing), and section 1 fails
#   - the region is always taken as intact (`|| true` on the hash test): section 4 fails,
#     the person's edit read as behind
#   - `del` drawn from every label rather than the four families: section 1 fails, `-hold`
#     is planned
. "$ROOT/test/lib.sh"
# A fixture's git must answer about the fixture: under a hook these name another repository.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR GIT_PREFIX

SYNC="$ROOT/scripts/github-sync"
fail() { printf '    %s\n' "$*"; exit 1; }
CLOSING='(^|[^[:alnum:]])(close|closes|closed|fix|fixes|fixed|resolve|resolves|resolved)[[:space:]]*:?[[:space:]]*#[0-9]+'

# ---------------------------------------------------------------- the scripted forge
# The remote is four files: the pull requests (the adapter's ten columns), the milestones,
# the issues and the labels. Reads print them; the six writes --prs makes change them; any
# other call fails the case.
FORGE="$T/forge"; GH_LOG="$T/gh.log"
FX_P="$T/prs.tsv"; FX_M="$T/ms.tsv"; FX_I="$T/issues.tsv"; FX_L="$T/labels.txt"
mkdir -p "$FORGE"; : > "$GH_LOG"
cat > "$FORGE/gh" <<'GH'
#!/usr/bin/env bash
set -eu
printf '%s\n' "$*" >> "$GH_LOG"
col()    { awk -F'\t' -v n="$1" -v c="$2" '$1 == n { print $c }' "$FX_P"; }
setcol() { awk -F'\t' -v OFS='\t' -v n="$1" -v c="$2" -v v="$3" '$1 == n { $c = v } { print }' "$FX_P" > "$FX_P.new"
           mv "$FX_P.new" "$FX_P"; }
csv()    { awk 'NF' | LC_ALL=C sort -u | paste -sd, -; }
case "$1 ${2:-}" in
  "auth status") exit 0 ;;
  "label list") cat "$FX_L"; exit 0 ;;
  "label create") printf '%s\n' "$3" >> "$FX_L"; exit 0 ;;
  # the issues as the adapter reads them (I3920): gh issue list, held to GraphQL's count
  "issue list") cat "$FX_I"; exit 0 ;;
  "api graphql") if [ -n "${FX_COUNT:-}" ]; then echo "$FX_COUNT"; else wc -l < "$FX_I" | tr -d ' '; fi; exit 0 ;;
esac
[ "$1" = api ] || { echo "scripted gh: unexpected call: $*" >&2; exit 1; }
shift
case "$*" in
  "--paginate repos/example/fixture/pulls?state=open"*) awk -F'\t' '$10 == "open"' "$FX_P"; exit 0 ;;
  "--paginate repos/example/fixture/milestones?"*) cat "$FX_M"; exit 0 ;;
  repos/example/fixture/pulls/[0-9]*" --jq "*) n="${1##*/}"; awk -F'\t' -v n="$n" '$1 == n' "$FX_P"; exit 0 ;;
esac
method="$2"; path="$3"; shift 3
n="$(printf '%s' "$path" | sed -n 's|^repos/example/fixture/[a-z]*/\([0-9]*\).*|\1|p')"
case "$method $path" in
  "POST repos/example/fixture/issues/$n/labels")
    have="$(col "$n" 6)"
    new="$( { printf '%s\n' "$have" | tr ',' '\n'; while [ $# -gt 0 ]; do printf '%s\n' "${2#*=}"; shift 2; done; } | csv)"
    setcol "$n" 6 "$new" ;;
  "DELETE repos/example/fixture/issues/$n/labels/"*)
    gone="${path##*/labels/}"
    setcol "$n" 6 "$(col "$n" 6 | tr ',' '\n' | grep -vxF "$gone" | csv)" ;;
  "PATCH repos/example/fixture/issues/$n")
    num="${2#milestone=}"
    setcol "$n" 7 "$(awk -F'\t' -v m="$num" '$1 == m { print $2 }' "$FX_M")" ;;
  "POST repos/example/fixture/issues/$n/assignees") setcol "$n" 8 "${2#*=}" ;;
  "PATCH repos/example/fixture/pulls/$n") setcol "$n" 9 "$(base64 < "${2#body=@}" | tr -d '\n')" ;;
  *) printf 'UNEXPECTED %s %s %s\n' "$method" "$path" "$*" >> "$GH_LOG"; echo "scripted gh: unexpected call" >&2; exit 1 ;;
esac
GH
chmod +x "$FORGE/gh"
export GH_LOG FX_P FX_M FX_I FX_L

sync() { env -u MJ_GH_FIXTURE_PRS -u MJ_GH_FIXTURE_ISSUES -u MJ_GH_FIXTURE_MILESTONES -u MJ_GH_REMOTE \
  -u MJ_GH_PR_ASSIGNEE PATH="$FORGE:$PATH" MJ_GH_PACE=0 "$SYNC" --prs "$@"; }
body_of()   { awk -F'\t' -v n="$1" '$1 == n { print $9 }' "$FX_P" | base64 --decode; }
labels_of() { awk -F'\t' -v n="$1" '$1 == n { print $6 }' "$FX_P"; }
field_of()  { awk -F'\t' -v n="$1" -v c="$2" '$1 == n { print $c }' "$FX_P"; }
writes()    { grep -cE '^(api -X |label create )' "$GH_LOG" || true; }

# ---------------------------------------------------------------- the plan and its branches
"$MJ" init >/dev/null
pj_init
pj_milestone M000
pj_milestone M001
pj_issue I0001 M000
pj_issue I0002 M001
# a title that would be a closing keyword if it were written as it stands
sed -e 's/^title: .*/title: "Fixes #3 in the parser"/' -e 's/^priority: .*/priority: p0/' \
  .ai/repo/project/issues/I0002.yaml > "$T/i2" && mv "$T/i2" .ai/repo/project/issues/I0002.yaml
# a projection the derive writes beside the records
mkdir -p derived; printf 'derived/** merge=derived\n' >> .gitattributes; printf '{}\n' > derived/plan.json
git add -A >/dev/null && git commit -qm "the plan" >/dev/null && git checkout -q -B master
git update-ref refs/remotes/origin/master HEAD
on() { git checkout -q -b "$1" master; }
change() { mkdir -p src; printf '%s\n' "$2" >> "src/$1"; git add -A >/dev/null; git commit -qm "$3" >/dev/null; }

on feature/a
change a one "feat(core): the thing (I0001)"
change a two "fix(core): its follow-up for I0002"
on chore/records
sed 's/^objective: .*/objective: "Recorded again."/' .ai/repo/project/issues/I0002.yaml > "$T/i2" \
  && mv "$T/i2" .ai/repo/project/issues/I0002.yaml
printf '{"restated":true}\n' > derived/plan.json
git add -A >/dev/null && git commit -qm "docs(plan): the record is restated" >/dev/null
on integration/train-20261022
git merge -q --no-ff feature/a -m "merge: feature/a into train 1" >/dev/null
on fix/no-issue
change b typo "fix: a typo"
on feature/new
pj_issue I0003 M000
change c three "feat(core): new work (I0003), after I0999"
git checkout -q master

sha() { git rev-parse "$1"; }
b64() { printf '%s' "$1" | base64 | tr -d '\n'; }
pr() { # <n> <title> <branch> <labels> <assignees> <body>   (every one written by `writer`)
  printf '%s\t%s\t%s\t%s\tmaster\t%s\t\t%s\t%s\topen\twriter\n' "$1" "$2" "$3" "$(sha "$3")" "$4" "$5" "$(b64 "$6")"
}
{ pr 10 'feat(core): the thing (I0001)' feature/a 'hold,type:fix' '' 'Words a person wrote.'
  pr 11 'docs(plan): the record is restated' chore/records '' someone ''
  pr 12 'chore(top): train 1' integration/train-20261022 '' '' ''
  pr 13 'fix: a typo' fix/no-issue '' '' ''
  pr 14 'feat(core): new work (I0003)' feature/new '' '' ''
} > "$FX_P"
printf '1\tM000 — Milestone M000\topen\n2\tM001 — Milestone M001\topen\n' > "$FX_M"
irow() { printf '%s\t%s — Issue\topen\t%s\tM000 — Milestone M000\t%s\n' "$1" "$2" "$(b64 "<!-- majordomus:record $2 -->")" "$2"; }
{ irow 7 I0001; irow 8 I0002; } > "$FX_I"
printf 'hold\npriority:p0\npriority:p1\n' > "$FX_L"
cp "$FX_P" "$T/prs.at-start"

# ---------------------------------------------------------------- 1. offline, this branch
git checkout -q feature/a
expect_exit 0 sync
expect_grep 'serves: +I0001 I0002$'
expect_grep 'labels: +priority:p0 type:feat$'
expect_grep 'milestone: M000 \(not on GitHub\)'
[ ! -s "$GH_LOG" ] || fail "the offline plan called gh: $(head -n 3 "$GH_LOG")"
git checkout -q master

# a fixture remote is read, and never written
expect_exit 11 env MJ_GH_FIXTURE_PRS="$FX_P" MJ_GH_FIXTURE_ISSUES="$FX_I" MJ_GH_FIXTURE_MILESTONES="$FX_M" "$SYNC" --prs --check
expect_grep 'DRIFT  labels +pr #10 labels: \+priority:p0 \+type:feat -type:fix'
expect_exit 15 env MJ_GH_FIXTURE_PRS="$FX_P" "$SYNC" --prs --apply
[ ! -s "$GH_LOG" ] || fail "a fixture run called gh: $(head -n 3 "$GH_LOG")"

# ---------------------------------------------------------------- 2. what is owed, then applied
expect_exit 11 sync --check
expect_grep 'DRIFT  milestone pr #10 is on .no milestone., canonically .M000 — Milestone M000. \(the milestone of I0001\)'
expect_grep 'DRIFT  assignee  pr #10 has no assignee; its author is writer'
expect_grep 'DRIFT  missing   pr #10 body has no metadata region'
expect_grep 'SKIP   named     pr #14 names I0999, which no record carries'
cmp -s "$FX_P" "$T/prs.at-start" || fail "--check changed the remote"
# a backfill from a clone may leave the labels to the workflow (a person's label is a
# `labeled` event, and validate runs on every one)
expect_exit 11 sync --fields milestone,assignee,body --check
expect_grep 'DRIFT  assignee  pr #10'
expect_no_grep 'DRIFT  labels'
expect_exit 2 sync --fields reviewers --check
[ "$(writes)" = 0 ] || fail "--check wrote to GitHub"

expect_exit 0 sync --apply
expect_grep 'github-sync prs apply: 5 pull request\(s\), 5 with drift'
grep -q UNEXPECTED "$GH_LOG" && fail "the adapter made a call the forge does not know: $(grep UNEXPECTED "$GH_LOG")"

# labels: its type, the highest priority it serves, a stale type removed, a person's label kept
[ "$(labels_of 10)" = 'hold,priority:p0,type:feat' ] || fail "pr 10 labels are '$(labels_of 10)'"
[ "$(labels_of 11)" = 'priority:p0,records,type:docs' ] || fail "pr 11 labels are '$(labels_of 11)'"
[ "$(labels_of 12)" = 'priority:p0,train,type:chore' ] || fail "pr 12 labels are '$(labels_of 12)'"
# a record only the head carries lends no priority and no milestone, and code beside a record
# is not `records`
[ "$(labels_of 14)" = 'type:feat' ] || fail "pr 14 labels are '$(labels_of 14)'"
[ -z "$(field_of 14 7)" ] || fail "pr 14 took milestone '$(field_of 14 7)' from a record not on the trunk"
for l in type:feat type:docs type:chore type:fix type:ci type:build records train; do
  grep -qxF "$l" "$FX_L" || fail "label $l was never created"
done
# milestone: the first served issue's canonical one
[ "$(field_of 10 7)" = 'M000 — Milestone M000' ] || fail "pr 10 milestone is '$(field_of 10 7)'"
[ "$(field_of 11 7)" = 'M001 — Milestone M001' ] || fail "pr 11 milestone is '$(field_of 11 7)'"
# assignee: its author when nobody is, and a person's choice is kept
[ "$(field_of 10 8)" = writer ] || fail "pr 10 assignee is '$(field_of 10 8)'"
[ "$(field_of 11 8)" = someone ] || fail "pr 11 assignee was replaced: '$(field_of 11 8)'"
# body: the person's words, then the region, a Refs line per issue
body_of 10 > "$T/b10"
grep -qxF 'Words a person wrote.' "$T/b10" || fail "pr 10 lost the text a person wrote"
grep -qxF -- '- Refs #7 (I0001 — Issue I0001)' "$T/b10" || fail "pr 10 has no Refs line for I0001: $(cat "$T/b10")"
grep -qxF -- '- Refs #8 (I0002 — Fixes # 3 in the parser)' "$T/b10" || fail "pr 10 has no Refs line for I0002: $(cat "$T/b10")"
grep -qE '^<!-- majordomus:pr-meta begin [0-9a-f]{16} -->$' "$T/b10" || fail "pr 10 region carries no hash"
grep -qF '**Milestone:** `M000` — Milestone M000' "$T/b10" || fail "pr 10 region names no milestone"
body_of 14 | grep -qxF -- '- I0003 (its record is in this pull request and not yet on the trunk)' \
  || fail "pr 14 does not name the record its head brings: $(body_of 14)"
# no closing keyword, anywhere the adapter wrote
for n in 10 11 12 13 14; do
  body_of "$n" | grep -iqE "$CLOSING" && fail "pr $n body carries a closing keyword: $(body_of "$n" | grep -ioE "$CLOSING")"
done
# a pull request that names no issue gets only its type label
[ "$(labels_of 13)" = 'type:fix' ] || fail "pr 13 labels are '$(labels_of 13)'"
[ -z "$(field_of 13 7)" ] || fail "pr 13 was given milestone '$(field_of 13 7)'"
body_of 13 | grep -q 'majordomus:pr-meta' && fail "pr 13 was given a region though it serves nothing"

# ---------------------------------------------------------------- 3. a second run changes nothing
before="$(writes)"
expect_exit 0 sync --apply
expect_grep 'github-sync prs apply: 5 pull request\(s\), 0 with drift, 0 drift finding'
[ "$(writes)" = "$before" ] || fail "the second apply wrote: $(grep -E '^(api -X |label create )' "$GH_LOG" | tail -n +"$((before + 1))")"
expect_exit 0 sync --check
expect_exit 0 sync --pr 11 --check
grep -q 'pulls/11 --jq' "$GH_LOG" || fail "--pr 11 did not read that one pull request"

# a listing shorter than the repository's own count is unreadable, never a set of absences
# (I3920's guard against the 487 duplicates of 2026-10-22), on this path as on the issues'
expect_exit 12 env FX_COUNT=99 PATH="$FORGE:$PATH" MJ_GH_PACE=0 "$SYNC" --prs --check
expect_grep 'could not list the issues of example/fixture'
expect_no_grep '^DRIFT'

# ---------------------------------------------------------------- 4. a person's edit of the region is kept
body_of 10 | sed 's/^- Refs #7 (I0001 — Issue I0001)$/- Refs #7, the main one/' > "$T/edited"
awk -F'\t' -v OFS='\t' -v b="$(base64 < "$T/edited" | tr -d '\n')" '$1 == 10 { $9 = b } { print }' "$FX_P" > "$T/p" && mv "$T/p" "$FX_P"
before="$(writes)"
expect_exit 11 sync --check
expect_grep 'DRIFT  edited    pr #10 has a hand-edited metadata region; not overwriting'
expect_exit 0 sync --apply
expect_grep 'DRIFT  edited    pr #10'
[ "$(writes)" = "$before" ] || fail "an apply wrote over a person's edit"
body_of 10 | grep -qxF -- '- Refs #7, the main one' || fail "the person's edit of pr 10 is gone"

# ---------------------------------------------------------------- 5. a closing keyword a person wrote is reported
awk -F'\t' -v OFS='\t' -v b="$(b64 'This closes #7.')" '$1 == 13 { $9 = b } { print }' "$FX_P" > "$T/p" && mv "$T/p" "$FX_P"
expect_exit 11 sync --check
expect_grep "WARN   closing   pr #13 body says 'closes #7'"

# ---------------------------------------------------------------- 6. the workflow is the model's adapter
# The repository's own files: the trigger, the token and the command are the model's. Case
# 1097 holds the same workflow to the landing rule (base checked out, issues never written).
WF="$ROOT/.github/workflows/pr-meta.yml"; MODEL="$ROOT/.ai/repo/ci/pr-meta.yaml"
types="$(sed -n 's/^    types: //p' "$MODEL")"
grep -qx '  pull_request_target:' "$WF" || fail "pr-meta.yml is not triggered by pull_request_target"
grep -qxF "    types: $types" "$WF" || fail "pr-meta.yml does not trigger on the model's types ($types)"
perms() { awk '/^permissions:$/ { on = 1; next } on && /^  [a-z-]+: [a-z]+$/ { print; next } on { exit }' "$1"; }
[ "$(perms "$WF")" = "$(perms "$MODEL")" ] || fail "pr-meta.yml asks for '$(perms "$WF" | tr '\n' ' ')', the model declares '$(perms "$MODEL" | tr '\n' ' ')'"
for k in pull_request sweep; do
  cmd="$(sed -n "s/^  $k: //p" "$MODEL" | sed 's/<number>/"$PR"/')"
  [ -n "$cmd" ] || fail "the model declares no $k command"
  grep -qxF "        run: $cmd" "$WF" || fail "pr-meta.yml does not run the model's $k command ($cmd)"
done
grep -qF "  schedule: '$(sed -n "s/^    - cron: '\(.*\)'$/\1/p" "$WF")'" "$MODEL" || fail "pr-meta.yml's schedule is not the model's"
grep -qF 'ref: ${{ github.event.repository.default_branch }}' "$WF" || fail "pr-meta.yml runs a code other than the trunk's"
grep -qF 'cancel-in-progress: false' "$WF" || fail "pr-meta.yml lets a newer run cancel one in flight"
# every job and step is bounded
awk '/^  [a-z-]+:$/ && injobs { job = $1 } /^jobs:/ { injobs = 1 }
     /^    timeout-minutes:/ { jt[job] = 1 } /^    runs-on:/ { jobs[job] = 1 }
     /^      - / { steps++; if (pending) bad = 1; pending = 1 } /^        timeout-minutes:/ { pending = 0 }
     END { if (pending) bad = 1; for (j in jobs) if (!(j in jt)) bad = 1; exit bad }' "$WF" \
  || fail "a job or step of pr-meta.yml has no timeout-minutes"
grep -iqE "$CLOSING" "$WF" "$MODEL" && fail "the workflow or its model carries a closing keyword"
exit 0
