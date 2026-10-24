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
<dl data-claims-proof><div><dt>guaranteed claims, declared</dt><dd data-declared="guaranteed">3</dd><dd data-supported="2"><span>2</span> supported by the recorded run of <a data-supported-commit="$RUN">0123456</a>, <span data-supported-behind="4">4 commit(s) behind</span> the one this page was built from</dd><dd>Declared, and naming the test that is to prove it.</dd></div></dl>
<span class="mj-badge mj-badge--not-run" data-claim="b">not run</span>
</section></main></body></html>
HTML
  printf '<html><head><title>evidence</title></head><body><dl><dd data-ci-supported="2">2 of 3</dd></dl></body></html>\n' > "$evpage"
}
# no run measured anything: the band and the evidence page both say so in words
unmeasured() {
  sed -i.bak 's#<dd data-supported="2">.*the one this page was built from</dd>#<dd data-supported="unmeasured"><span class="mj-badge mj-badge--unknown">unknown</span> supported by a recorded run: not yet measured</dd>#' "$home"
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
sed -i.bak 's#, <span data-supported-behind="4">4 commit(s) behind</span> the one this page was built from#, the commit this page was built from#' "$home"
expect_finding 0 'supported by the recorded run of 0123456789ab, the built commit' "a run at the built commit"
fresh; rm "$ev"; unmeasured
expect_finding 0 "with no recorded run measured, 'not yet measured' supported, never a number" "no evidence.json, as a local build has none"
fresh; jq -n '{schema: 1, available: false, reason: "no artifact", claims: {measured: false, declared: null, supported: null, reason: "no artifact"}}' > "$ev"; unmeasured
expect_finding 0 "'not yet measured' supported" "evidence unavailable"
fresh; jq '.claims = {measured: false, declared: null, supported: null, reason: "no verdict"}' "$ev" > "$ev.new" && mv "$ev.new" "$ev"; unmeasured
expect_finding 0 "'not yet measured' supported" "a run that counted nothing"

# --- a supported figure the run does not give
fresh; sed -i.bak 's#<dd data-supported="2"><span>2</span>#<dd data-supported="3"><span>3</span>#' "$home"
expect_finding 10 "the supported figure reads '3'.*the recorded run the evidence page reads says '2'" "an overstated supported figure"

# --- a bare 0 where no run measured anything, and the other ways an unmeasured figure lies
fresh; rm "$ev"
sed -i.bak 's#<dd data-supported="2">.*the one this page was built from</dd>#<dd data-supported="0"><span>0</span> supported by a recorded run</dd>#' "$home"
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
fresh; jq '.behind = 0' "$ev" > "$ev.new" && mv "$ev.new" "$ev"
expect_finding 10 'the supported figure says its run is 4 commit\(s\) behind, and it was recorded at the built commit' "a distance the run does not have"

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
grep -qF 'data-supported="{% if ci_measured %}{{ ci.claims.supported }}' "$TPL" || { echo "    the proof band does not read the recorded run's count"; exit 1; }
grep -qF 'model.evidence.supported' "$TPL" && { echo "    the homepage still reads the committed ledger's supported count"; exit 1; }
grep -qF 'data-ci-supported="{{ ci.claims.supported }}"' "$ROOT/site/templates/evidence.html" || { echo "    the evidence page does not show the recorded run's count"; exit 1; }
grep -A1 '^  - id: guaranteed$' "$ROOT/docs/CLAIMS.yaml" | grep -q 'test proves it' \
  && { echo "    docs/CLAIMS.yaml still says a test proves every guaranteed claim"; exit 1; }
exit 0
