# majordomus-covers: script:scripts/site-check
# A status on the site speaks the vocabulary of its kind, and the challenge counts its refusals
# only as the recorded run does.
#
# The site shows three statuses that look alike and mean different things: what
# docs/CLAIMS.yaml declares a claim to be (guaranteed, advisory, planned, rejected), what
# `majordomus evidence show` derived from the recorded runs for that claim, and what a feature
# is (feature/v1). partials/status-badge.html draws every one of them, naming its kind, and
# scripts/site-check (section 7c) holds each rendered badge to its own vocabulary through
# scripts/lib/status-vocabulary.awk. The same file refuses a count of refusals in the
# challenge's words that the recorded run does not give: "refuses twice" was written beside a
# run that refused three times.
#
# What is proved, against fixtures rather than this checkout's build: a clean page passes; a
# declared status outside the declaration, a proof badge that is not the derived state (or
# names no claim), and an unknown kind each fail by name; a stated count that disagrees fails
# in digits, in words and as once or twice, in the text and in a meta description, and one
# that agrees passes. Then the sources: the partial emits what the checker reads, site-check
# runs both modes, and the challenge's own words carry no count at all.
. "$ROOT/test/lib.sh"
AWK="$ROOT/scripts/lib/status-vocabulary.awk"
[ -f "$AWK" ] || { echo "    scripts/lib/status-vocabulary.awk is missing"; exit 1; }

printf 'claim\tguaranteed\nclaim\tadvisory\nclaim\tplanned\nclaim\trejected\nfeature\tstable\nfeature\tdraft\nproof\talpha\tnot-run\nproof\tbeta\tstale\n' > vocab.tsv
badges() { awk -v mode=badges -v vocab=vocab.tsv -f "$AWK" "$@" > out.txt; }
refusals() { awk -v mode=refusals -v want="$1" -f "$AWK" page.html > out.txt; }
clean_line() { grep -c '^FAIL' out.txt | tr -d ' '; }

# --- a clean page: every kind, each in its vocabulary
cat > clean.html <<'HTML'
<p><span class="mj-badge mj-badge--guaranteed" data-status="guaranteed" data-status-kind="claim">guaranteed</span>
<span class="mj-badge mj-badge--not-run" data-status="not-run" data-status-kind="proof" data-claim="alpha">not run</span>
<span class="mj-badge mj-badge--unknown" data-status="unknown" data-status-kind="proof" data-claim="gamma">unknown</span>
<span class="mj-badge mj-badge--stable" data-status="stable" data-status-kind="feature">stable</span>
<span class="mj-badge mj-badge--ok" data-status="ok">exit 0</span></p>
HTML
badges clean.html
[ "$(clean_line)" = 0 ] || { echo "    a clean page was refused:"; cat out.txt; exit 1; }
expect_grep '^COUNT	4$' out.txt

# --- each break is named
cat > bad.html <<'HTML'
<span class="mj-badge mj-badge--proven" data-status="proven" data-status-kind="claim">proven</span>
<span class="mj-badge mj-badge--proven" data-status="proven" data-status-kind="proof" data-claim="beta">proven</span>
<span class="mj-badge mj-badge--not-run" data-status="not-run" data-status-kind="proof" data-claim="">not run</span>
<span class="mj-badge mj-badge--inputs-unchanged" data-status="inputs-unchanged" data-status-kind="proof" data-claim="gamma">inputs unchanged</span>
<span class="mj-badge mj-badge--shipped" data-status="shipped" data-status-kind="feature">shipped</span>
<span class="mj-badge mj-badge--ok" data-status="ok" data-status-kind="verdict">ok</span>
HTML
badges bad.html
expect_grep "a claim badge says 'proven', which the claim vocabulary does not hold" out.txt
expect_grep "the proof badge of claim 'beta' says 'proven', and the evidence derived 'stale'" out.txt
expect_grep "a proof badge names no claim" out.txt
expect_grep "the proof badge of claim 'gamma' says 'inputs-unchanged', and the evidence derived 'unknown'" out.txt
expect_grep "a feature badge says 'shipped', which the feature vocabulary does not hold" out.txt
expect_grep "the status kind 'verdict'" out.txt
[ "$(clean_line)" = 6 ] || { echo "    expected six findings:"; cat out.txt; exit 1; }

# --- a stated refusal count must be the recorded run's
printf '<html><head><meta name="description" content="Watch it refuse it twice."></head><body><p>Majordomus refuses twice.</p><p>Two refusals.</p><p>It refused 2 times.</p><dl><dt>refusals</dt><dd>3</dd></dl><pre>finish: refused, 3 unmet</pre></body></html>\n' > page.html
refusals 3
expect_grep 'the page says "refuse it twice", and the recorded run counts 3' out.txt
expect_grep 'the page says "refuses twice", and the recorded run counts 3' out.txt
expect_grep 'the page says "two refusals", and the recorded run counts 3' out.txt
expect_grep 'the page says "refused 2 times", and the recorded run counts 3' out.txt
[ "$(clean_line)" = 4 ] || { echo "    expected four findings, and none for a figure or a verdict line:"; cat out.txt; exit 1; }
printf '<p>It refuses three times, three refusals in all.</p><dl><dt>refusals</dt><dd>3</dd></dl>\n' > page.html
refusals 3
[ "$(clean_line)" = 0 ] || { echo "    a count that agrees was refused:"; cat out.txt; exit 1; }
expect_grep '^COUNT	2$' out.txt

# --- the sources: what renders the badge, what checks it, and the words around the run
P="$ROOT/site/templates/partials/status-badge.html"
grep -q 'data-status-kind=' "$P" && grep -q 'data-status=' "$P" && grep -q 'data-claim=' "$P" \
  || { echo "    partials/status-badge.html does not emit the attributes the checker reads"; exit 1; }
grep -q 'mode=badges' "$ROOT/scripts/site-check" && grep -q 'mode=refusals' "$ROOT/scripts/site-check" \
  || { echo "    scripts/site-check does not run both modes of the vocabulary checker"; exit 1; }
cp "$ROOT/site/data/challenge.toml" page.html
refusals 999999
[ "$(grep -c '^FAIL' out.txt | tr -d ' ')" = 0 ] && expect_grep '^COUNT	0$' out.txt \
  || { echo "    site/data/challenge.toml states a refusal count; it is the recorded run's to state:"; cat out.txt; exit 1; }
cp "$ROOT/site/content-src/challenge.md" page.html
refusals 999999
expect_grep '^COUNT	0$' out.txt
