# majordomus-covers: plan
# The GitHub projection, computed offline. No case here reaches the network: what is
# proved is that the payload the adapter would post is derived from the canonical model
# and from nothing else.
. "$ROOT/test/lib.sh"
SYNC="$ROOT/scripts/github-sync"
"$MJ" init >/dev/null
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
expect_grep '^<!-- majordomus:end -->$'
expect_grep '^# I0002 — Issue I0002$'
"$SYNC" --render I0002 | sed '1d;$d' > /tmp/rendered.$$
"$MJ" plan body I0002 > /tmp/cli.$$
diff -q /tmp/rendered.$$ /tmp/cli.$$ >/dev/null \
  || { echo "    the projection body and 'plan body' are two different renderings"; diff /tmp/rendered.$$ /tmp/cli.$$ | head; exit 1; }

# --- the marker hash is the hash of the record it was generated from, so a canonical
#     change moves it and a GitHub-side edit does not
h1="$("$SYNC" --render I0002 | sed -n 's/^<!-- majordomus:begin \([0-9a-f]*\) -->$/\1/p')"
[ -n "$h1" ] || { echo "    no hash in the begin marker"; exit 1; }
h1b="$("$SYNC" --render I0002 | sed -n 's/^<!-- majordomus:begin \([0-9a-f]*\) -->$/\1/p')"
[ "$h1" = "$h1b" ] || { echo "    the projection is not deterministic"; exit 1; }
sed 's/^title: .*/title: A different title/' .majordomus/project/issues/I0002.yaml > /tmp/i.$$ && mv /tmp/i.$$ .majordomus/project/issues/I0002.yaml
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

# --- nothing the adapter does writes to the canonical model
before="$(find .majordomus/project -type f -exec shasum -a 256 {} \; | sort)"
"$SYNC" --plan >/dev/null; "$SYNC" --render M000 >/dev/null
after="$(find .majordomus/project -type f -exec shasum -a 256 {} \; | sort)"
[ "$before" = "$after" ] || { echo "    the adapter wrote to the canonical model"; exit 1; }
rm -f /tmp/rendered.$$ /tmp/cli.$$
