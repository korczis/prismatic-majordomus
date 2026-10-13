# majordomus-covers: none
# claims: a-trunk-failure-is-answered-by-a-post-mortem
# A failed run of the trunk that no record names is owed, and a failure seen before is
# diagnosed from the records (rule project.a-ci-failure-has-a-post-mortem, ADR 0156).
#
# The forge is not asked here: `postmortem owed` and `postmortem draft` read the runs from a
# file in the shape the forge's own listing has, which is the same reading with the network
# taken out. What is held: a run a record names is answered; an unanswered one is owed and
# refuses only past its grace; a cancelled run and a run from before the rule owe nothing; a
# draft printed for an owed run is a record the tool accepts, and once it is in the tree the
# run is answered; and the text of a failure names the record whose signal it carries.
. "$ROOT/test/lib.sh"

RB="$(rust_bin)" || rust_bin_exit $?
PM() { env -u MAJORDOMUS_ROOT -u MAJORDOMUS_URL MAJORDOMUS_SHARE="$ROOT/share" "$RB" postmortem "$@"; }

"$MJ" init >/dev/null
D=.ai/repo/post-mortems
mkdir -p "$D" src
printf 'a component\n' > src/part.txt
cat > "$D/pm-20261013-the-part-fell-over.md" <<'REC'
---
schema: post-mortem/v1
id: pm-20261013-the-part-fell-over
kind: post-mortem
title: "The part fell over"
description: "Hand-made for the case."
status: analysed
date: 2026-10-13
runs:
  - workflow: validate
    id: "1001"
    commit: abcdef1
    jobs: ["structure"]
cause:
  class: defect
  summary: "The part had no shelf."
components: ["file:src/part.txt"]
signals:
  - id: line
    text: "the part fell over at shelf seven"
---

# The part fell over

## What failed

It fell.

## Root cause

It had no shelf.

## Fix

A shelf.

## Prevention

A case.
REC
git add -A "$D" src >/dev/null

run() {   # id conclusion created
  printf '{"databaseId":%s,"workflowName":"validate","conclusion":"%s","headSha":"abcdef1234567","createdAt":"%s","updatedAt":"%s","url":"https://forge.invalid/runs/%s","headBranch":"master"}' "$1" "$2" "$3" "$3" "$1"
}
RUNS="$(mktemp "${TMPDIR:-/tmp}/mj3000.runs.XXXXXX")"
trap 'rm -f "$RUNS"' EXIT
printf '[%s,%s,%s,%s]\n' \
  "$(run 1001 failure 2026-10-13T08:00:00Z)" \
  "$(run 1002 failure 2026-10-13T09:00:00Z)" \
  "$(run 1003 cancelled 2026-10-13T09:30:00Z)" \
  "$(run 900 failure 2026-10-12T09:00:00Z)" > "$RUNS"

# within the grace: owed, named, and not yet refused
expect_exit 0 PM owed --runs-json "$RUNS" --now 2026-10-13T10:00:00Z || exit 1
expect_grep '1002' || exit 1
expect_no_grep 'validate 1001 ' || { echo "    an answered run was listed as owed"; exit 1; }
expect_no_grep 'validate 1003 ' || { echo "    a cancelled run owes nothing"; exit 1; }
expect_no_grep 'validate 900 ' || { echo "    the rule reached back before 2026-10-13"; exit 1; }

# past the grace: refused, naming the run
expect_exit 10 PM owed --runs-json "$RUNS" --now 2026-10-15T10:00:00Z || exit 1
expect_grep '1002' || exit 1

# a shorter grace is the caller's to state, and it refuses sooner
expect_exit 10 PM owed --runs-json "$RUNS" --now 2026-10-13T12:00:00Z --grace-hours 1 || exit 1

# the draft for the owed run is a record: the tool accepts it, and it answers the run
PM draft --run 1002 --runs-json "$RUNS" --job structure > "$D/pm-20261013-validate-1002.md" 2>/dev/null \
  || { echo "    no draft was printed for run 1002"; exit 1; }
grep -q '^status: draft$' "$D/pm-20261013-validate-1002.md" || { echo "    the draft is not a draft"; exit 1; }
grep -q 'class: undetermined' "$D/pm-20261013-validate-1002.md" || { echo "    a draft claimed a cause"; exit 1; }
git add -A "$D" >/dev/null
expect_exit 0 PM check || exit 1
expect_exit 0 PM owed --runs-json "$RUNS" --now 2026-10-15T10:00:00Z || exit 1

# a run the file does not hold cannot be drafted, and the tool says so instead of inventing one
expect_exit 12 PM draft --run 7777 --runs-json "$RUNS" || exit 1

# a failure seen before is named by its signal, whatever the case of the log
expect_exit 0 PM diagnose --text 'run-plan: THE PART FELL OVER AT SHELF SEVEN, again' || exit 1
expect_grep 'pm-20261013-the-part-fell-over' || exit 1
expect_grep 'The part had no shelf' || exit 1

# and a failure nothing matches is not given a record it does not have
expect_exit 0 PM diagnose --text 'an entirely different failure' || exit 1
expect_no_grep 'pm-20261013-the-part-fell-over' || exit 1
