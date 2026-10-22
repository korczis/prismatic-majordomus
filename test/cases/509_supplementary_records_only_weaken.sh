# majordomus-covers: none
# Supplementary records only weaken (I1953, ADR 0087 decisions 2 and 3). `evidence runs` lists
# the local run index, `evidence run <id>` shows one run, and `evidence subject <key>` reads
# those runs beside the verdict and applies them only through the monotone rule. This case
# drives all three through the Rust executable in a fixture repository of its own, with local
# runs the real stamp and recorder wrote.
#
# What it proves, in order:
#   1  before any local run, the index is listed as not there, and a passing claim answers
#      with its verdict and an empty supplementary section
#   2  a clean failing local run is listed newest first with its id, commit, tree, outcome and
#      counts, and shown alone by its id with its executions; an unknown id exits 13, prints
#      nothing on standard output and names where the runs are listed
#   3  that run caps the passing claim at stale, naming the run on the route and in the
#      supplementary section, and the feature made of the claim reads the weakened member
#   4  a newer passing local run raises nothing back: the claim stays stale, and only the
#      failing run is named
#   5  a failing claim stays failing beside passing local runs: a stronger record never
#      raises a verdict, and weakens nothing
#   6  a source outside the checkout exits 13 for the subject and for the runs
#
# The reports and the measurements live outside the repository under test: a file written
# inside it would be an untracked file, which is a dirty tree, and a run measured on a dirty
# tree weakens nothing. The local runs live under .ai/local/, which init ignores.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq: the answers are read with jq"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

W="$(mktemp -d "${TMPDIR:-/tmp}/mj509.XXXXXX")"; trap 'rm -rf "$W"' EXIT

ev() {           # ev <name> <args...> — the JSON answer of `evidence` into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" evidence --repo "$T" --format json "$@" > "$W/$name.json"
}
jqe() {          # jqe <name> <filter> <what broke> [jq args...]
  local name="$1" filter="$2" what="$3"; shift 3
  jq -e "$@" "$filter" "$W/$name.json" >/dev/null 2>&1 || {
    printf '    %s\n' "$what"; jq -c . "$W/$name.json" | head -c 2000; echo; return 1; }
}
clean() {        # clean <what> — the fixture's tree is clean, or the case says what dirtied it
  [ -z "$(git status --porcelain)" ] || { echo "    $1 left the tree dirty"; git status --porcelain; exit 1; }
}
measured() {     # measured <outcome word> — a report of one run of the case, and its stamp
  printf '901_alpha\t%s\t1\tparallel\n' "$1" > "$W/run.tsv"
  run_quiet "$W/stamp.err" "$MJB" evidence --repo "$T" stamp --producer suite \
    --report "$W/run.tsv" --out "$W/psuite.json" > /dev/null
}
local_run() {    # local_run <outcome word> <name> — record a local run; its id into $W/<name>.id
  measured "$1"
  ev "$2" record --suite "$W/run.tsv" --provenance "suite=$W/psuite.json" --ledger local
  jq -r '.run_record.id' "$W/$2.json" > "$W/$2.id"
  jqe "$2" '.local_run | type == "string"' "the local recording did not reach the local run index"
}

# ---------------------------------------------------------------- the fixture
# One claim and a feature made of it, and a passing run of its case recorded into the tracked
# ledger and committed.
"$MJ" init >/dev/null
mkdir -p docs lib test/cases .ai/repo/features
printf 'version: 1\nclaims:\n  - id: alpha-holds\n    claim: Alpha holds\n' > docs/CLAIMS.yaml
printf '    source: docs/ALPHA.md\n    implementation: lib/alpha.sh\n' >> docs/CLAIMS.yaml
printf '    test: test/cases/901_alpha.sh\n    status: guaranteed\n' >> docs/CLAIMS.yaml
printf '# Alpha\n' > docs/ALPHA.md
printf '#!/usr/bin/env bash\n# alpha\n' > lib/alpha.sh
printf '# majordomus-%s: %s\ntrue\n' covers none > test/cases/901_alpha.sh
{ printf -- '---\nschema: feature/v1\nid: alpha\nkind: feature\n'
  printf "title: 'The feature alpha'\nshort_title: 'alpha'\n"
  printf "headline: 'A feature this case declares.'\nsummary: 'A fixture feature.'\n"
  printf 'status: stable\nweight: 10\nfeatured: false\nclaims: [alpha-holds]\n'
  printf -- '---\n\n## What it does\n\nWhat the case says.\n\n## What it does not do\n\nAnything else.\n'
} > .ai/repo/features/alpha.md
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm fixture
measured ok
run_quiet "$W/record.err" "$MJB" evidence --repo "$T" record --suite "$W/run.tsv" \
  --provenance "suite=$W/psuite.json" > /dev/null
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm "the recorded run"
HEAD1="$(git rev-parse HEAD)"
clean "the fixture"

# ---------------------------------------------------------------- 1. before any local run
ev runs0 runs
jqe runs0 '.source == ".ai/local/evidence/runs" and .present == false and .runs == []' \
  "an index nothing was recorded into is not listed as not there"
ev before subject claim:alpha-holds
jqe before '(.verdict == "proven" or .verdict == "inputs_unchanged")
    and .supplementary.present == false and .supplementary.records == [] and .supplementary.weakened == []' \
  "the passing claim does not answer passing, beside an empty supplementary section"
BEFORE="$(jq -r '.verdict' "$W/before.json")"

# ---------------------------------------------------------------- 2. the runs, and one run
local_run fail failed
FAILED="$(cat "$W/failed.id")"
clean "a failing local recording"
ev runs1 runs
jqe runs1 '.present == true and (.runs | length == 1) and .runs[0].id == $id and .runs[0].commit == $h
    and .runs[0].working_tree == "clean" and .runs[0].outcome == "fail" and .runs[0].totals.executions == 1' \
  "the failing local run is not listed with its id, commit, tree, outcome and counts" \
  --arg id "$FAILED" --arg h "$HEAD1"
expect_exit 0 "$MJB" evidence --repo "$T" runs
expect_grep "^fail +$FAILED at "
ev one run "$FAILED"
jqe one '.run.id == $id and .record.id == $id and .record.executions[0].outcome == "fail"
    and .record.executions[0].test == "suite:901_alpha" and (.cases | type == "array")' \
  "the run is not shown alone by its id, with its executions" --arg id "$FAILED"
expect_exit 0 "$MJB" evidence --repo "$T" run "$FAILED"
expect_grep "^run +$FAILED \\(local\\)"
expect_grep '^outcome +fail'

rc=0; "$MJB" evidence --repo "$T" run local:000000000000:20260101T000000Z > "$W/unknown.out" 2> "$W/unknown.err" || rc=$?
[ "$rc" = 13 ] || { echo "    an unknown run exited $rc, not 13"; cat "$W/unknown.err"; exit 1; }
[ ! -s "$W/unknown.out" ] || { echo "    an unknown run printed an answer"; cat "$W/unknown.out"; exit 1; }
grep -qF 'majordomus evidence runs' "$W/unknown.err" \
  || { echo "    the refusal does not say where the runs are listed"; cat "$W/unknown.err"; exit 1; }

# ---------------------------------------------------------------- 3. the failing run weakens, named
ev weakened subject claim:alpha-holds
jqe weakened '.verdict == "stale" and .routes[0].state == "stale" and (.routes[0].detail | contains($id))' \
  "a clean failing local run did not cap the claim at stale, naming itself on the route" --arg id "$FAILED"
jqe weakened '.supplementary.weakened == [{test: "suite:901_alpha", from: $b, to: "stale", by: [$id],
    detail: .routes[0].detail}] and .supplementary.records[0].weakened == ["suite:901_alpha"]' \
  "the weakening is not listed beside the verdict with the record that made it" \
  --arg id "$FAILED" --arg b "$BEFORE"
expect_exit 0 "$MJB" evidence --repo "$T" subject claim:alpha-holds
expect_grep '^verdict +stale'
expect_grep "^weakened +suite:901_alpha $BEFORE -> stale by $FAILED"
ev feature subject feature:alpha
jqe feature '[.members[] | select(.subject == "claim:alpha-holds") | .state] == ["stale"]' \
  "the feature made of the claim does not read the weakened member"

# ---------------------------------------------------------------- 4. a newer pass raises nothing back
# a run's id carries the second it was recorded in, so the next one is recorded in another
sleep 1
local_run ok passed
PASSED="$(cat "$W/passed.id")"
clean "a passing local recording"
ev still subject claim:alpha-holds
jqe still '.verdict == "stale" and [.supplementary.weakened[].by[]] == [$f]
    and [.supplementary.records[] | {id, weakened}] == [{id: $p, weakened: []}, {id: $f, weakened: ["suite:901_alpha"]}]' \
  "a newer passing local run raised the verdict, or was named for a weakening" \
  --arg f "$FAILED" --arg p "$PASSED"

# ---------------------------------------------------------------- 5. a failing claim stays failing
measured fail
run_quiet "$W/record.err" "$MJB" evidence --repo "$T" record --suite "$W/run.tsv" \
  --provenance "suite=$W/psuite.json" > /dev/null
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm "a failing run"
clean "the failing recording"
local_run ok passed2
ev failing subject claim:alpha-holds
jqe failing '.verdict == "failing" and .routes[0].state == "failing" and .supplementary.weakened == []
    and (.supplementary.records | length == 3) and ([.supplementary.records[].weakened[]] == [])' \
  "a passing supplementary record raised a failing verdict, or weakened something"

# ---------------------------------------------------------------- 6. a source outside the checkout
expect_exit 13 "$MJB" evidence --repo "$T" subject claim:alpha-holds --from "$W"
expect_grep 'outside the repository'
expect_exit 13 "$MJB" evidence --repo "$T" runs --from "$W"
expect_grep 'outside the repository'
