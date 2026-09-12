# majordomus-covers: none
# A case number names one case: the gate that refuses a second file taking one, and the
# survey that says which one is next.
#
# On 2026-09-12, across 812 refs, 55 of the 172 case identities ever added had been carried
# by more than one distinct file, and three sessions took 278 within minutes. Each read the
# tree it branched from. The allocator reads four sources — this tree, every sibling worktree
# on disk, every ref's history, and the peer board — and this case builds a repository in
# which every one of them changes the answer, so a survey that skips any of them answers
# wrongly here and fails by name.
#
# Every fixture is its own repository; nothing reads or edits this checkout's cases.
. "$ROOT/test/lib.sh"
CN="$ROOT/scripts/case-numbers"

mkcase() { local f; for f in "$@"; do printf '# a case\ntrue\n' > "$f"; done; }
g() {
  local d="$1"; shift
  git -C "$d" -c user.name=case -c user.email=case@example.invalid -c commit.gpgsign=false \
    -c core.hooksPath=/dev/null "$@"
}
fixture() {  # fixture <dir> <case file>... — a tree with cases, no git
  local d="$1"; shift
  mkdir -p "$d/test/cases" "$d/.ai/repo/ci"
  local f; for f in "$@"; do mkcase "$d/test/cases/$f"; done
}

# ================================================================ the gate
# ---------------------------------------------------------------- every identity is its own
T1="$T/unique"; fixture "$T1" 01_alpha.sh 02_beta.sh 03_gamma.sh
expect_exit 0 env MJ_ROOT="$T1" "$CN" check
expect_grep 'no case identity is newly shared: 3 files, 3 identities'
expect_exit 0 env MJ_ROOT="$T1" "$CN" check --strict
expect_grep 'every case identity names one case: 3 files, 3 identities'

# ---------------------------------------------------------------- a second file takes one
T2="$T/collides"; fixture "$T2" 01_alpha.sh 02_beta.sh 02_beta_again.sh
expect_exit 10 env MJ_ROOT="$T2" "$CN" check
expect_grep '^REFUSE'
expect_grep '^  02_beta\.sh +shares 02 with: 02_beta_again\.sh'
expect_grep '^  02_beta_again\.sh +shares 02 with: 02_beta\.sh'
# the verdict states its population, and says where a free identity comes from
expect_grep 'examined 3 files, 2 identities'
expect_grep 'scripts/case-numbers next'

# ---------------------------------------------------------------- the ratchet
expect_exit 0 env MJ_ROOT="$T2" "$CN" write-baseline
expect_grep 'recorded 2 file\(s\) across 1 identit'
expect_file "$T2/.ai/repo/ci/case-numbers-baseline.txt"
expect_exit 0 env MJ_ROOT="$T2" "$CN" check
expect_grep '2 file\(s\) across 1 identit\(ies\) are accepted debt'
# --strict still refuses it: the debt is accepted, not forgotten
expect_exit 10 env MJ_ROOT="$T2" "$CN" check --strict
expect_grep '^  02_beta\.sh '
expect_grep 'the baseline is not consulted'

# ---------------------------------------------------------------- the debt may not grow
# A third file joining a collision the baseline carries is refused, and only the newcomer is
# named as refused: the two on the baseline are not re-reported as new.
mkcase "$T2/test/cases/02_beta_thrice.sh"
expect_exit 10 env MJ_ROOT="$T2" "$CN" check
expect_grep '^  02_beta_thrice\.sh +shares 02 with: 02_beta\.sh 02_beta_again\.sh'
expect_no_grep '^  02_beta_again\.sh +shares'
rm "$T2/test/cases/02_beta_thrice.sh"
expect_exit 0 env MJ_ROOT="$T2" "$CN" check

# ---------------------------------------------------------------- a renumbering is welcome
# The baseline names what may collide, not what must; entries that stopped colliding are named
# so the list can shrink, and are not a failure.
mv "$T2/test/cases/02_beta_again.sh" "$T2/test/cases/04_beta_again.sh"
expect_exit 0 env MJ_ROOT="$T2" "$CN" check
expect_grep '2 baseline entr\(ies\) no longer collide'
expect_exit 0 env MJ_ROOT="$T2" "$CN" check --strict
expect_grep 'every case identity names one case: 3 files, 3 identities'

# ---------------------------------------------------------------- the deliberate second half
T3="$T/suffixed"; fixture "$T3" 87_release_pipeline.sh 87b_release_archive_shape.sh 12b_x.sh 12c_y.sh
expect_exit 0 env MJ_ROOT="$T3" "$CN" check --strict
expect_grep 'every case identity names one case: 4 files, 4 identities'

# ---------------------------------------------------------------- what is taken, with its denominator
T4="$T/listing"; fixture "$T4" 01_a.sh 01_b.sh 02_c.sh
expect_exit 0 env MJ_ROOT="$T4" "$CN" list
expect_grep '^01 +01_a\.sh 01_b\.sh$'
expect_no_grep '02_c\.sh'
expect_grep '^1 of 2 identities in this tree are held by more than one file \(2 of 3 files\)$'

# ---------------------------------------------------------------- usage is refused, not guessed
expect_exit 2 env MJ_ROOT="$T1" "$CN"
expect_exit 2 env MJ_ROOT="$T1" "$CN" allocate
expect_grep 'unknown subcommand allocate'
expect_exit 2 env MJ_ROOT="$T1" "$CN" next --strict
expect_exit 2 env MJ_ROOT="$T1" "$CN" check --json
mkdir -p "$T/nocases"
expect_exit 12 env MJ_ROOT="$T/nocases" "$CN" check
expect_grep 'no test/cases/'

# ================================================================ the survey
# A repository in which each source moves the answer:
#
#   master          01_alpha 02_beta 03_gamma, then renames 03_gamma to 04_gamma
#   feature/five    05_five, unmerged — an identity the base lacks, held by a ref
#   rescue/seven    added 07_seven and deleted it — spent, and carried by nothing
#   feature/other   02_other — a different file under an identity the base holds
#   a worktree      09_nine, authored and never committed — the highest identity anywhere
#
# The three branches were cut before the rename, so each still carries 03_gamma: a generation
# of the base, not an allocation, and the survey must not list 03 as outstanding.
A="$T/alloc"; mkdir -p "$A/test/cases"
g "$A" init -q -b master
mkcase "$A/test/cases/01_alpha.sh" "$A/test/cases/02_beta.sh" "$A/test/cases/03_gamma.sh"
g "$A" add -A && g "$A" commit -q -m base
g "$A" checkout -q -b feature/five
mkcase "$A/test/cases/05_five.sh"; g "$A" add -A && g "$A" commit -q -m five
g "$A" checkout -q -b rescue/seven master
mkcase "$A/test/cases/07_seven.sh"; g "$A" add -A && g "$A" commit -q -m seven
g "$A" rm -q test/cases/07_seven.sh && g "$A" commit -q -m 'seven withdrawn'
g "$A" checkout -q -b feature/other master
mkcase "$A/test/cases/02_other.sh"; g "$A" add -A && g "$A" commit -q -m other
g "$A" checkout -q master
g "$A" mv test/cases/03_gamma.sh test/cases/04_gamma.sh && g "$A" commit -q -m renumber
W="$T/alloc-nine"
g "$A" worktree add -q -b feature/nine "$W" master
mkcase "$W/test/cases/09_nine.sh"

survey() { expect_exit 0 env MJ_ROOT="$A" MJ_CASE_BASE=master MJ_CASE_NO_BOARD=1 "$CN" "$@"; }

survey next
# the worktree's uncommitted file is the highest claim, and only a disk read sees it
expect_grep '^next free identity: case 10$'
expect_grep '^highest claimed: case 9  \(worktree: 09_nine\.sh\)$'
expect_grep 'every sibling worktree, as it stands on disk +[0-9]+ identities, from 1 of 1 worktrees'
# 01 02 03 05 07 and 04 by the rename; 02_other adds a file and no identity; across the five refs
expect_grep 'every ref: branches, tags, remotes, rescue refs +6 identities ever added, across 5 refs'
# the board is a source, and not asking it is said rather than absorbed
expect_grep 'the peer board of the shared server +not asked \(MJ_CASE_NO_BOARD=1\)'
# the holes are spent: 0, 6 and 8 below 9
expect_grep '^3 of 9 identities below it \(0-8\) are claimed by nothing: spent, not free\.$'
expect_grep '^  0 6 8$'
# outstanding, with who holds each: a branch nobody mentioned, and a worktree on a disk
expect_grep '^2 identit\(ies\) allocated and not landed on master'
expect_grep '^  05 +05_five\.sh held by: 1 ref \(feature/five\)$'
expect_grep '^  09 +09_nine\.sh held by: 1 worktree \(.*alloc-nine\)$'
# withdrawn is spent, not outstanding; a generation of the base is not an allocation
expect_no_grep '07_seven\.sh held by'
expect_no_grep '^  03 '
# a different file under an identity the base holds collides the day it lands
expect_grep '^1 identit\(ies\) that master carries are also carried, under a different file'
# and the limit of the survey is stated, every time
expect_grep '^not surveyed, and not surveyable'
expect_grep 'invisible to both, so case 10 is the best reading of what could be reached, not a reservation'

# Withdraw the worktree's file: the history of a deleted file still holds 7, so the answer is 8
# and not 6 — which is what a survey of current trees alone would say. And a file authored in
# this tree is an outstanding claim of its own.
rm "$W/test/cases/09_nine.sh"
mkcase "$A/test/cases/06_six.sh"
survey next
expect_grep '^next free identity: case 8$'
expect_grep '^highest claimed: case 7  \(ref: 07_seven\.sh\)$'
expect_grep '^  06 +06_six\.sh held by: this working tree$'
expect_grep '^1 of 7 identities below it \(0-6\)'

survey next --json
if command -v jq >/dev/null 2>&1; then
  printf '%s' "$LAST_OUT" | jq -e '.next == 8 and .highest_claimed == 7 and .board == "unasked"
    and .base == "master" and (.outstanding | map(.id) == ["05", "06"]) and .contested_on_base == 1
    and (.sources[] | select(.source == "peer") | .reachable == false)' >/dev/null \
    || { printf '    next --json is not the survey it printed:\n%s\n' "$LAST_OUT"; exit 1; }
else
  expect_grep '"next":8,"highest_claimed":7'
fi

survey collisions
expect_grep '^02  2 files$'
expect_grep '^  02_beta\.sh +on master — '
expect_grep '^  02_other\.sh +not on master — 1 ref \(feature/other\)$'
expect_grep '^1 of [0-9]+ identities ever claimed have been carried by more than one distinct file;$'

# ---------------------------------------------------------------- a base that does not resolve
expect_exit 0 env MJ_ROOT="$A" MJ_CASE_BASE=no-such-ref MJ_CASE_NO_BOARD=1 "$CN" next
expect_grep 'outstanding allocations were NOT surveyed: no base ref resolves here \(no-such-ref\)'
expect_no_grep 'allocated and not landed'

# ---------------------------------------------------------------- the board
# The one source that sees a number decided and written nowhere. A session names the identity
# in whichever of its scope paths and its intent it thinks of, so each is announced by a peer
# that uses only that one: a parse that reads either alone loses a claim and answers lower.
# The output says the highest claim is a board claim — which may be the asker's own.
if command -v jq >/dev/null 2>&1; then
  printf '%s\n' '{"peers":[
    {"id":"p3","announcement":{"name":"claude","scope":["test/cases/12_twelve.sh"],"intent":"a scope path only"}},
    {"id":"p4","announcement":{"name":"codex","scope":[],"intent":"allocating case 14 now"}}]}' > "$T/board.json"
  expect_exit 0 env MJ_ROOT="$A" MJ_CASE_BASE=master MJ_CASE_BOARD_FILE="$T/board.json" "$CN" next
  expect_grep '^next free identity: case 15$'
  expect_grep 'the peer board of the shared server +2 identities announced'
  expect_grep '^highest claimed: case 14  \(peer: p4 \(codex\)\)$'
  expect_grep '^that is a board claim, not a file'
  expect_grep '^  12 +announced on the board p3 \(claude\)$'
  expect_grep '^  14 +announced on the board p4 \(codex\)$'
  # a board that cannot be read is a source not reached, never an empty one
  printf 'not json\n' > "$T/board.bad"
  expect_exit 0 env MJ_ROOT="$A" MJ_CASE_BASE=master MJ_CASE_BOARD_FILE="$T/board.bad" "$CN" next
  expect_grep 'the peer board of the shared server +not reachable'
  expect_grep '^next free identity: case 8$'
else
  echo "    (jq absent: the board parse is not measured here)"
fi

echo "    a case identity names one case: a new collision is refused and today's debt may not grow; next reads the tree, every worktree, every ref's history and the board, names what is outstanding and who holds it, and states what it could not reach"
