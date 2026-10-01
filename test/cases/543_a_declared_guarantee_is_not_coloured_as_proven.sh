# majordomus-covers: none
# A success-coloured status badge has a supported evidence verdict behind it:
# scripts/ci/homepage-check, check `evidence`, over a tree this case builds.
#
# What is proved: a clean tree passes, with a declared `guaranteed` badge in the neutral tone
# and one verdict badge per claim; each of these fails, with a finding that names it — the
# declared status `guaranteed` filed under the success tone (the declaration and the page that
# renders it), a success-coloured verdict for a claim the evidence does not support, a
# success-coloured verdict while no evidence is available, a verdict badge for a claim with no
# verdict at all, a site that renders no verdict, and a product dataset with no evidence
# section. The real tokens are the ones the gate reads in the tree: share/design/tokens.yaml
# files `guaranteed` outside the success tone, and the committed site dataset agrees.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }

F="$PWD/fixture"
fresh() {
  rm -rf "$F"; mkdir -p "$F/site/data/registry" "$F/site/data/generated" "$F/docs/generated" "$F/site/public/guarantees"
  printf 'order = ["hero"]\n' > "$F/site/data/homepage.toml"
  printf '[[groups]]\nlabel = "Home"\nhref = "/"\n' > "$F/site/data/nav.toml"
  printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$F/docs/generated/web.json"
  printf '%s\n' '{}' > "$F/site/data/registry/distribution.json"
  printf '%s\n' '{"states":{"guaranteed":"neutral","advisory":"warn","inputs-unchanged":"info","proven":"ok","not-run":"neutral","no-test":"neutral","unknown":"neutral"}}' > "$F/site/data/registry/design.json"
  printf '%s\n' '{"status_order":["guaranteed","advisory"],"claims":[{"id":"a","status":"guaranteed"},{"id":"b","status":"guaranteed"},{"id":"c","status":"advisory"}]}' > "$F/site/data/generated/capabilities.json"
  printf '%s\n' '{"features":[],"providers":[],"evidence":{"available":true,"declared":2,"supported":1,"claims":{"a":{"state":"inputs-unchanged","label":"inputs unchanged","supported":true},"b":{"state":"not-run","label":"not run","supported":false},"c":{"state":"no-test","label":"no test","supported":false}}}}' > "$F/site/data/registry/product.json"
  printf '<html><head><title>home</title></head><body><main><section id="hero"><a href="/guarantees/">g</a></section></main></body></html>\n' > "$F/site/public/index.html"
  cat > "$F/site/public/guarantees/index.html" <<'HTML'
<html><head><title>g</title></head><body>
<span class="mj-badge mj-badge--guaranteed">guaranteed</span>
<span class="mj-badge mj-badge--inputs-unchanged" data-claim="a">inputs unchanged</span>
<span class="mj-badge mj-badge--not-run" data-claim="b">not run</span>
<span class="mj-badge mj-badge--unknown" data-claim="c">unknown</span>
</body></html>
HTML
}
page="$F/site/public/guarantees/index.html"
run() { MJ_ROOT="$F" "$CHECK" --only evidence > out.txt 2>&1; echo $?; }
expect_finding() { # <exit> <pattern> <what>
  rc="$(run)"
  [ "$rc" = "$1" ] || { echo "    $3: exit $rc, expected $1"; cat out.txt; exit 1; }
  grep -qE "$2" out.txt || { echo "    $3: no finding matching '$2'"; cat out.txt; exit 1; }
}

# --- a clean tree passes, and only the selected check reports
fresh
expect_finding 0 '^OK   evidence +3 claim evidence badge' "a clean tree"
[ "$(grep -c '^\(OK\|FAIL\) ' out.txt)" = 1 ] || { echo "    --only reported other checks"; cat out.txt; exit 1; }

# --- the declared status filed under the success tone: the declaration, and the page
fresh; sed -i.bak 's/"guaranteed":"neutral"/"guaranteed":"ok"/' "$F/site/data/registry/design.json"
expect_finding 10 "declared claim status 'guaranteed' is filed under the success tone" "guaranteed filed as ok"
grep -qE "/guarantees/index.html: the declared status 'guaranteed' is rendered in the success colour" out.txt \
  || { echo "    the page rendering an ok-toned declared status was not named"; cat out.txt; exit 1; }

# --- a success-coloured verdict for a claim the evidence does not support
fresh; sed -i.bak 's#mj-badge--not-run" data-claim="b"#mj-badge--proven" data-claim="b"#' "$page"
expect_finding 10 "claim 'b' is coloured as supported \('proven'\) and the evidence does not support it" "an unsupported claim coloured as proven"

# --- a success-coloured verdict while no evidence is available at all
fresh; sed -i.bak 's#mj-badge--inputs-unchanged" data-claim="a"#mj-badge--proven" data-claim="a"#' "$page"
expect_finding 0 '^OK   evidence' "a supported claim may carry the success colour"
sed -i.bak 's/"available":true/"available":false/' "$F/site/data/registry/product.json"
expect_finding 10 "claim 'a' is coloured as supported" "a success colour with no evidence available"

# --- a verdict badge for a claim with no verdict
fresh; sed -i.bak 's#data-claim="c">unknown#data-claim="c">unknown</span><span class="mj-badge mj-badge--not-run" data-claim="zz">not run#' "$page"
expect_finding 10 "an evidence badge 'not-run' for claim 'zz', which has no verdict" "a verdict with nothing behind it"

# --- a site that renders no verdict at all is not a clean one
fresh; printf '<html><head><title>g</title></head><body><span class="mj-badge mj-badge--guaranteed">guaranteed</span></body></html>\n' > "$page"
expect_finding 10 "no page renders a claim's evidence verdict" "no verdict rendered"

# --- no evidence section in the product dataset
fresh; printf '%s\n' '{"features":[],"providers":[]}' > "$F/site/data/registry/product.json"
expect_finding 10 'product.json carries no evidence section' "no evidence section"

# --- the real declaration: guaranteed is not filed under the success tone, in the tokens and in
#     the committed dataset the gate reads
awk '/^    ok: \[/' "$ROOT/share/design/tokens.yaml" | grep -qE '[[, ]guaranteed[],]' \
  && { echo "    share/design/tokens.yaml files guaranteed under the success tone"; exit 1; }
[ "$(jq -r '.states.guaranteed' "$ROOT/site/data/registry/design.json")" != ok ] \
  || { echo "    site/data/registry/design.json colours guaranteed as a success"; exit 1; }
exit 0
