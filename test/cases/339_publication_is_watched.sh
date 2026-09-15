# A deploy that did not happen is noticed within the window it is judged by, and says so.
#
# scripts/ci/pages-check decides whether the published site is still a projection of master;
# cases 104 and 180 hold both of its halves to fixtures. Neither asks *when* it runs, and that
# is what failed. The gate ran in `validate`: on push, when the deploy window has just opened
# and the honest answer is structurally "inside the window", and then once a day. Publication
# stopped on 2026-09-14 at 18:34 and the site served a commit two merges behind, with every run
# green, until a person ran the gate by hand.
#
# The schedule that fixes it lives in pages.yml and its model (test/cases/97 holds those two to
# each other, including the history the publishing job checks out). This case holds the half
# that rots quietly: that the scheduled path asks the gate, and that an owed publication is
# both repaired and reported rather than repaired in silence.
. "$ROOT/test/lib.sh"

P="$ROOT/.github/workflows/pages.yml"
expect_file "$P"
expect_file "$ROOT/scripts/ci/pages-check"

# 1. it is asked on a clock, at least hourly, and no less often than the window it judges by:
#    a cadence slower than the window leaves an owed publication unreported for longer than the
#    window it is judged by.
cron="$(grep -oE "cron: *['\"][^'\"]+['\"]" "$P" | head -1 | sed "s/.*['\"]\(.*\)['\"]/\1/")"
[ -n "$cron" ] || { echo "    pages.yml declares no schedule: nothing asks between pushes"; exit 1; }
minutes="$(printf '%s' "$cron" | awk '{ if ($1 ~ /^\*\/[0-9]+$/) { sub(/^\*\//, "", $1); print $1 } else if ($2 == "*") print 60; else print 1440 }')"
[ "$minutes" -le 60 ] || { echo "    the schedule fires every ${minutes}m; a publication may be owed that long unseen"; exit 1; }
window="$(sed -n 's/^OWED_AFTER="\${PAGES_CHECK_OWED_AFTER:-\([0-9]*\)}".*/\1/p' "$ROOT/scripts/ci/pages-check" | head -1)"
[ -z "$window" ] || [ $(( minutes * 60 )) -le "$window" ] || {
  echo "    the schedule fires every $(( minutes * 60 ))s but the gate judges by a ${window}s window"; exit 1; }

# 2. the scheduled path asks the gate rather than deploying blind, in a step that says so
grep -qE "schedule" "$P" || { echo "    no schedule trigger in pages.yml"; exit 1; }
expect_grep 'id: publication-owed' "$P"
awk '/id: *publication-owed/ { s = 1 } s && /scripts\/ci\/pages-check/ { found = 1 } s && /^      - / && !/id: *publication-owed/ { s = 0 } END { exit !found }' "$P" \
  || { echo "    the publication-owed step does not run scripts/ci/pages-check"; exit 1; }
grep -qE "steps\.publication-owed\.outputs\.owed" "$P" \
  || { echo "    nothing is conditioned on whether a publication is owed: a scheduled run would deploy blind"; exit 1; }

# 3. an owed publication is reported, not only repaired. A scheduled run that deploys and then
#    reports success hides a publisher that breaks again every cycle: it heals the symptom on
#    each pass and no run is ever red. The step that reports is named, so this asserts the
#    intervention rather than any failure anywhere in the file.
expect_grep 'id: report-intervention' "$P"
# the step's own block: from its id to the next step at the same indent
awk '/^ *- *name:|^ *- *id:/ { if (inblock) exit } /id: *report-intervention/ { inblock = 1 } inblock { print }' "$P" > "$T/report-step"
[ -s "$T/report-step" ] || { echo "    the report-intervention step could not be read"; exit 1; }
grep -qE "owed *== *'?true'?" "$T/report-step" || { echo "    the reporting step is not conditioned on a publication being owed"; exit 1; }
grep -qE '(^|[^a-z])exit 1' "$T/report-step" || { echo "    the reporting step does not end non-zero: an owed publication would be repaired silently"; exit 1; }
# the verdict is not swallowed — asserted inside the two steps that decide it, not by proximity.
# pages.yml legitimately carries continue-on-error on the post-publish verification step, which
# is a report about a deployment that already happened; what must not be softened is the gate
# that decides whether one is owed, and the step that reports having intervened.
for want in publication-owed report-intervention; do
  awk -v id="$want" '
    $0 ~ ("id: *" id) { inblock = 1; print; next }
    inblock && /^ *- *(name|id):/ { exit }
    inblock { print }
  ' "$P" > "$T/step-$want"
  [ -s "$T/step-$want" ] || { echo "    the $want step could not be read"; exit 1; }
  grep -qE '\|\| *true|continue-on-error: *true' "$T/step-$want" \
    && { echo "    the $want step softens its own verdict (|| true / continue-on-error)"; exit 1; }
done

# The checkout depth the publishing job needs is not asserted here. It is the same question
# as "does the workflow check out what the model declares", and test/cases/97 answers it
# against `scripts/pages checkout` rather than against a literal 0 — so it also fails when
# the model moves and the workflow does not, which a literal here could never see. One
# subject, one gate.

echo "    the publication is asked for every ${minutes}m and an owed one is reported and fails the run"
