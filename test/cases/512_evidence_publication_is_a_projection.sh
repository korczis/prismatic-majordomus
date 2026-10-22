# majordomus-covers: none
# The evidence publication is an allow-listed projection (I1955, ADR 0087 decisions 3 and 14).
# `evidence publish` writes what the site publishes about evidence: every claim judged from
# the ledger committed at the presented commit, the run records it is given read only through
# the monotone rule, one publication state from decision 3's table, only allow-listed fields,
# and every text through the one redactor. This case drives it through the Rust executable in
# a fixture repository of its own, with run records the real recorder wrote.
#
# What it proves, in order:
#   1  the presented commit's own run record, measured clean, makes the publication current;
#      the file is the answer, it carries no execution, command or digest, and the text
#      rendering names the state, the record and the file
#   2  a record whose run failed the claim's case caps the claim at stale and is named; a
#      dirty build that the same record contradicts reads dirty and still names it
#   3  no record of the presented commit reads pending while its run has not concluded, and
#      unknown once it has, or when no run is found
#   4  a record of an ancestor only never reads current: it is listed by its commit with the
#      commits and files between, and a failing one can only cap a verdict
#   5  no published verdict is stronger than `evidence show --presented HEAD`
#   6  an --out inside the checkout exits 13 and writes nothing
#
# The reports, the run records and the publications live outside the repository under test:
# a file written inside it would be an untracked file, which is a dirty tree. The recorder
# writes its rows into the local ledger under .ai/local/, which init ignores.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq: the publication is read with jq"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

W="$(mktemp -d "${TMPDIR:-/tmp}/mj512.XXXXXX")"; trap 'rm -rf "$W"' EXIT

pub() {          # pub <name> <args...> — the JSON publication of `evidence publish` into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" evidence --repo "$T" --format json publish --presented HEAD \
    --out "$W/out-$name" "$@" > "$W/$name.answer.json"
  jq -e '.publication' "$W/$name.answer.json" > "$W/$name.json"
  cmp -s <(jq -S . "$W/$name.json") <(jq -S . "$W/out-$name/evidence-publication.json") \
    || { echo "    the file $name wrote is not the publication it answered"; exit 1; }
}
jqe() {          # jqe <name> <filter> <what broke> [jq args...]
  local name="$1" filter="$2" what="$3"; shift 3
  jq -e "$@" "$filter" "$W/$name.json" >/dev/null 2>&1 || {
    printf '    %s\n' "$what"; jq -c . "$W/$name.json" | head -c 2000; echo; return 1; }
}
clean() {        # clean <what> — the fixture's tree is clean, or the case says what dirtied it
  [ -z "$(git status --porcelain)" ] || { echo "    $1 left the tree dirty"; git status --porcelain; exit 1; }
}
run_record() {   # run_record <outcome word> <file> — record one run of the case, as CI would keep it
  printf '901_alpha\t%s\t1\tparallel\n' "$1" > "$W/run.tsv"
  run_quiet "$W/stamp.err" "$MJB" evidence --repo "$T" stamp --producer suite \
    --report "$W/run.tsv" --out "$W/psuite.json" > /dev/null
  mkdir -p "$(dirname "$2")"
  run_quiet "$W/record.err" "$MJB" evidence --repo "$T" record --suite "$W/run.tsv" \
    --provenance "suite=$W/psuite.json" --ledger local --run-record "$2" > /dev/null
}

# ---------------------------------------------------------------- the fixture
# One claim, and a passing run of its case recorded into the tracked ledger and committed:
# at that commit the claim is judged from the ledger the commit holds.
"$MJ" init >/dev/null
mkdir -p docs lib test/cases
printf 'version: 1\nclaims:\n  - id: alpha-holds\n    claim: Alpha holds\n' > docs/CLAIMS.yaml
printf '    source: docs/ALPHA.md\n    implementation: lib/alpha.sh\n' >> docs/CLAIMS.yaml
printf '    test: test/cases/901_alpha.sh\n    status: guaranteed\n' >> docs/CLAIMS.yaml
printf '# Alpha\n' > docs/ALPHA.md
printf '#!/usr/bin/env bash\n# alpha\n' > lib/alpha.sh
printf '# majordomus-%s: %s\ntrue\n' covers none > test/cases/901_alpha.sh
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm fixture
printf '901_alpha\tok\t1\tparallel\n' > "$W/run.tsv"
run_quiet "$W/stamp.err" "$MJB" evidence --repo "$T" stamp --producer suite \
  --report "$W/run.tsv" --out "$W/psuite.json" > /dev/null
run_quiet "$W/record.err" "$MJB" evidence --repo "$T" record --suite "$W/run.tsv" \
  --provenance "suite=$W/psuite.json" > /dev/null
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm "the recorded run"
P2="$(git rev-parse HEAD)"
clean "the fixture"

# ---------------------------------------------------------------- 1. the commit's own record
run_record ok "$W/own/record.json"
clean "a local recording"
printf '{"schema": 1}\n' > "$W/own/manifest.json"
pub own --from "$W/own" --run concluded
jqe own '.state == "current" and .presented.commit == $p and .presented.tree == "clean"' \
  "the commit's own clean run record does not make the publication current" --arg p "$P2"
jqe own '.records | length == 1 and .[0].relation == "own" and .[0].commit == $p and .[0].working_tree == "clean"' \
  "the commit's own record is not listed as its own" --arg p "$P2"
jqe own '[.claims[] | {id, state}] == [{id: "alpha-holds", state: "proven"}]' \
  "the claim is not proven from the ledger the commit holds"
jqe own '.unread == [{file: "manifest.json", reason: .unread[0].reason}]' \
  "the file that is not a run record is not named as unread"
jqe own '[paths | select(. != ["ledger", "executions"]) | map(strings) | .[]
    | select(. == "executions" or . == "execution" or . == "command" or . == "digest" or . == "provenance")] == []' \
  "the publication carries a field off the allow-list: only the ledger's count names executions"
expect_exit 0 "$MJB" evidence --repo "$T" publish --presented HEAD --from "$W/own" --out "$W/out-text"
expect_grep '^published +current at '
expect_grep '^record +own '
expect_grep "^written +.*evidence-publication.json"

# ---------------------------------------------------------------- 2. a record that contradicts
run_record fail "$W/failed/record.json"
clean "a failing local recording"
pub failed --from "$W/failed"
jqe failed '.state == "stale" and (.capped_by | length == 1) and .claims[0].state == "stale"
    and .claims[0].capped_by == .capped_by and .records[0].capped == ["alpha-holds"]' \
  "a record whose run failed did not cap the claim at stale, naming itself"
pub dirty --from "$W/failed" --presented-tree dirty
jqe dirty '.state == "dirty" and .presented.tree == "dirty" and (.capped_by | length == 1)
    and ([.reasons[] | select(contains($id))] | length >= 1)' \
  "a dirty build that a record contradicts does not read dirty and name the record" \
  --arg id "$(jq -r '.capped_by[0]' "$W/failed.json")"

# ---------------------------------------------------------------- 3. no record of the commit
pub pending --run pending
jqe pending '.state == "pending" and .records == []' "no record and a run in progress is not pending"
pub concluded --run concluded
jqe concluded '.state == "unknown" and (.reasons[0] | test("concluded without a run record"))' \
  "a run concluded without a record does not read unknown, naming why"
pub absent --run absent
jqe absent '.state == "unknown" and (.reasons[0] | test("no CI run"))' "no run found does not read unknown"

# ---------------------------------------------------------------- 4. an ancestor's record only
printf 'more\n' > docs/OTHER.md
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm another
clean "another commit"
for run in concluded pending; do
  pub "ancestor-$run" --from "$W/own" --run "$run"
  jqe "ancestor-$run" '.state != "current" and .records[0].relation == "ancestor" and .records[0].commit == $p
      and .records[0].commits_between == 1 and .records[0].changed == ["docs/OTHER.md"]' \
    "a record of an ancestor only reads current, or is not listed with what lies between" --arg p "$P2"
done
jqe ancestor-pending '.state == "pending"' "an ancestor's record with the run in progress is not pending"
pub ancestor-failed --from "$W/failed" --run concluded
jqe ancestor-failed '.state == "stale" and .claims[0].state == "stale"' "an ancestor's failing record caps nothing"

# ---------------------------------------------------------------- 5. never stronger than the ledger
run_quiet "$W/show.err" "$MJB" evidence --repo "$T" --format json show --presented HEAD > "$W/show.json"
for name in ancestor-pending ancestor-failed; do
  jq -e --slurpfile show "$W/show.json" '
    def rank: {"proven":0,"inputs_unchanged":1,"stale":2,"failing":3,"not_run":4,"unrunnable":5,"no_test":6}[.];
    [.claims[] as $c | $show[0].claims[] | select(.id == $c.id) | ($c.state | rank) >= (.state | rank)] | all' \
    "$W/$name.json" >/dev/null \
    || { echo "    $name publishes a verdict stronger than the ledger committed at HEAD"; exit 1; }
done

# ---------------------------------------------------------------- 6. never into the checkout
expect_exit 13 "$MJB" evidence --repo "$T" publish --presented HEAD --out "$T/site/data"
expect_grep 'inside the checkout'
[ ! -e "$T/site/data/evidence-publication.json" ] || { echo "    a refused publication was written"; exit 1; }
clean "a refused publication"
