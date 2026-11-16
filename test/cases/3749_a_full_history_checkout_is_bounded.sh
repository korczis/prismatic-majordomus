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
. "$ROOT/test/lib.sh"

CHECK="$ROOT/scripts/ci/step-bound-check"
V="$ROOT/.github/workflows/validate.yml"
W="$ROOT/.github/workflows/pages.yml"
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

echo "    a full-history checkout is bounded under its job, and the coverage job fetches one base, not the history"
