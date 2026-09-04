# Closed sessions are read back with the divergence vocabulary that already exists, and
# ordered by what the record asserts rather than by the filesystem.
#
# The failure this prevents is specific: a session written before a branch was rewritten,
# presented to the next worker as if it still described this history. Nothing here invents a
# second word for stale — `exact`, `advanced`, `diverged` and `different_context` mean the
# same thing for a session as they do for a handover.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -q -m install

close_one() {   # close_one <summary>  -> prints the record path
  "$MJ" session start --owner tester >/dev/null
  printf '%s\n' "$1" | "$MJ" session close
}

# --- absence is an answer on every one of the three views
expect_exit 0 "$MJ" session list
expect_grep 'No closed sessions'
expect_exit 0 "$MJ" session latest
expect_grep 'No closed session'
expect_exit 12 "$MJ" session show s-nosuch-0000
expect_grep 'no closed session'

# --- a record written at this commit reads back as exact
r1="$(close_one 'first episode')"
sid1="$(sed -n 's/^session_id: //p' "$r1")"
expect_exit 0 "$MJ" session latest
expect_grep "Session: +$sid1"
expect_grep 'Git state: exact'

# --- git moving forward is `advanced`, which is trustworthy and says so
printf 'one\n' > f1.txt && git add f1.txt && git commit -q -m one
expect_exit 0 "$MJ" session show "$sid1"
expect_grep 'Git state: advanced'
expect_no_grep 'WARNING'

# --- ordering comes from the recorded timestamp, not from the filesystem. Touching an old
#     record must not make it the newest, and mtime does not survive a clone anyway.
r2="$(close_one 'second episode')"
sid2="$(sed -n 's/^session_id: //p' "$r2")"
expect_exit 0 "$MJ" session list
first_listed="$(printf '%s\n' "$LAST_OUT" | head -1 | awk '{ print $2 }')"
[ "$first_listed" = "$sid2" ] || { echo "    list is not newest-first: got $first_listed"; exit 1; }
touch "$r1"
expect_exit 0 "$MJ" session list
first_listed="$(printf '%s\n' "$LAST_OUT" | head -1 | awk '{ print $2 }')"
[ "$first_listed" = "$sid2" ] || { echo "    touching an old record reordered the list"; exit 1; }

# --- two records inside one second still order deterministically, because the ledger is
#     append-only and written in the order the commands ran. created_at has one-second
#     resolution, so without that tiebreak the order would come from a random filename.
#
#     The collision is forced rather than hoped for. Two closes a fraction of a second apart
#     land in the same second only sometimes, and a case that exercises its own hardest path
#     only sometimes is a case that reports green when that path is broken.
r3="$(close_one 'third')"; r4="$(close_one 'fourth')"
sid3="$(sed -n 's/^session_id: //p' "$r3")"; sid4="$(sed -n 's/^session_id: //p' "$r4")"
collide=2026-09-04T12:00:00Z
for f in "$r3" "$r4"; do
  sed -e "s/^created_at: .*/created_at: $collide/" -e "s/^closed_at: .*/closed_at: $collide/" "$f" > "$f.tmp"
  mv "$f.tmp" "$f"
done
[ "$(sed -n 's/^created_at: //p' "$r3")" = "$(sed -n 's/^created_at: //p' "$r4")" ] \
  || { echo "    the timestamp collision was not applied"; exit 1; }

# The tiebreak must be the LEDGER and not the filename, and the two normally agree — the
# filename leads with the same timestamp, so an implementation that fell through to it would
# look right. So make them disagree: swap the two close events in the ledger, which makes r3
# the later one there while r4 is still the later one by filename. Only an implementation
# that reads the ledger puts r3 first.
LED=.ai/local/state/ledger.jsonl
b3="$(basename "$r3")"; b4="$(basename "$r4")"
grep -q "$b3" "$LED" && grep -q "$b4" "$LED" \
  || { echo "    the ledger does not name the records whose order it decides"; exit 1; }
awk -v b3="$b3" -v b4="$b4" '
  index($0, b3) { l3 = $0; next }
  index($0, b4) { print; print l3; next }
  { print }' "$LED" > "$LED.tmp" && mv "$LED.tmp" "$LED"
tail -2 "$LED" | head -1 | grep -q "$b4" || { echo "    the ledger swap did not take"; tail -3 "$LED"; exit 1; }

"$MJ" session list > list1.txt
"$MJ" session list > list2.txt
cmp -s list1.txt list2.txt || { echo "    two listings of the same store disagreed"; exit 1; }
o3="$(grep -n "$sid3" list1.txt | cut -d: -f1)"; o4="$(grep -n "$sid4" list1.txt | cut -d: -f1)"
[ -n "$o3" ] && [ -n "$o4" ] || { echo "    a record is missing from the listing"; exit 1; }
[ "$o3" -lt "$o4" ] || {
  echo "    inside one second the order came from the filename, not from the ledger"
  cat list1.txt; exit 1; }

# --- a rewritten history makes every view say `diverged`, and say it loudly
base="$(git rev-parse HEAD~1)"
git reset -q --hard "$base"
printf 'other\n' > f2.txt && git add f2.txt && git commit -q -m other
expect_exit 0 "$MJ" session show "$sid4"
expect_grep 'Git state: diverged'
expect_grep 'WARNING'
expect_exit 0 "$MJ" session list
expect_grep 'diverged'
expect_exit 0 "$MJ" session latest
expect_grep 'diverged'

# --- a session from another branch is never offered as this branch's latest
git checkout -q -b sidebranch
expect_exit 0 "$MJ" session latest
expect_grep 'No closed session'
expect_exit 0 "$MJ" session list
expect_grep 'No closed sessions'
# --all lifts the rule and says which branch each record is from, because a record from
# elsewhere is worth seeing when you asked for everything and never worth being handed
# silently
expect_exit 0 "$MJ" session list --all
expect_grep "$sid4"
expect_grep 'different_context'
git checkout -q -

# --- a malformed record is skipped with a warning, never silently, and never fatally
bad="$(dirname "$r1")/20260904T000000Z--s-broken-0000--x--0000000--ffffffffffffffff.md"
printf 'not a record at all\n' > "$bad"
expect_exit 0 "$MJ" session list
expect_grep 'skipped .*malformed record'
expect_grep "$sid4"
rm -f "$bad"

# --- every one of these is read-only
rm -f list1.txt list2.txt
before_status="$(git status --porcelain)"
"$MJ" session list >/dev/null 2>&1
"$MJ" session list --all >/dev/null 2>&1
"$MJ" session latest >/dev/null 2>&1
"$MJ" session latest --path >/dev/null 2>&1
"$MJ" session show "$sid4" >/dev/null 2>&1
"$MJ" --json session list >/dev/null 2>&1
"$MJ" --json session latest >/dev/null 2>&1
[ "$(git status --porcelain)" = "$before_status" ] \
  || { echo "    a read-only session command changed the working tree"; git status --porcelain; exit 1; }

# --- JSON carries the label, so a caller can refuse a diverged record without re-deriving it
expect_exit 0 "$MJ" --json session show "$sid4"
expect_grep '"label":"diverged"'
expect_grep '"session_id":"'"$sid4"'"'
