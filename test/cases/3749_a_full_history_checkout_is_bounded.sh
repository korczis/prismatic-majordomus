# majordomus-covers: none
# claims: none
# A checkout of the whole history carries a bound of its own, smaller than its job's, so a
# fetch that stalls fails by name instead of waiting out the job.
#
# On run 39438896504 the coverage job's `actions/checkout` with `fetch-depth: 0` fetched every
# head and every tag and did not return. It waited out the job's 55 minutes, GitHub reported the
# job cancelled, and nothing was measured. The same checkouts take 95 to 212 s on the other jobs
# of validate.yml. scripts/ci/step-bound-check refuses such a checkout without its own
# `timeout-minutes`, or with one that is not smaller than its job's; the coverage job no longer
# checks out the history at all, and fetches only the differential's base.
#
# And a checkout fetches only what its job's gates read (I3241). On 2026-10-28 the seven
# `fetch-depth: 0` checkouts fetched 2.85 GiB each, every head, tag and blob, and hit their bound
# six times (runs 40708407384, 40733537887); the blobless one of structure takes 18 s. With the CI
# model, step-bound-check refuses a `fetch-depth: 0` checkout in a job none of whose gates
# declares history_reads heads or tags, one without `filter: blob:none` where none reads blobs,
# and a `git fetch` of `refs/heads/*` (or of the tags) where none reads them.
. "$ROOT/test/lib.sh"

CHECK="$ROOT/scripts/ci/step-bound-check"
V="$ROOT/.github/workflows/validate.yml"
W="$ROOT/.github/workflows/pages.yml"
G="$ROOT/.ai/repo/ci/gates.yaml"
expect_file "$CHECK"

# --- the good shape: both workflows of this repository
expect_exit 0 "$CHECK" "$V"
expect_grep 'as is every full-history checkout'
expect_exit 0 "$CHECK" "$W"

# --- the broken shape of run 39438896504: an unbounded full-history checkout
cat > unbounded.yml <<'Y'
jobs:
  coverage:
    timeout-minutes: 55
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - run: true
Y
expect_exit 10 "$CHECK" unbounded.yml
expect_grep 'unbounded.yml:5  job coverage — the full-history checkout declares no timeout-minutes of its own'

# --- bounded, but not under its job: the job still kills it first
cat > atjob.yml <<'Y'
jobs:
  coverage:
    timeout-minutes: 55
    steps:
      - uses: actions/checkout@v4
        timeout-minutes: 55
        with:
          fetch-depth: 0
Y
expect_exit 10 "$CHECK" atjob.yml
expect_grep 'bound 55m is not smaller than the job bound 55m'

# --- and with no job bound at all there is nothing to be smaller than
cat > nojob.yml <<'Y'
jobs:
  coverage:
    steps:
      - uses: actions/checkout@v4
        timeout-minutes: 5
        with:
          fetch-depth: 0
Y
expect_exit 10 "$CHECK" nojob.yml
expect_grep 'declares no timeout-minutes, so its full-history checkout has nothing to be smaller than'

# --- a bounded full checkout, and a shallow one with no bound, are both fine
cat > fine.yml <<'Y'
jobs:
  structure:
    timeout-minutes: 55
    steps:
      - uses: actions/checkout@v4
        timeout-minutes: 10
        with:
          fetch-depth: 0
  plan:
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 2
Y
expect_exit 0 "$CHECK" fine.yml

# --- every full-history checkout of the real workflows is bounded under its job: the gate
#     above, held against a copy of validate.yml with each such bound removed
awk '/^      - uses: actions\/checkout@/ { c = 1; print; next }
     c && /^        timeout-minutes: / { c = 0; next }
     { c = 0; print }' "$V" > stripped.yml
expect_exit 10 "$CHECK" stripped.yml
expect_grep 'job structure — the full-history checkout'
expect_no_grep 'job coverage — '   # the coverage job checks out one commit, and needs no history

# --- the coverage job reads one commit, and fetches only the differential's base
awk '$0 == "  coverage:" { f = 1; next } /^  [a-z][a-z-]*:$/ { f = 0 } f' "$V" > coverage.yml
grep -q 'fetch-depth: 0' coverage.yml && { echo "    the coverage job checks out the whole history again: every head and every tag, for one base commit"; exit 1; }
grep -q 'git fetch --no-tags --depth=1 origin "$BASE"' coverage.yml \
  || { echo "    the coverage job does not fetch the pull request's base, so the differential would answer UNKNOWN"; exit 1; }

# ================================================================ what a checkout fetches (I3241)
# --- the real workflow against the real model
expect_exit 0 "$CHECK" --gates "$G" "$V"
expect_grep 'every checkout fetches only what its gates read'

# a model of one gate in job `gate`, its history_reads given as $1 (empty: none declared)
model() {
  { printf 'version: 1\ngates:\n  - id: g\n    job: gate\n    runs: true\n'
    [ -z "$1" ] || printf '    history_reads: [%s]\n' "$1"; } > model.yml
}
# a workflow whose job `gate` runs the gate, with the checkout lines and the steps given
workflow() {
  { printf 'jobs:\n  gate:\n    timeout-minutes: 30\n    steps:\n      - uses: actions/checkout@v4\n        timeout-minutes: 5\n'
    printf '%s' "$1"
    printf '      - run: scripts/ci/run-plan --job gate\n'
    printf '%s' "${2:-}"; } > flow.yml
}
FULL='        with:
          fetch-depth: 0
'
BLOBLESS='        with:
          fetch-depth: 0
          filter: blob:none
'

# --- a full checkout in a job whose gate declares no history_reads: both what it fetches
#     beyond the ancestry (every head and tag) and every blob are refused
model ""; workflow "$FULL"
expect_exit 10 "$CHECK" --gates model.yml flow.yml
expect_grep 'flow.yml:5  job gate — the checkout fetches every head and every tag \(fetch-depth: 0\), and no gate of the job declares history_reads heads or tags'
expect_grep 'job gate — the full-history checkout fetches every blob of the history \(no filter: blob:none\), and no gate of the job declares history_reads blobs'

# --- the gate declares that it reads heads: the heads are justified, the blobs still are not
model "heads"; workflow "$FULL"
expect_exit 10 "$CHECK" --gates model.yml flow.yml
expect_no_grep 'every head and every tag'
expect_grep 'fetches every blob of the history'

# --- blobless, the declaration is enough; and a gate that reads blobs in bulk may have them
model "heads"; workflow "$BLOBLESS"
expect_exit 0 "$CHECK" --gates model.yml flow.yml
model "tags, blobs"; workflow "$FULL"
expect_exit 0 "$CHECK" --gates model.yml flow.yml

# --- a blobless full checkout with nothing declared is still every head and every tag
model ""; workflow "$BLOBLESS"
expect_exit 10 "$CHECK" --gates model.yml flow.yml
expect_grep 'every head and every tag'
expect_no_grep 'fetches every blob'

# --- HEAD's ancestry needs no declaration: one blobless commit, then its ancestry
ANCESTRY='        with:
          filter: blob:none
      - timeout-minutes: 5
        run: git fetch --no-tags --unshallow origin "$GITHUB_SHA"
'
model ""; workflow "$ANCESTRY"
expect_exit 0 "$CHECK" --gates model.yml flow.yml

# --- a fetch of every branch needs a gate that reads other branches
BRANCHES='      - run: git fetch --no-tags origin "+refs/heads/*:refs/remotes/origin/*"
'
model ""; workflow "" "$BRANCHES"
expect_exit 10 "$CHECK" --gates model.yml flow.yml
expect_grep 'flow.yml:8  job gate — the step fetches every branch \(refs/heads/\*\), and no gate of the job declares history_reads heads'
model "tags"; workflow "" "$BRANCHES"
expect_exit 10 "$CHECK" --gates model.yml flow.yml
model "heads"; workflow "" "$BRANCHES"
expect_exit 0 "$CHECK" --gates model.yml flow.yml
# one branch, by name, is not every branch: the coverage job's fetch of master stays allowed
model ""; workflow "" '      - run: git fetch --no-tags origin +refs/heads/master:refs/remotes/origin/master
'
expect_exit 0 "$CHECK" --gates model.yml flow.yml
# a comment that names the refspec fetches nothing
model ""; workflow "" '      - run: |
          # not: git fetch origin "+refs/heads/*:refs/remotes/origin/*"
          true
'
expect_exit 0 "$CHECK" --gates model.yml flow.yml

# --- and the tags need a gate that reads the tags
model ""; workflow "" '      - run: git fetch --tags origin
'
expect_exit 10 "$CHECK" --gates model.yml flow.yml
expect_grep 'job gate — the step fetches the tags, and no gate of the job declares history_reads tags'
model "tags"; workflow "" '      - run: git fetch --tags origin
'
expect_exit 0 "$CHECK" --gates model.yml flow.yml
model ""; workflow '        with:
          fetch-tags: true
'
expect_exit 10 "$CHECK" --gates model.yml flow.yml
expect_grep 'the checkout fetches the tags \(fetch-tags: true\)'

# --- a job named <job>-shard runs the gates of <job>
model "heads"
cat > flow.yml <<Y
jobs:
  gate:
    timeout-minutes: 30
    steps:
      - run: scripts/ci/run-plan --job gate
  gate-shard:
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v4
        timeout-minutes: 5
$BLOBLESS
Y
expect_exit 0 "$CHECK" --gates model.yml flow.yml

# --- the real workflow, held to the real model: without the declarations, the jobs that
#     check out every head and tag for them are refused, by name
grep -v '^    history_reads:' "$G" > undeclared.yml
expect_exit 10 "$CHECK" --gates undeclared.yml "$V"
expect_grep 'job structure — the checkout fetches every head and every tag'
expect_grep 'job site — the checkout fetches every head and every tag'
expect_grep 'job suite-shard — the full-history checkout fetches every blob'
expect_no_grep 'job evidence — '   # the evidence job fetches HEAD's ancestry, which nothing declares

# --- and without its blob:none filters, every job whose gates read no blobs in bulk is refused,
#     while the suite, whose cases clone the checkout with --no-local, keeps its blobs
grep -v '^          filter: blob:none' "$V" > blobful.yml
expect_exit 10 "$CHECK" --gates "$G" blobful.yml
for j in structure structure-b site pages; do
  expect_grep "job $j — the full-history checkout fetches every blob"
done
expect_no_grep 'job (suite-shard|rust|macos) — '

# --- the model refuses a word history_reads does not know
sed 's/^    history_reads: \[heads\]$/    history_reads: [heads, branches]/' "$G" > unknown.yml
expect_exit 10 "$ROOT/scripts/ci-plan" --check --model unknown.yml
expect_grep 'history_reads names branches; it is heads, tags or blobs'

echo "    a full-history checkout is bounded under its job, the coverage job fetches one base, not the history, and every checkout fetches only what its gates read"
