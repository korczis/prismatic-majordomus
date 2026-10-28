# majordomus-covers: none
# A figure of the homepage's hero strip and the page it links are two renderings of one count,
# so they say the same number. On 2026-10-28 majordomus.dev (serving 81ef931) said "rules a
# machine enforces, not a reviewer 68" and linked /doctrines/, which said "declared 50
# blocking 43 advisory 7": the homepage counted this repository's resolved rules
# (product.json `rules`), the page it linked counted the rule package's doctrines (I4318).
# scripts/ci/homepage-check, check `figures`, over a tree this case builds.
#
# What is proved: a clean tree passes, its rule figure linked by an absolute URL of the site as
# the build writes it, beside a figure that names no key and links a page that declares none.
# Each of these fails, with a finding that names it — the evidence of I4318 itself (68 against
# 43), a key the linked page does not carry, a linked page the build did not write, a figure
# that names no key while its target declares figures, and a homepage with no hero strip.
# --only figures reports that check alone. The real templates name the pair on both pages and
# the homepage reads the doctrine package's blocking count, never this repository's rules.
# Rule: project.homepage-tells-a-declared-story.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }

F="$PWD/fixture"
home="$F/site/public/index.html"
doc="$F/site/public/doctrines/index.html"
fresh() {
  rm -rf "$F"
  mkdir -p "$F/site/data/registry" "$F/site/data/generated" "$F/docs/generated" \
    "$F/site/public/doctrines" "$F/site/public/use-cases"
  printf 'base_url = "https://x.test"\n' > "$F/site/config.toml"
  printf 'order = ["hero"]\n' > "$F/site/data/homepage.toml"
  printf '[[groups]]\nlabel = "Home"\nhref = "/"\n' > "$F/site/data/nav.toml"
  printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$F/docs/generated/web.json"
  printf '%s\n' '{}' > "$F/site/data/registry/distribution.json"
  printf '%s\n' '{"states":{"guaranteed":"neutral","unknown":"neutral"}}' > "$F/site/data/registry/design.json"
  printf '%s\n' '{"status_order":["guaranteed"],"claims":[]}' > "$F/site/data/generated/capabilities.json"
  printf '%s\n' '{"features":[],"providers":[],"evidence":{"available":false,"claims":{}}}' > "$F/site/data/registry/product.json"
  cat > "$home" <<'HTML'
<html><head><title>home</title></head><body><main><section id="hero">
<dl class="grid" data-hero-figures>
<div class="p"><dt class="t"><a href="https://x.test/doctrines/" class="u">blocking rules a machine enforces, not a reviewer</a></dt><dd data-figure="rules-blocking" class="v">43</dd></div>
<div class="p"><dt class="t"><a href="https://x.test/use-cases/" class="u">worked examples</a></dt><dd class="v">12</dd></div>
</dl>
</section></main></body></html>
HTML
  cat > "$doc" <<'HTML'
<html><head><title>doctrines</title></head><body><dl>
<div><dt>declared</dt><dd class="v">50</dd></div>
<div><dt>blocking</dt><dd data-figure="rules-blocking" class="v">43</dd></div>
<div><dt>advisory</dt><dd class="v">7</dd></div>
</dl></body></html>
HTML
  printf '<html><head><title>use cases</title></head><body><p>12 use cases</p></body></html>\n' > "$F/site/public/use-cases/index.html"
}
run() { MJ_ROOT="$F" "$CHECK" --only figures > out.txt 2>&1; echo $?; }
expect_finding() { # <exit> <pattern> <what>
  rc="$(run)"
  [ "$rc" = "$1" ] || { echo "    $3: exit $rc, expected $1"; cat out.txt; exit 1; }
  grep -qE "$2" out.txt || { echo "    $3: no finding matching '$2'"; cat out.txt; exit 1; }
}

# --- clean: the rule figure reads what /doctrines/ shows; a keyless figure whose target
#     declares no figure is not compared
fresh
expect_finding 0 '^OK   figures +1 hero figure\(s\) read what the page they link shows: rules-blocking 43 = /doctrines/$' "a clean tree"
[ "$(grep -c '^\(OK\|FAIL\) ' out.txt)" = 1 ] || { echo "    --only figures reported other checks"; cat out.txt; exit 1; }

# --- the evidence of I4318: the homepage counts 68, the page it links shows 43
fresh; sed -i.bak 's#data-figure="rules-blocking" class="v">43#data-figure="rules-blocking" class="v">68#' "$home"
expect_finding 10 "^FAIL figures +the figure 'blocking rules a machine enforces, not a reviewer' reads '68', and /doctrines/ shows rules-blocking '43'" "68 against 43"

# --- the linked page changes its count and the homepage does not follow
fresh; sed -i.bak 's#data-figure="rules-blocking" class="v">43#data-figure="rules-blocking" class="v">44#' "$doc"
expect_finding 10 "reads '43', and /doctrines/ shows rules-blocking '44'" "a target that moved"

# --- a key the linked page does not carry
fresh; sed -i.bak 's#data-figure="rules-blocking" ##' "$doc"
expect_finding 10 "names 'rules-blocking', and /doctrines/ shows no such figure" "a key the target lost"

# --- a linked page the build did not write
fresh; rm "$doc"
expect_finding 10 "links /doctrines/, a page the build did not write" "a target never built"

# --- a figure that names no key while its target declares figures
fresh; sed -i.bak 's#<dd data-figure="rules-blocking" class="v">43</dd></div>#<dd class="v">68</dd></div>#' "$home"
expect_finding 10 "the figure 'blocking rules a machine enforces, not a reviewer' \(68\) names no data-figure, and /doctrines/ shows rules-blocking 43" "a keyless figure"

# --- a relative link resolves the same way the absolute one does
fresh; sed -i.bak 's#href="https://x.test/doctrines/"#href="/doctrines/"#' "$home"
sed -i.bak 's#class="v">43</dd></div>#class="v">68</dd></div>#' "$home"
expect_finding 10 "reads '68', and /doctrines/ shows rules-blocking '43'" "a relative link"

# --- no hero strip at all
fresh; printf '<html><head><title>home</title></head><body><main><section id="hero">x</section></main></body></html>\n' > "$home"
expect_finding 10 'the homepage has no hero figure strip' "no strip"

# --- the real templates: the pair is named on both pages, and the homepage reads the rule
#     package's blocking count rather than counting this repository's resolved rules
TPL="$ROOT/site/templates/index.html"
DOC="$ROOT/site/templates/doctrines-section.html"
grep -qF 'load_data(path="data/generated/doctrines.json")' "$TPL" || { echo "    the homepage does not read the doctrine dataset"; exit 1; }
grep -qF 'set blocking = doctrines.counts.blocking' "$TPL" || { echo "    the homepage's rule figure is not the doctrine package's blocking count"; exit 1; }
grep -qF '"v": blocking, "h": "/doctrines/", "k": "rules-blocking"' "$TPL" || { echo "    the homepage's rule figure does not name rules-blocking beside its /doctrines/ link"; exit 1; }
grep -qF 'data-figure="{{ f.k }}"' "$TPL" || { echo "    the hero strip does not render a figure's key"; exit 1; }
grep -qE 'model\.rules if r\.class == "blocking" and r\.enforced' "$TPL" && { echo "    the homepage still counts this repository's resolved rules for its figure"; exit 1; }
grep -qF 'data-figure="rules-blocking" class="mt-1 text-2xl font-semibold text-heading">{{ doc.counts.blocking }}' "$DOC" \
  || { echo "    /doctrines/ does not name its blocking count rules-blocking"; exit 1; }
exit 0
