# majordomus-covers: plan
# plan show says why an issue with no dependency waits, and reads a milestone's counts by name
# (I4266). An adopter's dependency-free issue in a milestone that depends on an unfinished one
# showed "status BLOCKED" beside "None. This issue is a root of the graph.", and every
# milestone body read its counts by position from a row whose format had moved on: "issues |
# done of total=15,required=15,...", "ready / blocked / active / verify | 1 / m0 / m0 / m3".
#
#   a BLOCKED root issue names its milestone and the milestones that one waits on;
#   a READY root issue is still a root of the graph;
#   a milestone body shows done of total, the four counts, and what it depends and waits on.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
pj_init
pj_milestone m0 0
pj_milestone m1 1
printf 'depends_on:\n  - m0\n' >> .ai/repo/project/milestones/m1.yaml
pj_issue I0001 m0
pj_issue I0002 m0
pj_issue I0010 m1
git add . && git commit -qm plan

expect_exit 0 "$MJ" plan show I0010
expect_grep 'None in the issue graph. It waits on its milestone m1, which waits on m0.'
expect_no_grep 'root of the graph'

expect_exit 0 "$MJ" plan show I0001
expect_grep 'None. This issue is a root of the graph.'

expect_exit 0 "$MJ" plan show m1
expect_grep '\| issues \| 0 done of 1 \|'
expect_grep '\| ready / blocked / active / verify \| 0 / 1 / 0 / 0 \|'
expect_grep '\| depends on \| m0 \|'
expect_grep '\| waits on \| m0 \|'
expect_exit 0 "$MJ" plan show m0
expect_grep '\| issues \| 0 done of 2 \|'
expect_grep '\| ready / blocked / active / verify \| 2 / 0 / 0 / 0 \|'
echo "ok: a waiting root names its milestone; milestone counts are read by name"
