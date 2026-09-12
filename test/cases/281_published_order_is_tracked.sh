# majordomus-covers: session
# majordomus-negative: session
# The published order of the session records is a fact about the tracked tree, and about
# nothing else.
#
# `session list` decides site/data/generated/sessions.json and the `weight` of every page
# under site/content/sessions/. Both are committed, and `scripts/generate-site-data --check`
# compares them byte for byte — so every input to that order has to survive a clone.
#
# It did not. created_at has second resolution, and the tie inside one second was broken by
# mj_record_rank: the line number of the record's file name inside
# .ai/local/state/ledger.jsonl, a file that is gitignored, machine-local and rotated under a
# retention cap. This repository holds six records sharing a created_at second, so their
# committed order was decided by a number no clone can reproduce.
#
# CI was green by luck, which is why nothing caught it: a runner's ledger names none of
# those records, every rank came back 000000, the tie fell through to the path — and that is
# the order which happens to be committed. The first developer to commit generated output
# from a machine whose ledger did name them would have moved the records and turned the
# check red for everybody else.
#
# So the property is not "the order is stable here". It is that one repository read on two
# machines gives one answer: the order with an empty ledger and the order with a ledger that
# ranks every record in the opposite direction must be the same list.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base

# ---------------------------------------------------------------- three records, one second
# Synthetic rather than copied out of this checkout: the case must keep measuring this after
# the six records that exposed it have rotated out of the section.
# The section the manifest names, not a directory of this case's choosing: MJ_SESSIONS_DIR
# is computed from .ai/manifest.yaml on every run, so exporting it does nothing.
STORE=".ai/repo/sessions"; mkdir -p "$STORE"
COLLIDE=2026-09-11T07:27:23Z
# Named so that the order of the names and the order they are written to the ledger in
# disagree. A test whose two orders happen to coincide proves nothing.
A="20260911T072724Z--s-20260910211505-aaaa--master--06fa258--1111111111111111.md"
B="20260911T072724Z--s-20260910211853-bbbb--master--06fa258--2222222222222222.md"
C="20260911T072724Z--s-20260910212104-cccc--master--06fa258--3333333333333333.md"
rec() {                                   # rec FILE SESSION_ID
  cat > "$STORE/$1" <<Y
---
schema: session/v1
kind: session
created_at: $COLLIDE
repository_id: git@example.com:fixture/three.git
worktree_id: 0000000000000000
branch: master
head: 06fa25891550d6118f3446d0b33ff211388ce779
working_tree: clean
session_id: $2
started_at: 2026-09-10T21:15:05Z
closed_at: $COLLIDE
outcome: completed
title: "Session $2"
---
Y
}
rec "$A" s-20260910211505-aaaa
rec "$B" s-20260910211853-bbbb
rec "$C" s-20260910212104-cccc

ids() { jq -r '.sessions[].session_id' "$1"; }
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }

# ---------------------------------------------------------------- a clone that knows nothing
run_quiet "$T/clean.err" "$MJ" --json session list --all > "$T/clean.json"
ids "$T/clean.json" > "$T/clean.ids"
[ "$(wc -l < "$T/clean.ids" | tr -d ' ')" = 3 ] || {
  echo "    expected the three fixture records, got:"; sed 's/^/    | /' "$T/clean.ids"; exit 1; }
# Newest first, and within one second by the record's own name, descending. Asserted
# literally so that a future change of tie-break has to say so here rather than pass
# whatever it produces.
printf 's-20260910212104-cccc\ns-20260910211853-bbbb\ns-20260910211505-aaaa\n' > "$T/want.ids"
diff -u "$T/want.ids" "$T/clean.ids" || {
  echo "    the tie inside one second is not broken by the record's name, descending"; exit 1; }

# ---------------------------------------------------------------- a machine that wrote them
# What lib/session.sh:517 writes when it closes an episode: session_path names the record,
# so a machine that closed these three has all three file names in its ledger. Written in
# the reverse of the name order, which is what makes the two orders disagree — under the old
# tie-break the last-ranked record sorted first, and this ledger reversed the list.
mkdir -p .ai/local/state
for name in "$C" "$B" "$A"; do
  printf '{"ts":"%s","event":"session.closed","head":"06fa258","branch":"master","by":"majordomus/0.6.0","outcome":"completed","session_path":".ai/repo/sessions/%s"}\n' \
    "$COLLIDE" "$name" >> .ai/local/state/ledger.jsonl
done
# the ledger really does name them: a rank of 000000 for all three would make the
# comparison below vacuous, which is exactly how CI stayed green on the defect
for name in "$A" "$B" "$C"; do
  grep -Fq "$name" .ai/local/state/ledger.jsonl || {
    echo "    the fixture ledger does not name $name, so it ranks nothing"; exit 1; }
done

run_quiet "$T/ranked.err" "$MJ" --json session list --all > "$T/ranked.json"
ids "$T/ranked.json" > "$T/ranked.ids"
diff -u "$T/clean.ids" "$T/ranked.ids" || {
  echo "    the published order changed when the machine-local ledger did"
  echo "    this order is committed as site/data/generated/sessions.json and as the weight of"
  echo "    every page under site/content/sessions/, so a machine-local input makes it"
  echo "    unreproducible: whoever commits generated output next moves the records for"
  echo "    everyone, and core-check goes red on a tree nobody changed"
  exit 1; }

# ---------------------------------------------------------------- and it cannot come back
# The behavioural check above only fires when the store holds a created_at collision. This
# one holds regardless: the function that produces the published order must not read the
# ledger at all. mj_record_rank stays — mj_resolve_latest legitimately asks "which of these
# did this machine write last", a question whose answer is never committed.
awk '/^mj_session_keys\(\) \{/{f=1} f{print} f&&/^}/{exit}' "$ROOT/lib/session.sh" > "$T/keys.sh"
[ -s "$T/keys.sh" ] || { echo "    could not find mj_session_keys in lib/session.sh"; exit 1; }
if grep -qE 'mj_record_rank|MJ_STATE_DIR|ledger' "$T/keys.sh"; then
  echo "    mj_session_keys reads the machine-local ledger again — the published order is back"
  echo "    to being decided by a line number in a gitignored, rotated file. mj_record_rank"
  echo "    itself is fine where it is: mj_resolve_latest asks which record this machine wrote"
  echo "    last, and that answer is never committed."
  exit 1
fi

printf '    one repository, two ledgers, one published order\n'
