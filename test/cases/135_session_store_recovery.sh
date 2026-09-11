# majordomus-covers: recover
# majordomus-negative: recover
# Recovery of the record stores: what it closes, what it refuses to close, and what it will
# not delete.
#
# Three guarantees this case exists for, each of which has already failed somewhere:
#
#   A live episode is never closed. The whole command is a sweep that selects by age, and
#   the rule this repository wrote after one such sweep cleaned three *active* worktrees
#   (project.destructive-sweeps-fail-closed) says a predicate whose input is missing must be
#   false, never the most extreme value in the set. So a live episode is opened here, an
#   unreadable one is planted beside it, and the run is asserted to have left both.
#
#   Recovery is exactly once and survives being interrupted. A provider's end event fires
#   more than once — that is what produced four records of one episode in this repository —
#   so every subject is run twice and the second run is asserted to take no action.
#
#   Nothing is deleted for being unrecognised. A temp file holding the only copy of a record
#   is published, not removed; one holding content this version cannot classify is left
#   exactly where it is and counted.
. "$ROOT/test/lib.sh"

# A bare `[ ... ]` under the runner's `set -e` ends a case having printed nothing anywhere:
# no message, no line number, a bare FAIL. It happened twice while this case was being
# written. `must` is the same test with the reason attached, and every assertion below that
# is not an expect_* uses it.
must() { local why="$1"; shift; "$@" || { printf '    %s (failed: %s)\n' "$why" "$*"; return 1; }; }
# Session records only. The store also holds its own README.md, which is a context document
# and not a record, and counting it is how the first draft of this case asserted 0 and got 1.
n_records() { grep -l '^session_id: ' .ai/repo/sessions/*.md 2>/dev/null | wc -l | tr -d ' '; }
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add . && git commit -qm base
S=.ai/local/state

# ---------------------------------------------------------------- surface
expect_exit 0 "$MJ" recover --help
expect_grep 'usage: majordomus recover'
expect_exit 2 "$MJ" recover nonsense
expect_grep 'unknown subject'
expect_exit 2 "$MJ" recover episodes --nonsense
expect_grep 'unknown option'
expect_exit 2 "$MJ" recover episodes --older-than nonsense
expect_grep 'is not a duration'
# a duration that needs a value and has none is a usage error, not a silent default
expect_exit 2 "$MJ" recover episodes --older-than
expect_grep 'needs a value'

# a clean store is an answer, not a failure: `empty is not failure` applies here too
expect_exit 0 "$MJ" recover
expect_grep 'nothing to recover'

# ---------------------------------------------------------------- the fixture
# Two episodes, one of which will be made old, and one this process is inside.
"$MJ" session start --provider-session gone >/dev/null
DEAD="$(sed -n 's/^session_id: //p' "$S/sessions-open/gone.yaml")"
"$MJ" session start --provider-session here >/dev/null
LIVE="$(sed -n 's/^session_id: //p' "$S/sessions-open/here.yaml")"

# Backdating the whole trail, not only the open record: the last sign of life is the later
# of started_at and the newest ledger line the episode stamped, so an episode whose ledger
# is fresh is young however old its file says it is. The test would pass for the wrong
# reason if only one half were moved.
old_stamp() {
  sed -i.bak 's/^started_at: .*/started_at: 2020-01-01T00:00:00Z/' "$S/sessions-open/$1.yaml"
  rm -f "$S/sessions-open/$1.yaml.bak"
  awk -v s="$2" '{ if (index($0, "\"session\":\"" s "\"") > 0) sub(/"ts":"[^"]*"/, "\"ts\":\"2020-01-01T00:00:00Z\""); print }' \
    "$S/ledger.jsonl" > "$S/ledger.tmp" && mv "$S/ledger.tmp" "$S/ledger.jsonl"
}
old_stamp gone "$DEAD"

# ---------------------------------------------------------------- the measurement first
# The rule requires the value that decided a verdict to be printed before anything is done
# with it, for every candidate, including the ones nothing happens to.
expect_exit 0 "$MJ" recover episodes --check
expect_grep "$DEAD.*last seen 2020-01-01T00:00:00Z"
expect_grep 'stranded: .*over the 12h threshold'
expect_grep "$LIVE.*last seen"
expect_grep 'check: 1 action'
# --check is a dry run and the word is not decoration
must "the dry run left the open episode where it was" [ -f "$S/sessions-open/gone.yaml" ]
must "the dry run published no record" [ "$(n_records)" = 0 ]

# ---------------------------------------------------------------- recovery
expect_exit 0 "$MJ" recover episodes
expect_grep 'recovered: 1 action'
must "the stranded episode's open record is gone" [ ! -f "$S/sessions-open/gone.yaml" ]
# the live one is untouched, which is the guarantee this case is named for
must "the LIVE episode is still open" [ -f "$S/sessions-open/here.yaml" ]
expect_exit 0 "$MJ" session status
expect_grep "Session: *$LIVE"

REC="$(grep -l "^session_id: $DEAD\$" .ai/repo/sessions/*.md)"
expect_file "$REC"
must "outcome is interrupted, which is what it was" grep -q '^outcome: interrupted$' "$REC"
must "started_at came from the open record" grep -q "^started_at: 2020-01-01T00:00:00Z\$" "$REC"
# Never fabricated. The working tree at recovery time belongs to whoever is working now, so
# a recovered record claims no commits and no changed files, and says neither head nor
# working_tree, which describe a close that never happened.
must "no commits are claimed" grep -q '^commits: \[\]$' "$REC"
must "no changed files are claimed" grep -q '^changed_files: \[\]$' "$REC"
must "no head, which would describe a close that never happened" [ "$(grep -c '^head: ' "$REC")" = 0 ]
must "no working_tree, for the same reason" [ "$(grep -c '^working_tree: ' "$REC")" = 0 ]
# the record says how it came to exist, where a person reading it will be standing
must "the body names the recovery" grep -q 'closed by .majordomus recover.' "$REC"
must "the body names the evidence" grep -q 'last sign of life: 2020-01-01T00:00:00Z' "$REC"

# the reason is a ledger event, and it names the episode recovered rather than the recoverer
must "a session.recovered event exists" grep -q '"event":"session.recovered"' "$S/ledger.jsonl"
must "it names the episode recovered, not the recoverer" grep -q "\"session_id\":\"$DEAD\"" "$S/ledger.jsonl"
must "it carries the reason" grep -q '"reason":"no end event' "$S/ledger.jsonl"

# ---------------------------------------------------------------- exactly once
expect_exit 0 "$MJ" recover episodes
expect_grep 'nothing to recover'
must "still exactly one record for the episode" [ "$(grep -l "^session_id: $DEAD\$" .ai/repo/sessions/*.md | wc -l | tr -d ' ')" = 1 ]

# An interrupted run is the same as a re-run: the record is written before the open file is
# removed, so a crash between them leaves both, and the next run keeps the record it finds.
# `pwd -P` and not `$PWD`: on macOS the scratch repository is under /var, which is a symlink
# to /private/var, and MJ_ROOT resolves to the real path. A record written with the
# unresolved one is *foreign*, not stranded, and the first draft of this case proved that
# recovery refuses to close another checkout's episode instead of what it meant to prove.
printf 'session_id: %s\nstarted_at: 2020-01-01T00:00:00Z\nowner: "t"\nworktree: %s\nbranch: %s\nstart_head: %s\nstart_working_tree: clean\n' \
  "$DEAD" "$(pwd -P)" "$(git branch --show-current)" "$(git rev-parse HEAD)" > "$S/sessions-open/gone.yaml"
expect_exit 0 "$MJ" recover episodes
expect_grep 'record exists at'
must "the re-opened record was torn down" [ ! -f "$S/sessions-open/gone.yaml" ]
must "and no second record was written" [ "$(grep -l "^session_id: $DEAD\$" .ai/repo/sessions/*.md | wc -l | tr -d ' ')" = 1 ]

# ---------------------------------------------------------------- fail closed
# The guard the rule is about: an episode whose last sign of life cannot be read as a
# timestamp is not old, it is unmeasured. It is skipped, counted, and never closed.
printf 'session_id: s-unreadable-0001\nstarted_at: not-a-timestamp\nowner: "t"\nworktree: %s\nbranch: %s\nstart_head: %s\nstart_working_tree: clean\n' \
  "$(pwd -P)" "$(git branch --show-current)" "$(git rev-parse HEAD)" > "$S/sessions-open/broken.yaml"
# and one with no identity at all
printf 'owner: "t"\nstarted_at: 2020-01-01T00:00:00Z\n' > "$S/sessions-open/nameless.yaml"
expect_exit 0 "$MJ" recover episodes
expect_grep 'cannot read as a timestamp|which this platform cannot read'
expect_grep 'carries no session_id'
expect_grep 'skipped 2 candidate'
must "an unmeasurable episode is left open" [ -f "$S/sessions-open/broken.yaml" ]
must "so is one with no identity" [ -f "$S/sessions-open/nameless.yaml" ]
must "and neither produced a record" [ "$(n_records)" = 1 ]
rm -f "$S/sessions-open/broken.yaml" "$S/sessions-open/nameless.yaml"

# ---------------------------------------------------------------- duplicate records
# The incident: one episode, four records, because a provider's end event fired four times
# and mj_publish_record gives every file a unique name. The union of what they prove becomes
# one record; nothing is picked between them on a field where they disagree.
mk_record() {
  local file="$1" sid="$2" created="$3" extra="$4"
  {
    printf -- '---\nschema: session/v1\nkind: session\ncreated_at: %s\ntask_id: none\nprofile: none\n' "$created"
    printf 'repository_id: local:test\nworktree_id: test\nbranch: main\nchanged_files:\n'
    printf '  - shared.txt\n  - %s\n' "$extra"
    printf 'session_id: %s\nstarted_at: 2020-02-02T00:00:00Z\nclosed_at: %s\noutcome: closed\n' "$sid" "$created"
    printf 'title: "dup"\nstart_head: abc1234\nstart_working_tree: clean\ncommits:\n  - "%s"\n' "${extra%%.*}"
    printf -- '---\n'
  } > ".ai/repo/sessions/$file"
}
mk_record "20260101T000000Z--s-dup-0001--main--aaa1111--1.md" s-dup-0001 2026-01-01T00:00:00Z first.txt
mk_record "20260102T000000Z--s-dup-0001--main--aaa1111--2.md" s-dup-0001 2026-01-02T00:00:00Z second.txt
mk_record "20260103T000000Z--s-dup-0001--main--aaa1111--3.md" s-dup-0001 2026-01-03T00:00:00Z third.txt

expect_exit 0 "$MJ" recover records --check
expect_grep 's-dup-0001 — 3 records'
expect_grep 'canonical: 20260101T000000Z.*oldest created_at 2026-01-01'
expect_grep 'union: 4 changed_files, 3 commits'
must "the dry run removed none of the three" [ "$(grep -l '^session_id: s-dup-0001$' .ai/repo/sessions/*.md | wc -l | tr -d ' ')" = 3 ]

expect_exit 0 "$MJ" recover records
expect_grep 'folded into'
KEEP="$(grep -l '^session_id: s-dup-0001$' .ai/repo/sessions/*.md)"
must "exactly one record survives" [ "$(printf '%s\n' "$KEEP" | wc -l | tr -d ' ')" = 1 ]
# the union is preserved, not the canonical record's own half of it
must "the shared entry survives" grep -q '^  - shared.txt$' "$KEEP"
must "the canonical record's own entry survives" grep -q '^  - first.txt$' "$KEEP"
must "the second record's entry was unioned in" grep -q '^  - second.txt$' "$KEEP"
must "and the third's" grep -q '^  - third.txt$' "$KEEP"
must "the union de-duplicates" [ "$(grep -c '^  - shared.txt$' "$KEEP")" = 1 ]
must "commits are unioned too" grep -q '^  - "first"$' "$KEEP"
must "every one of them" grep -q '^  - "third"$' "$KEEP"
# and the provenance is in the record, not only in the ledger
must "the provenance is in the record itself" grep -q '^## Recovery$' "$KEEP"
must "it says how many there were" grep -q 'had 3 closed records' "$KEEP"
must "and names the superseded files" grep -q '20260103T000000Z' "$KEEP"
must "the fold is a ledger event" grep -q '"session_id":"s-dup-0001"' "$S/ledger.jsonl"
# idempotent
expect_exit 0 "$MJ" recover records
expect_grep 'no episode has more than one record'

# A disagreement about the episode's own identity is reported, and nothing is folded: an
# episode whose records cannot agree on when it started is a defect upstream of recovery.
mk_record "20260201T000000Z--s-cnf-0001--main--bbb2222--1.md" s-cnf-0001 2026-02-01T00:00:00Z a.txt
mk_record "20260202T000000Z--s-cnf-0001--main--bbb2222--2.md" s-cnf-0001 2026-02-02T00:00:00Z b.txt
sed -i.bak 's/^started_at: .*/started_at: 2020-09-09T00:00:00Z/' ".ai/repo/sessions/20260202T000000Z--s-cnf-0001--main--bbb2222--2.md"
rm -f .ai/repo/sessions/*.bak
expect_exit 0 "$MJ" recover records
expect_grep 'records disagree'
expect_grep 'conflict: nothing folded, nothing removed'
must "a conflict folds nothing" [ "$(grep -l '^session_id: s-cnf-0001$' .ai/repo/sessions/*.md | wc -l | tr -d ' ')" = 2 ]
rm -f .ai/repo/sessions/*s-cnf-0001*.md

# ---------------------------------------------------------------- orphans
# Four temps, four verdicts. Nothing here is decided by the file name.
: > .ai/repo/sessions/.tmp.empty000
printf 'this is not a record of any kind\n' > .ai/repo/sessions/.tmp.foreign0
# one whose episode is already published: the hard link succeeded and the temp was not removed
printf 'session_id: %s\n' "$DEAD" > .ai/repo/sessions/.tmp.publishd
# one holding the only copy of an episode: a publish that died before the link
{ printf -- '---\nschema: session/v1\nkind: session\ncreated_at: 2026-03-03T00:00:00Z\n'
  printf 'task_id: none\nprofile: none\nrepository_id: local:test\nworktree_id: test\n'
  printf 'branch: main\nchanged_files: []\nsession_id: s-orphan-0001\n'
  printf 'started_at: 2026-03-03T00:00:00Z\nclosed_at: 2026-03-03T00:00:00Z\noutcome: closed\n'
  printf 'title: "rescued"\nstart_head: abc1234\nstart_working_tree: clean\ncommits: []\n---\n'
} > .ai/repo/sessions/.tmp.rescue00
# a staging directory of the site generator, and a rename temp whose target arrived
mkdir -p .mj-stage.leftover
touch "$S/checkpoints/keep.md.mj-tmp" "$S/checkpoints/keep.md"
# and a directory somebody borrowed the checkpoint store for
mkdir -p "$S/checkpoints/campaign-scratch"
printf '# ledger\n' > "$S/checkpoints/campaign-scratch/LEDGER.md"

# Every stray file is age-gated: a publish temp a few milliseconds old belongs to a close
# running right now, and a .mj-stage a few minutes old to a derive running right now. The
# fixture ages them, which is also the assertion that the gate exists — without the touch
# below, none of these is a candidate at all.
for stray in .ai/repo/sessions/.tmp.empty000 .ai/repo/sessions/.tmp.foreign0 \
             .ai/repo/sessions/.tmp.publishd .ai/repo/sessions/.tmp.rescue00 \
             .mj-stage.leftover "$S/checkpoints/keep.md.mj-tmp"; do
  touch -t 202001010000 "$stray"
done
# one that is *not* aged: a close or a derive running right now must survive the sweep
: > .ai/repo/sessions/.tmp.inflight
expect_exit 0 "$MJ" recover orphans --check
expect_grep 'live +\.ai/repo/sessions/\.tmp\.inflight.*under the 12h threshold'
expect_grep 'orphan +\.ai/repo/sessions/\.tmp\.empty000'
expect_grep 'foreign +\.ai/repo/sessions/\.tmp\.foreign0'
expect_grep 'incomplete +\.ai/repo/sessions/\.tmp\.rescue00'
expect_grep 'foreign +.*campaign-scratch'
expect_grep 'holds 1 Markdown file'
# a dry run removed nothing
must "the dry run kept the empty temp" [ -f .ai/repo/sessions/.tmp.empty000 ]
must "and the staging directory" [ -d .mj-stage.leftover ]

expect_exit 0 "$MJ" recover orphans
must "a temp younger than the threshold is left alone" [ -f .ai/repo/sessions/.tmp.inflight ]
must "an empty temp is removed" [ ! -f .ai/repo/sessions/.tmp.empty000 ]
must "so is one whose episode is already published" [ ! -f .ai/repo/sessions/.tmp.publishd ]
must "and a leftover staging directory" [ ! -d .mj-stage.leftover ]
must "and a rename temp whose target arrived" [ ! -f "$S/checkpoints/keep.md.mj-tmp" ]
# the unpublished record was published rather than deleted
must "the rescued temp is gone from the store" [ ! -f .ai/repo/sessions/.tmp.rescue00 ]
expect_file "$(grep -l '^session_id: s-orphan-0001$' .ai/repo/sessions/*.md)"
# content this version cannot classify is left exactly where it is, and counted
must "unclassifiable content is left exactly where it is" [ -f .ai/repo/sessions/.tmp.foreign0 ]
# a directory in the checkpoint store is never deleted by this command
must "a foreign directory is never removed" [ -d "$S/checkpoints/campaign-scratch" ]
must "nor anything inside it" [ -f "$S/checkpoints/campaign-scratch/LEDGER.md" ]

expect_exit 0 "$MJ" recover orphans
expect_grep 'skipped 1 candidate'

# ---------------------------------------------------------------- status is read-only
before="$(find .ai -type f | wc -l | tr -d ' ')"
expect_exit 0 "$MJ" recover status
expect_exit 0 "$MJ" recover
must "status wrote nothing" [ "$(find .ai -type f | wc -l | tr -d ' ')" = "$before" ]

# ---------------------------------------------------------------- the stores with an owner
# archive/ and completed/ look legacy and are not: each has exactly one writer. The report
# says so, and says the other half too — neither has a reader anywhere.
mkdir -p "$S/archive" "$S/completed"
touch "$S/archive/t-old.yaml" "$S/completed/t-old.md"
expect_exit 0 "$MJ" recover orphans
expect_grep 'live +.*state/archive.*written by lib/start.sh'
expect_grep 'live +.*state/completed.*written by lib/finish.sh'
must "archive/ is reported, never touched" [ -f "$S/archive/t-old.yaml" ]
must "and completed/ likewise" [ -f "$S/completed/t-old.md" ]


# ---------------------------------------------------------------- the order is correctness
# Measured on the real store. In the primary checkout on 2026-09-11, episode
# s-20260910200453-afbb was open *and* stranded, and .ai/repo/sessions/.tmp.5bcIMW held the
# only copy of its closed record: a publish that wrote the temp, died before the hard link,
# and never tore the open record down. Recover episodes first and that episode gets a freshly
# synthesised record with nothing in it, after which orphans sees the temp's episode as
# "already published" and removes the complete one. `recover all` runs orphans first, and
# this is the case that says so.
rm -f "$S"/sessions-open/*.yaml "$S/session-current.yaml"
"$MJ" session start --provider-session interrupted >/dev/null
BOTH="$(sed -n 's/^session_id: //p' "$S/sessions-open/interrupted.yaml")"
old_stamp interrupted "$BOTH"
# A live episode beside it, opened after, so that the pointer names the live one. Without it
# the checkout has exactly one open episode, the pointer necessarily names it, and the
# stranded episode would be exempt as "this process's" — a fixture in which nothing could be
# recovered whatever the evidence said.
"$MJ" session start --provider-session still-here >/dev/null
# the record its publish never linked, carrying a fact only it knows
{ printf -- '---\nschema: session/v1\nkind: session\ncreated_at: 2026-03-03T00:00:00Z\n'
  printf 'task_id: none\nprofile: none\nrepository_id: local:test\nworktree_id: test\n'
  printf 'branch: main\nchanged_files:\n  - the-work-it-actually-did.txt\n'
  printf 'session_id: %s\nstarted_at: 2020-01-01T00:00:00Z\nclosed_at: 2026-03-03T00:00:00Z\n' "$BOTH"
  printf 'outcome: closed\ntitle: "the complete record"\nstart_head: abc1234\n'
  printf 'start_working_tree: clean\ncommits:\n  - "deadbee"\n---\n'
} > .ai/repo/sessions/.tmp.interrup
# aged, like every other stray in this case: a temp written a moment ago belongs to a close
# that is still running, and recovery leaves it exactly where it is
touch -t 202001010000 .ai/repo/sessions/.tmp.interrup

expect_exit 0 "$MJ" recover all
must "the open record was torn down" [ ! -f "$S/sessions-open/interrupted.yaml" ]
KEPT="$(grep -l "^session_id: $BOTH\$" .ai/repo/sessions/*.md)"
must "exactly one record for it" [ "$(printf '%s\n' "$KEPT" | wc -l | tr -d ' ')" = 1 ]
must "and it is the complete one the publish had written" \
  grep -q '^  - the-work-it-actually-did.txt$' "$KEPT"
must "not a synthesised one with nothing in it" [ "$(grep -c '^changed_files: \[\]$' "$KEPT")" = 0 ]
must "its commits survived too" grep -q '^  - "deadbee"$' "$KEPT"

echo "  135 ok"
