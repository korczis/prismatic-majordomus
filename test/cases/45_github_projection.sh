# majordomus-covers: plan
# claims: github-projection
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
expect_no_grep 'DRIFT +(never-created|deleted-remotely|behind|edited|conflict) +issue I0002'

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
expect_grep 'DRIFT +never-created +issue I0002 is not on GitHub'

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

# a fixture is a remote to read, never one to write
MJ_GH_FIXTURE_ISSUES="$FX_I" expect_exit 15 "$SYNC" --apply
expect_grep 'refuses a fixture remote'

# --- a record GitHub does not serve is one of two absences, and they never read as one (I4290)
#     GitHub says when it created each projected record; the newest of those is the last apply
#     that created anything. A record that reached the trunk before it was created by it and
#     then deleted on GitHub; one that reached the trunk later was never created. The records
#     land now, so a projection dated 2999 came after them and one dated 2000 before.
# MUTATION, RUN: make absence_of print never-created always -> the deleted-remotely lines
# below are missing; make it print deleted-remotely always -> the never-created ones are.
git add -A >/dev/null; git commit -qm "the records land on the trunk" >/dev/null
created() { awk -v c="$1" 'BEGIN { FS = OFS = "\t" } { print $0, c }'; }
remote_at() {
  { "$SYNC" --render I0001 | row 21 'I0001 — Issue I0001' I0001
    "$SYNC" --render I0100 | row 22 'I0100 — Issue I0100' I0100; } | created "$1" > "$FX_I"
  printf '1\tM000 — Milestone M000\topen\t%s\n2\tM001 — Milestone M001\topen\t%s\n' "$1" "$1" > "$FX_M"
}
remote_at 2999-01-01T00:00:00Z
expect_exit 11 check_fx
expect_grep 'DRIFT +deleted-remotely +issue I0002 was on GitHub \(on the trunk since [0-9T:-]+Z, before the projection of 2999-01-01T00:00:00Z\)'
expect_grep 'MJ_GH_RECREATE=I0002'
expect_no_grep 'never-created +issue I0002'
remote_at 2000-01-01T00:00:00Z
expect_exit 11 check_fx
expect_grep 'DRIFT +never-created +issue I0002 is not on GitHub; --apply creates it'
expect_no_grep 'deleted-remotely'
# a projected milestone dates the projection as an issue does
printf '1\tM000 — Milestone M000\topen\t2999-01-01T00:00:00Z\n' > "$FX_M"; : > "$FX_I"
expect_exit 11 check_fx
expect_grep 'DRIFT +deleted-remotely +milestone M001'
expect_grep 'DRIFT +deleted-remotely +issue I0002'
# a remote that carries no projected record yet (an adopter's first apply) dates nothing,
# and neither does a row written before the remote said when it was created
: > "$FX_M"
expect_exit 11 check_fx
expect_grep 'DRIFT +never-created +milestone M001'
expect_grep 'DRIFT +never-created +issue I0002'
expect_no_grep 'deleted-remotely'
printf '1\tM000 — Milestone M000\topen\n2\tM001 — Milestone M001\topen\n' > "$FX_M"
"$SYNC" --render I0001 | row 21 'I0001 — Issue I0001' I0001 > "$FX_I"
expect_exit 11 check_fx
expect_grep 'DRIFT +never-created +issue I0002'
expect_no_grep 'deleted-remotely'

# --- --apply refuses a deleted-remotely record, names it and writes nothing for it; a
#     person names it in MJ_GH_RECREATE and the next apply creates it. A fixture is never
#     applied, so `gh` is a stub on PATH that answers the listings and records every call,
#     and the trunk is a local bare remote, as in case 602.
STUB="$T/stub"; mkdir -p "$STUB"; CALLS="$T/gh.calls"
cat > "$STUB/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$MJ_STUB_CALLS"
case "$1 $2" in "auth status"|"label list") exit 0 ;; esac
case "$*" in
  *"--paginate repos/"*"/milestones"*) cat "$MJ_STUB_MS"; exit 0 ;;
  *"issue list"*)                      cat "$MJ_STUB_IS"; exit 0 ;;
  *"api graphql"*)                     wc -l < "$MJ_STUB_IS" | tr -d ' '; exit 0 ;;
  *"issue create"*)                    echo "https://github.com/example/fixture/issues/99"; exit 0 ;;
esac
exit 0
STUB
chmod +x "$STUB/gh"
printf '/stub/\n/remote.git/\n/gh.calls\n' >> .git/info/exclude
git init -q --bare "$T/remote.git"
git -C "$T/remote.git" fetch -q --no-prune "$T" HEAD:refs/heads/trunk
git -C "$T/remote.git" symbolic-ref HEAD refs/heads/trunk
git remote add origin "$T/remote.git"; git fetch -q origin; git remote set-head origin trunk >/dev/null
remote_at 2999-01-01T00:00:00Z
apply() {
  : > "$CALLS"
  PATH="$STUB:$PATH" MJ_GH_PACE=0 MJ_STUB_MS="$FX_M" MJ_STUB_IS="$FX_I" MJ_STUB_CALLS="$CALLS" \
    "$SYNC" --apply
}
expect_exit 15 apply
expect_grep 'REFUSED +created +issue I0002 — deleted on GitHub; nothing was written'
expect_grep 'name it in MJ_GH_RECREATE'
if grep -q 'issue create' "$CALLS"; then
  echo "    --apply created an issue GitHub had deleted:"; grep 'issue create' "$CALLS"; exit 1
fi
MJ_GH_RECREATE=I0002 expect_exit 0 apply
expect_grep 'OK +created +issue I0002'
grep -q 'issue create .*I0002 — ' "$CALLS" || { echo "    MJ_GH_RECREATE=I0002 did not create I0002"; cat "$CALLS"; exit 1; }
rm -f "$FX_I" "$FX_M"

# --- nothing the adapter does writes to the canonical model
before="$(find .ai/repo/project -type f -print0 | mj_sha256_xargs -0 | LC_ALL=C sort)"
"$SYNC" --plan >/dev/null; "$SYNC" --render M000 >/dev/null
after="$(find .ai/repo/project -type f -print0 | mj_sha256_xargs -0 | LC_ALL=C sort)"
[ "$before" = "$after" ] || { echo "    the adapter wrote to the canonical model"; exit 1; }
rm -f /tmp/rendered.$$ /tmp/cli.$$
