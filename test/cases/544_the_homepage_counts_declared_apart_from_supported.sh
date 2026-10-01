# majordomus-covers: none
# The homepage's proof band says how many guarantees are declared and, apart from that, how many
# a recorded run supports — or `unknown` when no evidence is available — and never that a test
# proves them while fewer are supported: scripts/ci/homepage-check, check `declared`, over a tree
# this case builds.
#
# What is proved: a clean tree passes, with evidence and without it (`unknown`); each of these
# fails, with a finding that names it — a supported figure the evidence does not give, a figure
# that reads as known when no evidence is available, a declared figure the matrix does not give,
# "a test proves it" while the evidence supports fewer claims than are declared, and a homepage
# with no proof band. The real homepage template carries the band, and the declared meaning of
# `guaranteed` in docs/CLAIMS.yaml no longer says a test proves it.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }

F="$PWD/fixture"
home="$F/site/public/index.html"
product="$F/site/data/registry/product.json"
fresh() {
  rm -rf "$F"; mkdir -p "$F/site/data/registry" "$F/site/data/generated" "$F/docs/generated" "$F/site/public"
  printf 'order = ["hero"]\n' > "$F/site/data/homepage.toml"
  printf '[[groups]]\nlabel = "Home"\nhref = "/"\n' > "$F/site/data/nav.toml"
  printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$F/docs/generated/web.json"
  printf '%s\n' '{}' > "$F/site/data/registry/distribution.json"
  printf '%s\n' '{"states":{"guaranteed":"neutral","inputs-unchanged":"info","not-run":"neutral","unknown":"neutral"}}' > "$F/site/data/registry/design.json"
  printf '%s\n' '{"status_order":["guaranteed"],"claims":[{"id":"a","status":"guaranteed"},{"id":"b","status":"guaranteed"},{"id":"c","status":"guaranteed"}]}' > "$F/site/data/generated/capabilities.json"
  printf '%s\n' '{"features":[],"providers":[],"evidence":{"available":true,"declared":3,"supported":1,"claims":{"a":{"state":"inputs-unchanged","supported":true},"b":{"state":"not-run","supported":false},"c":{"state":"not-run","supported":false}}}}' > "$product"
  cat > "$home" <<'HTML'
<html><head><title>home</title></head><body><main><section id="hero">
<dl data-claims-proof><div><dt>guaranteed claims, declared</dt><dd data-declared="guaranteed">3</dd><dd data-supported="1"><span>1</span> supported by a recorded run</dd><dd>Declared, and naming the test that is to prove it.</dd></div></dl>
<span class="mj-badge mj-badge--not-run" data-claim="b">not run</span>
</section></main></body></html>
HTML
}
run() { MJ_ROOT="$F" "$CHECK" --only declared > out.txt 2>&1; echo $?; }
expect_finding() { # <exit> <pattern> <what>
  rc="$(run)"
  [ "$rc" = "$1" ] || { echo "    $3: exit $rc, expected $1"; cat out.txt; exit 1; }
  grep -qE "$2" out.txt || { echo "    $3: no finding matching '$2'"; cat out.txt; exit 1; }
}

# --- clean, with evidence, and without it
fresh
expect_finding 0 '^OK   declared +the proof band says 3 declared guaranteed and 1 supported' "a clean tree"
fresh; sed -i.bak 's/"available":true/"available":false/' "$product"
sed -i.bak 's#<dd data-supported="1"><span>1</span> supported by a recorded run</dd>#<dd data-supported="unknown">support by a recorded run: unknown</dd>#' "$home"
expect_finding 0 'and unknown supported by a recorded run' "no evidence, said as unknown"

# --- a supported figure the evidence does not give
fresh; sed -i.bak 's#<dd data-supported="1"><span>1</span>#<dd data-supported="3"><span>3</span>#' "$home"
expect_finding 10 "the supported figure reads '3'.*the evidence says '1'" "an overstated supported figure"

# --- a figure that reads as known when no evidence is available
fresh; sed -i.bak 's/"available":true/"available":false/' "$product"
expect_finding 10 "the supported figure reads '1'.*the evidence says 'unknown'" "a known figure without evidence"

# --- a declared figure the matrix does not give
fresh; sed -i.bak 's#data-declared="guaranteed">3#data-declared="guaranteed">4#' "$home"
expect_finding 10 "the declared guaranteed figure is '4', and docs/CLAIMS.yaml declares 3" "a wrong declared figure"

# --- "a test proves it" while fewer are supported than declared
fresh; sed -i.bak 's#Declared, and naming the test that is to prove it.#Implemented, and a behavioural test proves it.#' "$home"
expect_finding 10 'the homepage says a test proves it, and a recorded run supports 1 of 3' "the proof overstated in prose"

# --- no proof band at all
fresh; printf '<html><head><title>home</title></head><body><main><section id="hero">x</section></main></body></html>\n' > "$home"
expect_finding 10 'the homepage has no claims proof band' "no band"

# --- the real sources: the template renders the band from the evidence dataset, and the
#     declared meaning of `guaranteed` does not say that a test proves it
grep -q 'data-claims-proof' "$ROOT/site/templates/index.html" || { echo "    site/templates/index.html has no proof band"; exit 1; }
grep -q 'model.evidence.supported' "$ROOT/site/templates/index.html" || { echo "    the proof band does not read the evidence dataset"; exit 1; }
grep -A1 '^  - id: guaranteed$' "$ROOT/docs/CLAIMS.yaml" | grep -q 'test proves it' \
  && { echo "    docs/CLAIMS.yaml still says a test proves every guaranteed claim"; exit 1; }
exit 0
