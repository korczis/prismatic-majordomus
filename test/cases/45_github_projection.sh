# majordomus-covers: plan
# The GitHub projection, computed offline. No case here reaches the network: what is
# proved is that the payload the adapter would post is derived from the canonical model
# and from nothing else.
. "$ROOT/test/lib.sh"
SYNC="$ROOT/scripts/github-sync"
"$MJ" init >/dev/null
# init adds the local-state ignore line; commit it so the task that follows starts from a clean tree
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000 I0001

# --- the tool itself has no network client; the adapter is the only file that talks out
grep -rlE '(^|[^a-zA-Z_./-])gh[[:space:]]' "$ROOT/lib" "$ROOT/bin" 2>/dev/null && { echo "    the tool calls gh"; exit 1; }
grep -q 'gh api' "$SYNC" || { echo "    the adapter does not talk to GitHub at all"; exit 1; }

# --- the offline plan names every record, its GitHub state and its labels
expect_exit 0 "$SYNC" --plan
expect_grep 'repository: example/fixture'
expect_grep 'I0001 +open +READY'
expect_grep 'I0002 +open +BLOCKED'
expect_grep 'labels: majordomus,priority:p1'
expect_grep 'status:blocked'

# --- GitHub state is derived from canonical status, not from anything on GitHub
"$MJ" plan start I0001 >/dev/null
"$MJ" plan evidence I0001 --covers proof --type test --command "true" --result ok >/dev/null
"$MJ" plan "done" I0001 >/dev/null
expect_exit 0 "$SYNC" --plan
expect_grep 'I0001 +closed +DONE'
expect_grep 'I0002 +open +READY'

# --- the body is the same rendering the CLI prints, inside a generated region
expect_exit 0 "$SYNC" --render I0002
expect_grep '^<!-- majordomus:begin [0-9a-f]+ -->$'
expect_grep '^<!-- majordomus:record I0002 -->$'
expect_grep '^<!-- majordomus:end -->$'
expect_grep '^# I0002 — Issue I0002$'
"$SYNC" --render I0002 | sed '1d;$d' | grep -v '^<!-- majordomus:record ' > /tmp/rendered.$$
"$MJ" plan body I0002 > /tmp/cli.$$
diff -q /tmp/rendered.$$ /tmp/cli.$$ >/dev/null \
  || { echo "    the projection body and 'plan body' are two different renderings"; diff /tmp/rendered.$$ /tmp/cli.$$ | head; exit 1; }

# --- the region is composed in one place, and the marker is always a whole line
# The adapter needs the region's content twice: to print a body (--render) and to splice
# one into an issue that already exists. While it composed that content twice, the two
# compositions drifted — the splice wrote the marker with printf and no newline, so the
# first line of the record was glued to the marker in every body it posted. region_intact()
# drops the marker line before hashing, which took that first line with it, so the region
# never hashed back to its own marker: every issue written through the splice was reported
# `edited` for ever, and --force could not repair one because it rewrote it the same way.
# Two things are asserted because the defect needed both to be false at once: the body is
# rendered in one place, and no caller writes the marker without terminating its line.
composers="$(grep -cE '^ *(if )?mj_pj_is_milestone .*mj_plan_body_milestone' "$SYNC")"
[ "$composers" = 2 ] \
  || { echo "    the region body is composed in $composers places, not 2 (one to render, one to hash)"; \
       grep -nE '^ *(if )?mj_pj_is_milestone .*mj_plan_body_milestone' "$SYNC"; exit 1; }
# `|| true`, because the passing shape of this search is no output, and a case runs under
# set -e: without it a correct adapter killed the case silently and reported a bare FAIL.
unterminated="$(grep -n 'record_marker "' "$SYNC" | grep -vF "printf -- '%s\\n' \"\$(record_marker" || true)"
[ -z "$unterminated" ] \
  || { echo "    record_marker is written without the newline that ends its line:"; \
       printf '%s\n' "$unterminated"; exit 1; }

# --- the marker hash is the hash of the record it was generated from, so a canonical
#     change moves it and a GitHub-side edit does not
h1="$("$SYNC" --render I0002 | sed -n 's/^<!-- majordomus:begin \([0-9a-f]*\) -->$/\1/p')"
[ -n "$h1" ] || { echo "    no hash in the begin marker"; exit 1; }
h1b="$("$SYNC" --render I0002 | sed -n 's/^<!-- majordomus:begin \([0-9a-f]*\) -->$/\1/p')"
[ "$h1" = "$h1b" ] || { echo "    the projection is not deterministic"; exit 1; }
sed 's/^title: .*/title: A different title/' .ai/repo/project/issues/I0002.yaml > /tmp/i.$$ && mv /tmp/i.$$ .ai/repo/project/issues/I0002.yaml
h2="$("$SYNC" --render I0002 | sed -n 's/^<!-- majordomus:begin \([0-9a-f]*\) -->$/\1/p')"
[ "$h1" != "$h2" ] || { echo "    a canonical change did not move the projection hash"; exit 1; }

# --- a milestone projects its own body, with the DAG in it
expect_exit 0 "$SYNC" --render M000
expect_grep '## Issue DAG'
expect_grep 'flowchart LR'

# --- restricting to a milestone restricts the projection
pj_milestone M001 1
pj_issue I0100 M001
expect_exit 0 "$SYNC" --plan --milestone M001
expect_grep 'I0100'
expect_no_grep 'I0002'

# --- the adapter refuses what it does not understand, and says so
expect_exit 2 "$SYNC" --nonsense
expect_grep 'unknown option'
expect_exit 12 "$SYNC" --render I9999
expect_grep "no record 'I9999'"


# --- identity is what the record carries, not what its title says
#     A GitHub number is GitHub's to assign and a title is a person's to rewrite. Matching
#     on a title is how an adapter loses a renamed issue and then creates a second one for
#     the same canonical record, which is the one mistake a projection may not make.
FX_I=/tmp/fx_issues.$$; FX_M=/tmp/fx_ms.$$
printf '1\tM000 — Milestone M000\topen\n' > "$FX_M"

# one remote row, from a body on stdin: number, title, state, body, milestone, identity
row() {
  b="$(cat | base64 | tr -d '\n')"
  printf '%s\t%s\topen\t%s\tM000 — Milestone M000\t%s\n' "$1" "$2" "$b" "$3"
}
check_fx() { MJ_GH_FIXTURE_ISSUES="$FX_I" MJ_GH_FIXTURE_MILESTONES="$FX_M" "$SYNC" --check; }

# the region exactly as the adapter would post it: nothing has drifted
"$SYNC" --render I0002 | row 7 'Renamed by a person entirely' I0002 > "$FX_I"
expect_exit 11 check_fx
expect_grep 'OK +insync +issue I0002 \(#7\)'
expect_no_grep 'DRIFT +(missing|behind|edited|conflict) +issue I0002'

# the same region after the canonical record moves: behind, and safe to rewrite
"$SYNC" --render I0002 | row 7 'Renamed by a person entirely' I0002 > "$FX_I"
sed 's/^objective: .*/objective: "A different objective entirely."/' \
  .ai/repo/project/issues/I0002.yaml > /tmp/i2.$$ && mv /tmp/i2.$$ .ai/repo/project/issues/I0002.yaml
expect_exit 11 check_fx
expect_grep 'DRIFT +behind +issue I0002 \(#7\)'

# a person rewrote the region while the canonical record also moved. The marker still
# claims the hash it was written with, so the edit is visible even though the plan moved —
# the case the old comparison against the canonical hash could not see, and would have
# spliced straight over.
"$SYNC" --render I0002 | sed 's/^| status |.*/| status | somebody typed this |/' \
  | row 7 'Renamed by a person entirely' I0002 > "$FX_I"
sed 's/^objective: .*/objective: "Moved once more."/' \
  .ai/repo/project/issues/I0002.yaml > /tmp/i2.$$ && mv /tmp/i2.$$ .ai/repo/project/issues/I0002.yaml
expect_exit 11 check_fx
expect_grep 'DRIFT +conflict +issue I0002 \(#7\)'

# a person rewrote the region and the canonical record has not moved: edited, not conflict
"$SYNC" --render I0002 | sed 's/^| status |.*/| status | somebody typed this |/' \
  | row 7 'Renamed by a person entirely' I0002 > "$FX_I"
expect_exit 11 check_fx
expect_grep 'DRIFT +edited +issue I0002 \(#7\)'
expect_no_grep 'DRIFT +conflict'

# an issue projected before the marker existed is adopted by its title, once, and said so
"$SYNC" --render I0002 | grep -v '^<!-- majordomus:record ' \
  | row 8 'I0002 — Issue I0002' '' > "$FX_I"
expect_exit 11 check_fx
expect_grep 'DRIFT +adopt +issue I0002 \(#8\)'

# a title that merely looks canonical never captures a record carrying another identity
"$SYNC" --render I0100 | row 9 'I0002 — Issue I0002' I0100 > "$FX_I"
expect_exit 11 check_fx
expect_grep 'DRIFT +missing +issue I0002 is not on GitHub'

# a remote issue claiming a record this repository does not have is drift, and is counted:
# it used to be printed from inside a pipeline, where the count could never see it
"$SYNC" --render I0002 | sed 's/majordomus:record I0002/majordomus:record I9999/' \
  | row 10 'I9999 — Gone from the model' I9999 > "$FX_I"
expect_exit 11 check_fx
expect_grep 'DRIFT +unmanaged +issue #10 claims to be I9999'

# a human-written title is not an identity. An issue with no marker whose title does not
# carry the canonical `<id> — <title>` shape claims no record, so it is not unmanaged: the
# whole title used to be promoted to a canonical id, which made every hand-written issue on
# the remote a permanent finding of a gate that had no other one.
"$SYNC" --render I0002 | grep -v '^<!-- majordomus:record ' \
  | row 11 'P0: 100% new-code coverage is a blocking invariant' '' > "$FX_I"
expect_exit 11 check_fx
expect_no_grep 'DRIFT +unmanaged +issue #11'
expect_no_grep 'claims to be P0:'

# ...and a title that does carry the canonical shape still claims its record without a
# marker, so the two title branches stay symmetric and adoption by title is not lost
"$SYNC" --render I0002 | grep -v '^<!-- majordomus:record ' \
  | row 12 'I9999 — Gone from the model' '' > "$FX_I"
expect_exit 11 check_fx
expect_grep 'DRIFT +unmanaged +issue #12 claims to be I9999'

# --- a milestone is found by the identity it carries, not by its title
# The issue half stopped matching on titles when identity moved into the body. The milestone
# half kept the title as its only key, and a milestone carried no identity at all: the
# description named the record in prose — `Canonical: .ai/repo/project/milestones/<id>.yaml`
# — which nothing could read as a key. So renaming a milestone on GitHub orphaned its record
# and the next --apply created a second milestone for it.
#
# Both halves are asserted, because the fallback is what makes this easy to get wrong: a
# marker that is never read behaves exactly like the old code while looking fixed. The first
# fixture is a milestone whose title matches nothing and whose marker matches — it must be
# found. The second is the same rename with the marker gone — it must not be.
ms_row() { printf '%s\t%s\topen\t%s\n' "$1" "$2" "$3"; }
"$SYNC" --render I0002 | row 7 'I0002 — Issue I0002' I0002 > "$FX_I"

ms_row 1 'Somebody renamed this milestone entirely' M000 > "$FX_M"
expect_exit 11 check_fx
expect_no_grep 'DRIFT +missing +milestone M000'

ms_row 1 'Somebody renamed this milestone entirely' '' > "$FX_M"
expect_exit 11 check_fx
expect_grep 'DRIFT +missing +milestone M000 is not on GitHub'

# a milestone projected before the marker existed is still adopted by its title prefix
ms_row 1 'M000 — Milestone M000' '' > "$FX_M"
expect_exit 11 check_fx
expect_no_grep 'DRIFT +missing +milestone M000'

# ...and a title that merely looks canonical never captures a milestone carrying another
# identity, which is the milestone twin of the issue assertion above
ms_row 1 'M000 — Milestone M000' M001 > "$FX_M"
expect_exit 11 check_fx
expect_grep 'DRIFT +missing +milestone M000 is not on GitHub'

printf '1\tM000 — Milestone M000\topen\n' > "$FX_M"

# ...and the writer half, without which the reader above is a reader of nothing. Removing the
# marker from what --apply posts leaves every fixture assertion above passing, because a
# fixture is a remote this case writes by hand. There is no offline surface that prints a
# milestone description — `--render` prints the issue-style body — so this is asserted of the
# composition itself, as the two assertions near the top of this file are. It is worth saying
# what that does and does not prove: that the marker is composed into the description, not
# that GitHub received it.
sed -n '/^milestone_description()/,/^}/p' "$SYNC" > /tmp/msdesc.$$
[ -s /tmp/msdesc.$$ ] || { echo "    milestone_description() is gone; the description is composed somewhere unasserted"; exit 1; }
grep -q 'record_marker' /tmp/msdesc.$$ \
  || { echo "    the milestone description the adapter posts carries no record marker, so"; \
       echo "    every milestone it writes could only ever be found by its title again"; exit 1; }
rm -f /tmp/msdesc.$$

# ...and the half between them: the listing the reader reads. A fixture is a TSV this case
# writes, so it enters the adapter past the `gh api` that builds the real one — which means
# dropping the identity from the milestone listing's jq leaves every assertion above green
# and the live gate blind. Same shape as the two above, and the third time in this one file
# that a seam hid a side: the fixture proves the reader, the composition proves the writer,
# and this proves the listing they meet in.
grep -n 'milestones?state=all' -A 1 "$SYNC" | grep -q 'majordomus:record' \
  || { echo "    the milestone listing does not extract the record marker, so the identity"; \
       echo "    reaches the fixture and never the remote"; exit 1; }

# a fixture is a remote to read, never one to write
MJ_GH_FIXTURE_ISSUES="$FX_I" expect_exit 15 "$SYNC" --apply
expect_grep 'refuses a fixture remote'
rm -f "$FX_I" "$FX_M"

# --- nothing the adapter does writes to the canonical model
before="$(find .ai/repo/project -type f -exec shasum -a 256 {} \; | LC_ALL=C sort)"
"$SYNC" --plan >/dev/null; "$SYNC" --render M000 >/dev/null
after="$(find .ai/repo/project -type f -exec shasum -a 256 {} \; | LC_ALL=C sort)"
[ "$before" = "$after" ] || { echo "    the adapter wrote to the canonical model"; exit 1; }
rm -f /tmp/rendered.$$ /tmp/cli.$$
