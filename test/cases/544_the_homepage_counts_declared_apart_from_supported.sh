# majordomus-covers: none
# The homepage's proof band says how many guarantees are declared and, apart from that, how many
# the recorded run supports — the run the evidence page reads (site/data/evidence.json `claims`,
# written by scripts/pages evidence), named by the commit it was recorded at and how far that is
# behind the built one — or the words "not yet measured" and no number when no run measured it.
# It never says a test proves it while fewer are supported: scripts/ci/homepage-check, check
# `declared`, over a tree this case builds (I3916).
#
# What is proved: a clean tree passes with a run recorded behind the built commit, with one
# recorded at it, and with no run measured (no evidence.json, evidence unavailable, a run that
# counted nothing). Each of these fails, with a finding that names it — a supported figure the
# run does not give, a bare 0 where no run measured anything, a number in the words of an
# unmeasured figure, an unmeasured figure that does not say so, a figure that names another
# revision, one silent about how far its run is behind, one that says it is behind when it is
# not, an evidence page that counts another figure or none, a declared figure the matrix does
# not give, "a test proves it" while fewer are supported, and a homepage with no proof band.
# The real templates read the evidence dataset on both pages, the committed ledger's verdict no
# longer feeds the band, and the declared meaning of `guaranteed` does not say a test proves it.
#
# The run's own state (I4317): a run of the built commit that recorded failures (stale) or ran
# in a dirty tree (unknown), or a run behind the build with reasons of its own, confirms no
# count, so the band, /evidence/ and /guarantees/ say "not confirmed" and show no number; the
# served homepage of 81ef931 (a plain "232 supported" beside a stale run) fails, and so do a
# number in the withheld words, words that do not say so, a state attribute that is not the
# run's, an evidence page that still counts, and "a test proves it" beside such a run.
# /guarantees/ and its guaranteed page show the band's figure: the page as served at 81ef931
# (the ledger's 0 as "supported by a recorded run") fails, a second count that does not say it
# is the committed ledger's fails, and a figure that disagrees with the run fails on each page.
# The real templates read one definition of the figure's word (partials/run-figure.html) and
# draw one figure (partials/supported-figure.html) on the homepage and both guarantees pages.
# The evidence chain shows the same figure on its last node and that node's detail: a chain
# showing a count beside a stale run (the review of 3d0e11d), one whose attribute reads the
# count again, a number beside its withheld word, a state that is not the run's and a missing
# detail each fail. Every claim's page under /guarantees/ and /features/matrix/ show the
# committed ledger's verdict and say so: "not supported by a recorded run" on a claim page and
# "0/9 claims supported by a recorded run" in the matrix fail, as each page was served.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }

F="$PWD/fixture"
home="$F/site/public/index.html"
evpage="$F/site/public/evidence/index.html"
ev="$F/site/data/evidence.json"
RUN="0123456789abcdef0123456789abcdef01234567"
fresh() {
  rm -rf "$F"; mkdir -p "$F/site/data/registry" "$F/site/data/generated" "$F/docs/generated" "$F/site/public/evidence"
  printf 'order = ["hero"]\n' > "$F/site/data/homepage.toml"
  printf '[[groups]]\nlabel = "Home"\nhref = "/"\n' > "$F/site/data/nav.toml"
  printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$F/docs/generated/web.json"
  printf '%s\n' '{}' > "$F/site/data/registry/distribution.json"
  printf '%s\n' '{"states":{"guaranteed":"neutral","inputs-unchanged":"info","not-run":"neutral","unknown":"neutral"}}' > "$F/site/data/registry/design.json"
  printf '%s\n' '{"status_order":["guaranteed"],"claims":[{"id":"a","status":"guaranteed"},{"id":"b","status":"guaranteed"},{"id":"c","status":"guaranteed"}]}' > "$F/site/data/generated/capabilities.json"
  # the committed ledger's verdict says 0: it no longer decides the band
  printf '%s\n' '{"features":[],"providers":[],"evidence":{"available":true,"declared":3,"supported":0,"claims":{"a":{"state":"stale","supported":false},"b":{"state":"not-run","supported":false},"c":{"state":"not-run","supported":false}}}}' > "$F/site/data/registry/product.json"
  # the run the deployment published: 2 of 3 supported, recorded 4 commits behind the build
  jq -n --arg c "$RUN" '{schema: 1, available: true, commit: $c, state: "stale", behind: 4, changed: 9,
    claims: {measured: true, declared: 3, supported: 2, reason: null}}' > "$ev"
  cat > "$home" <<HTML
<html><head><title>home</title></head><body><main><section id="hero">
<dl data-claims-proof><div><dt>guaranteed claims, declared</dt><dd data-declared="guaranteed">3</dd><dd data-supported="2" data-supported-state="stale"><span>2</span> supported by the recorded run of <a data-supported-commit="$RUN">0123456</a>, <span data-supported-behind="4">4 commit(s) behind</span> the one this page was built from</dd><dd>Declared, and naming the test that is to prove it.</dd></div></dl>
<span class="mj-badge mj-badge--not-run" data-claim="b">not run</span>
</section></main></body></html>
HTML
  printf '<html><head><title>evidence</title></head><body><dl><dd data-ci-supported="2">2 of 3</dd></dl></body></html>\n' > "$evpage"
}
# no run measured anything: the band and the evidence page both say so in words
unmeasured() {
  sed -i.bak 's#<dd data-supported="2" data-supported-state="stale">.*the one this page was built from</dd>#<dd data-supported="unmeasured" data-supported-state="unmeasured"><span class="mj-badge mj-badge--unknown">unknown</span> supported by a recorded run: not yet measured</dd>#' "$home"
  sed -i.bak 's#<dd data-ci-supported="2">2 of 3</dd>#<dd data-ci-supported="unmeasured">not yet measured</dd>#' "$evpage"
}
run() { MJ_ROOT="$F" "$CHECK" --only declared > out.txt 2>&1; echo $?; }
expect_finding() { # <exit> <pattern> <what>
  rc="$(run)"
  [ "$rc" = "$1" ] || { echo "    $3: exit $rc, expected $1"; cat out.txt; exit 1; }
  grep -qE "$2" out.txt || { echo "    $3: no finding matching '$2'"; cat out.txt; exit 1; }
}

# --- clean: a run behind the build, a run at it, and no run measured in three ways
fresh
expect_finding 0 '^OK   declared +the proof band says 3 declared guaranteed and 2 supported by the recorded run of 0123456789ab, 4 commit\(s\) behind the built one' "a run behind the build"
fresh; jq '.behind = 0 | .state = "current"' "$ev" > "$ev.new" && mv "$ev.new" "$ev"
sed -i.bak -e 's#, <span data-supported-behind="4">4 commit(s) behind</span> the one this page was built from#, the commit this page was built from#' -e 's#data-supported-state="stale"#data-supported-state="current"#' "$home"
expect_finding 0 'supported by the recorded run of 0123456789ab, the built commit' "a run at the built commit"
fresh; rm "$ev"; unmeasured
expect_finding 0 "with no recorded run measured, 'not yet measured' supported, never a number" "no evidence.json, as a local build has none"
fresh; jq -n '{schema: 1, available: false, reason: "no artifact", claims: {measured: false, declared: null, supported: null, reason: "no artifact"}}' > "$ev"; unmeasured
expect_finding 0 "'not yet measured' supported" "evidence unavailable"
fresh; jq '.claims = {measured: false, declared: null, supported: null, reason: "no verdict"}' "$ev" > "$ev.new" && mv "$ev.new" "$ev"; unmeasured
expect_finding 0 "'not yet measured' supported" "a run that counted nothing"

# --- a supported figure the run does not give
fresh; sed -i.bak 's#<dd data-supported="2" data-supported-state="stale"><span>2</span>#<dd data-supported="3" data-supported-state="stale"><span>3</span>#' "$home"
expect_finding 10 "the supported figure reads '3'.*the recorded run the evidence page reads says '2'" "an overstated supported figure"

# --- a bare 0 where no run measured anything, and the other ways an unmeasured figure lies
fresh; rm "$ev"
sed -i.bak 's#<dd data-supported="2" data-supported-state="stale">.*the one this page was built from</dd>#<dd data-supported="0"><span>0</span> supported by a recorded run</dd>#' "$home"
sed -i.bak 's#<dd data-ci-supported="2">2 of 3</dd>#<dd data-ci-supported="unmeasured">not yet measured</dd>#' "$evpage"
expect_finding 10 "the supported figure reads '0' \('0 supported by a recorded run'\).*says 'unmeasured'" "a bare 0 with no recorded run"
fresh; rm "$ev"; unmeasured; sed -i.bak 's#not yet measured</dd>#not yet measured, 0 of them</dd>#' "$home"
expect_finding 10 'no recorded run measured the guaranteed claims, and the supported figure shows a number' "a number in an unmeasured figure's words"
fresh; rm "$ev"; unmeasured; sed -i.bak 's#supported by a recorded run: not yet measured#supported by a recorded run: unknown#' "$home"
expect_finding 10 "the supported figure does not say 'not yet measured'" "an unmeasured figure that does not say so"
fresh; jq '.available = false' "$ev" > "$ev.new" && mv "$ev.new" "$ev"
expect_finding 10 "the supported figure reads '2'.*says 'unmeasured'" "a known figure when the evidence is unavailable"

# --- the revision it was recorded at, and its distance from the built one
fresh; sed -i.bak "s#data-supported-commit=\"$RUN\"#data-supported-commit=\"fedcba9876543210fedcba9876543210fedcba98\"#" "$home"
expect_finding 10 "the supported figure names the revision 'fedcba987654', and its run was recorded at '0123456789ab'" "a figure naming another revision"
fresh; sed -i.bak 's#<a data-supported-commit="[0-9a-f]*">0123456</a>#a run#' "$home"
expect_finding 10 "the supported figure names the revision 'nowhere'" "a figure naming no revision"
fresh; sed -i.bak 's#, <span data-supported-behind="4">4 commit(s) behind</span> the one this page was built from##' "$home"
expect_finding 10 'the run behind the supported figure is 4 commit\(s\) behind the built one, and the figure says nothing of it' "a figure silent about its distance"
fresh; jq '.behind = 0 | .state = "current" | .unconfirmed = []' "$ev" > "$ev.new" && mv "$ev.new" "$ev"
sed -i.bak 's#data-supported-state="stale"#data-supported-state="current"#' "$home"
expect_finding 10 'the supported figure says its run is 4 commit\(s\) behind, and it was recorded at the built commit' "a distance the run does not have"

# --- the run's own state (I4317). Evidence from 2026-10-28 on https://majordomus.dev, serving
#     81ef931: the band printed '<dd data-supported="232"><span>232</span> supported by the
#     recorded run of 81ef931, the commit this page was built from' while /evidence/ said the
#     run "recorded failures, so it does not confirm the report below. A job that ran tests ran
#     them in a dirty tree." A run that does not confirm its report confirms no count.
DIRTY="A job that ran tests ran them in a dirty tree."
FAILED="The run recorded 3 failed, timed-out or errored execution(s)."
FAILED_RE='The run recorded 3 failed, timed-out or errored execution\(s\)\.'
at_built() { # <state> <reason>...: the run is of the built commit, and withheld for its reasons
  st="$1"; shift
  jq --arg st "$st" '.behind = 0 | .changed = 0 | .state = $st | .unconfirmed = $ARGS.positional' "$ev" --args "$@" > "$ev.new" && mv "$ev.new" "$ev"
}
# the homepage of 81ef931, as it was served: a plain count beside a run that confirms none
plain_count() {
  sed -i.bak -e 's#, <span data-supported-behind="4">4 commit(s) behind</span> the one this page was built from#, the commit this page was built from#' -e "s#data-supported-state=\"stale\"#data-supported-state=\"$1\"#" "$home"
}
# the band as partials/supported-figure.html draws a withheld count, and /evidence/ beside it
not_confirmed() { # <state> <words>
  sed -i.bak "s#<dd data-supported=\"2\" data-supported-state=\"stale\">.*the one this page was built from</dd>#<dd data-supported=\"unconfirmed\" data-supported-state=\"$1\"><span class=\"mj-badge mj-badge--$1\">$1</span> supported by a recorded run: not confirmed. $2</dd>#" "$home"
  sed -i.bak 's#<dd data-ci-supported="2">2 of 3</dd>#<dd data-ci-supported="unconfirmed">not confirmed (the run counted 2 of 3)</dd>#' "$evpage"
}
STALE_WORDS="CI's run of <a data-supported-commit=\"$RUN\">0123456</a>, the commit this page was built from, recorded failures, so it does not confirm a count"
fresh; at_built stale "$DIRTY" "$FAILED"; plain_count stale
expect_finding 10 "the supported figure reads '2' .*the recorded run of 0123456789ab is stale, at the built commit: $DIRTY $FAILED_RE, so it confirms no count; it must say 'not confirmed'" "a plain count beside a stale run at the built commit (81ef931)"
fresh; at_built stale "$DIRTY" "$FAILED"; not_confirmed stale "$STALE_WORDS"
expect_finding 0 "^OK   declared .*the recorded run of 0123456789ab is stale, at the built commit: .*, 'not confirmed' supported, never a count, the word the evidence page reads" "a stale run at the built commit, said as not confirmed"
fresh; at_built unknown "$DIRTY"; plain_count unknown
expect_finding 10 "the supported figure reads '2' .*is unknown, at the built commit: $DIRTY, so it confirms no count" "a plain count beside a run that ran in a dirty tree"
fresh; at_built unknown "$DIRTY"; not_confirmed unknown "CI ran <a data-supported-commit=\"$RUN\">0123456</a>, the commit this page was built from, but its run cannot confirm a count"
expect_finding 0 "'not confirmed' supported, never a count" "a dirty run, said as not confirmed"
fresh; jq --arg r "$FAILED" '.unconfirmed = [$r]' "$ev" > "$ev.new" && mv "$ev.new" "$ev"
expect_finding 10 "the supported figure reads '2' .*is stale, 4 commit\(s\) behind the built one: $FAILED_RE, so it confirms no count" "a count beside a run behind the build that recorded failures"
fresh; at_built stale "$FAILED"; not_confirmed stale "$STALE_WORDS, 2 of them"
expect_finding 10 'confirms no count, and the supported figure shows a number' "a number in a withheld figure's words"
fresh; at_built stale "$FAILED"; not_confirmed stale "$STALE_WORDS"; sed -i.bak 's#supported by a recorded run: not confirmed. #supported by a recorded run: #' "$home"
expect_finding 10 "confirms no count, and the supported figure does not say 'not confirmed'" "a withheld figure that does not say so"
fresh; at_built stale "$FAILED"; not_confirmed stale "$STALE_WORDS"; sed -i.bak 's#data-supported-state="stale"#data-supported-state="current"#' "$home"
expect_finding 10 "the supported figure says its run is 'current' \(data-supported-state\), and the run's state is 'stale'" "a figure calling a stale run current"
fresh; at_built stale "$FAILED"; not_confirmed stale "$STALE_WORDS"; sed -i.bak 's#<dd data-ci-supported="unconfirmed">not confirmed (the run counted 2 of 3)</dd>#<dd data-ci-supported="2">2 of 3</dd>#' "$evpage"
expect_finding 10 "the evidence page counts '2' supported guaranteed claim\(s\), and the homepage's figure is 'unconfirmed'" "an evidence page counting what its run does not confirm"
fresh; at_built stale "$FAILED"; not_confirmed stale "$STALE_WORDS"; sed -i.bak 's#Declared, and naming the test that is to prove it.#Implemented, and a behavioural test proves it.#' "$home"
expect_finding 10 'the homepage says a test proves it, and a recorded run supports an unconfirmed number of 3' "the proof overstated beside a stale run"

# --- /guarantees/ shows the homepage's figure, and any other count says it is the ledger's
gpage="$F/site/public/guarantees/index.html"
gstatus="$F/site/public/guarantees/guaranteed/index.html"
guarantees() { # <figure element> [<second figure>]: both guarantees pages
  mkdir -p "$(dirname "$gstatus")"
  for p in "$gpage" "$gstatus"; do
    printf '<html><head><title>guarantees</title></head><body><a href="/guarantees/guaranteed/">%s%s</a></body></html>\n' "$1" "${2:-}" > "$p"
  done
}
LEDGER='<p data-ledger-supported="0"><span>0</span> supported by the runs the committed ledger records, the verdict each row below shows</p>'
fresh; guarantees "<p data-supported=\"2\" data-supported-state=\"stale\"><span>2</span> supported by the recorded run of <span data-supported-commit=\"$RUN\">0123456</span>, <span data-supported-behind=\"4\">4 commit(s) behind</span> the one this page was built from</p>" "$LEDGER"
expect_finding 0 '^OK   declared .*the count the evidence page reads and /guarantees/ shows' "/guarantees/ with the homepage's figure and the ledger's, each named"
fresh; at_built stale "$DIRTY" "$FAILED"; not_confirmed stale "$STALE_WORDS"
guarantees "<p data-supported=\"unconfirmed\" data-supported-state=\"stale\">supported by a recorded run: not confirmed. CI's run of <span data-supported-commit=\"$RUN\">0123456</span>, the commit this page was built from, recorded failures, so it does not confirm a count</p>" "$LEDGER"
expect_finding 0 "'not confirmed' supported, never a count, the word the evidence page reads and /guarantees/ shows" "/guarantees/ beside a stale run, said as not confirmed"
fresh; guarantees '<p><span>0</span> supported by a recorded run</p>'
expect_finding 10 '/guarantees/ does not say how many guaranteed claims a recorded run supports' "/guarantees/ as served at 81ef931: the ledger's count as the run's"
fresh; guarantees "<p data-supported=\"2\" data-supported-state=\"stale\"><span>2</span> supported by the recorded run of <span data-supported-commit=\"$RUN\">0123456</span>, <span data-supported-behind=\"4\">4 commit(s) behind</span> the one this page was built from</p>" '<p><span>0</span> supported by a recorded run</p>'
expect_finding 10 "/guarantees/: counts 0 supported by a recorded run apart from the run's figure \('2'\)" "/guarantees/ with a second count that does not say it is the ledger's"
fresh; guarantees "<p data-supported=\"2\" data-supported-state=\"stale\"><span>2</span> supported by the recorded run of <span data-supported-commit=\"$RUN\">0123456</span>, <span data-supported-behind=\"4\">4 commit(s) behind</span> the one this page was built from</p>" '<p data-ledger-supported="0"><span>0</span> supported</p>'
expect_finding 10 "/guarantees/: a second supported figure \('0 supported'\) does not say it counts the committed ledger" "a ledger figure that does not name the ledger"
fresh; at_built stale "$FAILED"; not_confirmed stale "$STALE_WORDS"
guarantees "<p data-supported=\"2\" data-supported-state=\"stale\"><span>2</span> supported by the recorded run of <span data-supported-commit=\"$RUN\">0123456</span>, the commit this page was built from</p>" "$LEDGER"
expect_finding 10 "/guarantees/: the supported figure reads '2' .*so it confirms no count" "/guarantees/ counting what a stale run does not confirm"
expect_finding 10 "/guarantees/guaranteed/: the supported figure reads '2'" "the guaranteed page counting it too"

# --- the homepage's evidence chain shows the run's figure a second time, on its last node and in
#     that node's detail (partials/evidence-chain.html); each value is the band's figure. A
#     review on 2026-10-28 replaced both chain values of a build beside a stale run with '232'
#     and `declared` still passed: the chain was read by nothing.
chain() { # <data-supported> <state> <words> <unit>: the chain's evidence node and its detail
  sed -i.bak "s#</section>#<figure class=\"x\" data-chain x-data=\"{}\"><button type=\"button\" data-chain-node=\"evidence\"><span data-supported=\"$1\" data-supported-state=\"$2\">$3</span><span>supported by the recorded run</span></button><div id=\"chain-detail-evidence\" data-chain-detail=\"evidence\"><p>supported by the recorded run</p><p data-supported=\"$1\" data-supported-state=\"$2\">$3</p><p>of 3 guaranteed$4</p></div></figure></section>#" "$home"
}
fresh; chain 2 stale 2 " · recorded at 0123456, stale"
expect_finding 0 '^OK   declared .*2 supported by the recorded run of 0123456789ab' "a chain that shows the band's count of a run behind the build"
fresh; at_built stale "$DIRTY" "$FAILED"; not_confirmed stale "$STALE_WORDS"; chain unconfirmed stale "not confirmed" " · the run of 0123456 is stale"
expect_finding 0 "'not confirmed' supported, never a count" "a chain beside a stale run, said as not confirmed"
fresh; at_built stale "$DIRTY" "$FAILED"; not_confirmed stale "$STALE_WORDS"; chain unconfirmed stale 2 " · the run of 0123456 is stale"
expect_finding 10 "the evidence chain's node 'supported by the recorded run' shows '2', and the run's figure is 'not confirmed': the recorded run of 0123456789ab is stale" "the reviewed defect: a chain showing a count beside a stale run"
expect_finding 10 "the evidence chain's detail 'supported by the recorded run' shows '2'" "the chain's detail showing it too"
fresh; at_built stale "$DIRTY" "$FAILED"; not_confirmed stale "$STALE_WORDS"; chain 2 stale 2 " · recorded at 0123456"
expect_finding 10 "the evidence chain's node 'supported by the recorded run' reads '2' \('2'\), and the recorded run the evidence page reads says 'unconfirmed'" "a chain whose template reads the count again"
fresh; at_built stale "$DIRTY" "$FAILED"; not_confirmed stale "$STALE_WORDS"; chain unconfirmed stale "not confirmed" " · the run of 0123456 is stale, 2 counted"
expect_finding 10 "the evidence chain's detail 'supported by the recorded run' shows a number beside the run's 'not confirmed'" "a count beside the chain's withheld word"
fresh; at_built stale "$DIRTY" "$FAILED"; not_confirmed stale "$STALE_WORDS"; chain unconfirmed current "not confirmed" " · the run of 0123456 is stale"
expect_finding 10 "the evidence chain's node 'supported by the recorded run' says its run is 'current' \(data-supported-state\), and the run's state is 'stale'" "a chain calling a stale run current"
fresh; rm "$ev"; unmeasured; chain unmeasured unmeasured unmeasured ""
expect_finding 0 "'not yet measured' supported, never a number" "a chain with no run measured"
fresh; chain 2 stale 2 " · recorded at 0123456, stale"; sed -i.bak 's# data-chain-detail="evidence"##' "$home"
expect_finding 10 "the evidence chain's detail 'supported by the recorded run' is missing" "a chain without the evidence detail"

# --- every page that shows the committed ledger's verdict says it is the ledger's: each claim's
#     page under /guarantees/ and /features/matrix/. Evidence from the same review: with a
#     current run, the homepage printed '232 supported by the recorded run of 0123456, the
#     commit this page was built from' while /guarantees/ci-verdict/ printed 'not supported by
#     a recorded run' and /features/matrix/ '0/N claims supported by a recorded run' on every row.
cpage="$F/site/public/guarantees/ci-verdict/index.html"
mpage="$F/site/public/features/matrix/index.html"
claim_page() { mkdir -p "$(dirname "$cpage")"; printf '<html><head><title>claim</title></head><body><p><span class="mj-badge" data-claim="a">stale</span><span data-ledger-supported="unsupported">%s</span></p></body></html>\n' "$1" > "$cpage"; }
matrix_page() { mkdir -p "$(dirname "$mpage")"; printf '<html><head><title>matrix</title></head><body><table><tr><td data-ledger-supported="f">0/9<span class="sr-only"> %s</span></td></tr></table></body></html>\n' "$1" > "$mpage"; }
fresh; claim_page "Declared guaranteed, and not supported by the runs the committed ledger records: the test is named, and the ledger holds no passing run of the current inputs."
matrix_page "claims supported by the runs the committed ledger records"
expect_finding 0 '^OK   declared ' "a claim page and the matrix that name the ledger"
fresh; claim_page "Declared guaranteed, and not supported by a recorded run: the test is named, and the ledger holds no passing run of the current inputs."
expect_finding 10 "/guarantees/ci-verdict/: says 'not supported by a recorded run' apart from the run's figure .*the committed ledger's verdict, not the recorded run's" "a claim page as served beside the homepage's count"
fresh; claim_page "Supported by a recorded run whose inputs this claim names have not changed since."
expect_finding 10 "/guarantees/ci-verdict/: says 'Supported by a recorded run'" "a claim page calling the ledger's verdict a run's"
fresh; matrix_page "claims supported by a recorded run"
expect_finding 10 "/features/matrix/: a second supported figure \('0/9 claims supported by a recorded run'\) does not say it counts the committed ledger" "the matrix as served beside the homepage's count"
fresh; matrix_page "claims supported"; sed -i.bak 's# data-ledger-supported="f"##' "$mpage"; sed -i.bak 's#claims supported</span>#claims supported by a recorded run</span>#' "$mpage"
expect_finding 10 "/features/matrix/: counts 0/9 supported by a recorded run apart from the run's figure" "a matrix count that is not marked as the ledger's"

# --- the evidence page reads the same run, and says the same count
fresh; sed -i.bak 's#data-ci-supported="2"#data-ci-supported="0"#' "$evpage"
expect_finding 10 "the evidence page counts '0' supported guaranteed claim\(s\), and the homepage's figure is '2'" "an evidence page that disagrees"
fresh; sed -i.bak 's# data-ci-supported="2"##' "$evpage"
expect_finding 10 'the evidence page does not say how many guaranteed claims the recorded run supports' "an evidence page silent about the count"

# --- a declared figure the matrix does not give
fresh; sed -i.bak 's#data-declared="guaranteed">3#data-declared="guaranteed">4#' "$home"
expect_finding 10 "the declared guaranteed figure is '4', and docs/CLAIMS.yaml declares 3" "a wrong declared figure"

# --- "a test proves it" while fewer are supported, measured or not
fresh; sed -i.bak 's#Declared, and naming the test that is to prove it.#Implemented, and a behavioural test proves it.#' "$home"
expect_finding 10 'the homepage says a test proves it, and a recorded run supports 2 of 3' "the proof overstated in prose"
fresh; rm "$ev"; unmeasured; sed -i.bak 's#Declared, and naming the test that is to prove it.#Implemented, and a behavioural test proves it.#' "$home"
expect_finding 10 'the homepage says a test proves it, and a recorded run supports an unmeasured number of 3' "the proof overstated with no run"

# --- no proof band at all
fresh; printf '<html><head><title>home</title></head><body><main><section id="hero">x</section></main></body></html>\n' > "$home"
expect_finding 10 'the homepage has no claims proof band' "no band"

# --- the real sources: both pages read the recorded run's count from the evidence dataset, the
#     committed ledger's verdict no longer feeds the band, and the declared meaning of
#     `guaranteed` does not say that a test proves it
TPL="$ROOT/site/templates/index.html"
grep -q 'data-claims-proof' "$TPL" || { echo "    site/templates/index.html has no proof band"; exit 1; }
grep -qF 'load_data(path="data/evidence.json", required=false)' "$TPL" || { echo "    the homepage does not read the evidence the deployment published"; exit 1; }
grep -qF '{% include "partials/supported-figure.html" %}' "$TPL" || { echo "    the proof band does not draw the shared supported figure"; exit 1; }
FIG="$ROOT/site/templates/partials/supported-figure.html"
grep -qF 'data-supported="{{ sf_value }}" data-supported-state="{{ sf_state }}"' "$FIG" || { echo "    the shared figure does not carry the run's count and state"; exit 1; }
grep -qF 'not confirmed' "$FIG" || { echo "    the shared figure has no words for a count its run does not confirm"; exit 1; }
for t in index.html evidence.html guarantees.html status.html; do
  grep -qF '{% include "partials/run-figure.html" %}' "$ROOT/site/templates/$t" || { echo "    site/templates/$t does not read the one definition of what the run's count may be shown as"; exit 1; }
done
for t in guarantees.html status.html; do
  grep -qF '{% include "partials/supported-figure.html" %}' "$ROOT/site/templates/$t" || { echo "    site/templates/$t does not show the homepage's supported figure"; exit 1; }
  grep -qF 'data-ledger-supported=' "$ROOT/site/templates/$t" || { echo "    site/templates/$t shows the ledger's count without naming it"; exit 1; }
done
RF="$ROOT/site/templates/partials/run-figure.html"
grep -qF 'rf_reasons == 0 and rf_state == "current"' "$RF" || { echo "    a run that does not confirm its report may show its count as current"; exit 1; }
grep -qF 'model.evidence.supported' "$TPL" && { echo "    the homepage still reads the committed ledger's supported count"; exit 1; }
grep -qF 'data-ci-supported="{{ ci.claims.supported }}"' "$ROOT/site/templates/evidence.html" || { echo "    the evidence page does not show the recorded run's count"; exit 1; }
CHAIN="$ROOT/site/templates/partials/evidence-chain.html"
[ "$(grep -cF 'data-supported="{{ chain_sv }}" data-supported-state="{{ chain_ss }}"' "$CHAIN")" -eq 2 ] \
  || { echo "    the evidence chain's node and detail do not both carry the run's figure"; exit 1; }
for t in claim.html features-matrix.html; do
  grep -qF 'data-ledger-supported=' "$ROOT/site/templates/$t" || { echo "    site/templates/$t shows the ledger's verdict without naming it"; exit 1; }
  grep -qiE 'supported by (a|the) recorded run' "$ROOT/site/templates/$t" \
    && { echo "    site/templates/$t still calls the committed ledger's verdict a recorded run's"; exit 1; }
done
grep -A1 '^  - id: guaranteed$' "$ROOT/docs/CLAIMS.yaml" | grep -q 'test proves it' \
  && { echo "    docs/CLAIMS.yaml still says a test proves every guaranteed claim"; exit 1; }
exit 0
