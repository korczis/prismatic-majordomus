# majordomus-covers: session
# claims: retention-caps, session-records
# The sessions section is capped by the policy like every other store, and the cap is
# answered by moving records aside when a person asks — never by deleting one, never in the
# background, and never over a record already archived. A record that names a checkpoint
# this checkout no longer has is reported with both names.
#
# What each part refuses:
#
#   a section nothing bounds. `retention` counted the ledger, the handovers and the
#   checkpoints and said nothing of the one store that is tracked and published.
#
#   a cap that deletes. The archive holds every byte the section held; the count of files
#   before and after is the same, and each archived record compares equal to what it was.
#
#   a half-done move. Every refusal — archiving switched off, a name already archived —
#   comes before the first record moves.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
echo a > a.txt
git add -A >/dev/null; git commit -qm base

close_one() {
  "$MJ" session start --worker "test/worker" >/dev/null
  printf 'Episode %s.\n' "$1" | "$MJ" session close >/dev/null
  sleep 1                      # the name opens with the closing second, and it is the order
}
close_one one; close_one two; close_one three
S=.ai/repo/sessions
records() { ls -1 "$S"/*.md | grep -v '/README\.md$' | LC_ALL=C sort; }
[ "$(records | wc -l | tr -d ' ')" = 3 ] || { echo "    expected three records, have: $(records)"; exit 1; }
oldest="$(records | sed -n 1p)"; second="$(records | sed -n 2p)"; newest="$(records | sed -n 3p)"
mkdir -p keep && cp "$oldest" "$second" "$newest" keep/

# ---------------------------------------------------------------- under the cap
"$MJ" doctor > out.txt 2>&1 || true
grep -Eq 'OK +retention +sessions — 3 records, cap 500' out.txt \
  || { echo "    doctor did not report the section against its cap"; grep retention out.txt; exit 1; }
expect_exit 0 "$MJ" session archive
expect_grep 'nothing to archive: 3 record\(s\), cap 500'
[ ! -e "$S/archive" ] || { echo "    an archive directory appeared under the cap"; exit 1; }

# ---------------------------------------------------------------- over it: doctor fails, watch drifts
sed -i.bak -e 's/^  retention_max_files: 500$/  retention_max_files: 1/' .ai/repo/policy.yaml
rm -f .ai/repo/policy.yaml.bak
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL retention +sessions — 3 records over cap 1'
expect_grep 'majordomus session archive --dry-run'
"$MJ" watch > out.txt 2>&1 && { echo "    watch found no drift over the cap"; exit 1; }
grep -Eq 'retention +sessions — 3 records over cap 1' out.txt \
  || { echo "    watch did not name the section"; cat out.txt; exit 1; }

# ---------------------------------------------------------------- a dry run moves nothing
expect_exit 0 "$MJ" session archive --dry-run
expect_grep 'would archive 2 of 3 record\(s\) to \.ai/repo/sessions/archive/, 1 kept'
expect_grep "$(basename "$oldest")"
expect_grep "$(basename "$second")"
expect_no_grep "$(basename "$newest")"
[ ! -e "$S/archive" ] && [ "$(records | wc -l | tr -d ' ')" = 3 ] \
  || { echo "    --dry-run moved something"; exit 1; }

# ---------------------------------------------------------------- switched off: refused, nothing moved
sed -i.bak -e 's/^  archive: true$/  archive: false/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
expect_exit 15 "$MJ" session archive
expect_grep "session.archive is 'false', not true; nothing moved"
[ ! -e "$S/archive" ] && [ "$(records | wc -l | tr -d ' ')" = 3 ] \
  || { echo "    archiving ran while the policy switched it off"; exit 1; }
sed -i.bak -e 's/^  archive: false$/  archive: true/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak

# ---------------------------------------------------------------- a name already archived: refused before the first move
mkdir -p "$S/archive"
printf 'already here\n' > "$S/archive/$(basename "$second")"
expect_exit 15 "$MJ" session archive
expect_grep "archive/$(basename "$second") already exists; nothing moved"
[ -f "$oldest" ] && [ -f "$second" ] \
  || { echo "    a record moved although the run was refused"; exit 1; }
[ "$(cat "$S/archive/$(basename "$second")")" = 'already here' ] \
  || { echo "    the record already archived was overwritten"; exit 1; }
[ ! -e "$S/archive/$(basename "$oldest")" ] \
  || { echo "    the oldest was archived before the refusal"; exit 1; }
rm -rf "$S/archive"

# ---------------------------------------------------------------- archived: moved, not deleted, not altered
expect_exit 0 "$MJ" session archive
expect_grep 'archived: 2 record\(s\) to \.ai/repo/sessions/archive/, 1 kept'
[ "$(records | wc -l | tr -d ' ')" = 1 ] && [ "$(records)" = "$newest" ] \
  || { echo "    the section does not hold exactly the newest record: $(records)"; exit 1; }
for f in "$oldest" "$second"; do
  cmp -s "keep/$(basename "$f")" "$S/archive/$(basename "$f")" \
    || { echo "    $(basename "$f") is not in the archive byte for byte"; exit 1; }
done
cmp -s "keep/$(basename "$newest")" "$newest" || { echo "    the record kept was altered"; exit 1; }
[ "$(ls -1 "$S/archive" | wc -l | tr -d ' ')" = 2 ] || { echo "    the archive holds something else"; ls "$S/archive"; exit 1; }
grep -q '"event":"session.archived"' .ai/local/state/ledger.jsonl \
  || { echo "    the move left no ledger line"; exit 1; }
grep '"event":"session.archived"' .ai/local/state/ledger.jsonl | grep -q '"archived":2,"kept":1,"archive":".ai/repo/sessions/archive"' \
  || { echo "    the ledger line does not say what moved where"; exit 1; }
expect_exit 0 "$MJ" history --validate

# the cap is met, and the readers of the section see what the section holds
"$MJ" doctor > out.txt 2>&1 || true
grep -Eq 'OK +retention +sessions — 1 records, cap 1' out.txt \
  || { echo "    doctor still counts what was archived"; grep retention out.txt; exit 1; }
"$MJ" session list > out.txt
grep -q "$(sed -n 's/^session_id: //p' "$newest")" out.txt || { echo "    list lost the record kept"; exit 1; }
grep -q "$(sed -n 's/^session_id: //p' "keep/$(basename "$oldest")")" out.txt \
  && { echo "    list still shows an archived record"; exit 1; }
expect_exit 0 "$MJ" session archive
expect_grep 'nothing to archive: 1 record\(s\), cap 1'

# ---------------------------------------------------------------- the refusals of the command itself
expect_exit 2 "$MJ" session archive --force
sed -i.bak -e '/^  retention_max_files: 1$/d' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
expect_exit 10 "$MJ" session archive
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL retention +sessions — policy declares no session.retention_max_files'
reset_policy

# ---------------------------------------------------------------- a reference to a record that is gone
# The record is this working copy's own, so what it names can be looked for here.
gone=.ai/local/state/checkpoints/20260101T000000Z--gone.md
sed -i.bak -e "s|^checkpoints: \[\]\$|checkpoints:\n  - \"$gone\"|" "$newest"; rm -f "$newest.bak"
grep -q "^  - \"$gone\"\$" "$newest" || { echo "    the fixture did not plant the reference"; sed -n '1,30p' "$newest"; exit 1; }
"$MJ" doctor > out.txt 2>&1 || true
grep -E 'WARN +session' out.txt | grep -F "$(basename "$newest")" | grep -qF "names 1 record(s) this checkout no longer has: $gone" \
  || { echo "    doctor did not name the record and what it names"; grep -i session out.txt; exit 1; }
"$MJ" watch > out.txt 2>&1 || true
grep -F "$(basename "$newest")" out.txt | grep -qF "$gone" \
  || { echo "    watch did not report the dangling reference"; grep -i session out.txt; exit 1; }

# there, it is not reported
mkdir -p "$(dirname "$gone")"; printf 'x\n' > "$gone"
"$MJ" doctor > out.txt 2>&1 || true
grep -qF 'this checkout no longer has' out.txt && { echo "    a reference that resolves was reported"; exit 1; }
rm -f "$gone"

# another working copy's record names paths that were never here: nothing to report
sed -i.bak -e 's/^worktree_id: .*$/worktree_id: 0000000000000000/' "$newest"; rm -f "$newest.bak"
"$MJ" doctor > out.txt 2>&1 || true
grep -qF 'this checkout no longer has' out.txt \
  && { echo "    another working copy's record was judged against this disk"; exit 1; }
exit 0
