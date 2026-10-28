# majordomus-covers: none
# A figure of the homepage's hero strip and the page it links are two renderings of one count,
# so they say the same number. On 2026-10-28 majordomus.dev (serving 81ef931) said "rules a
# machine enforces, not a reviewer 68" and linked /doctrines/, which said "declared 50
# blocking 43 advisory 7": the homepage counted this repository's resolved rules
# (product.json `rules`), the page it linked counted the rule package's doctrines (I4318).
# scripts/ci/homepage-check, check `figures`, over a tree this case builds.
#
# What is proved: a clean tree passes, its rule figure linked by an absolute URL of the site as
# the build writes it, beside a figure that names no key and links a page that declares none,
# and the evidence chain's rules node, outside the strip, linked by its own data-figure-href.
# Each of these fails, with a finding that names it — the evidence of I4318 itself (68 against
# 43), also when the rules <dt> carries markup before its link and when the chain node says it,
# a key the linked page does not carry, a linked page the build did not write, a figure that
# names no key while its target declares figures, a homepage with no hero strip, a <dd> of the
# strip or a data-figure the check cannot read, and a keyed figure outside the strip that links
# no page. --only figures reports that check alone. The real templates name the pair on both
# pages, the homepage and its chain read the doctrine package's blocking count, never this
# repository's rules, and /method/argument/ labels its count of those rules as another set.
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
</section><section id="chain"><button><span data-figure="rules-blocking" data-figure-href="https://x.test/doctrines/" class="n">43</span><span>blocking doctrines that stop the command</span></button>
<div data-chain-detail="rules"><p data-figure="rules-blocking" data-figure-href="https://x.test/doctrines/" class="d">43</p><a href="https://x.test/doctrines/">blocking doctrines that stop the command</a></div>
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
expect_finding 0 '^OK   figures +3 homepage figure\(s\) read what the page they link shows: rules-blocking 43 = /doctrines/$' "a clean tree"
[ "$(grep -c '^\(OK\|FAIL\) ' out.txt)" = 1 ] || { echo "    --only figures reported other checks"; cat out.txt; exit 1; }

# --- the evidence of I4318: the homepage counts 68, the page it links shows 43
fresh; sed -i.bak 's#data-figure="rules-blocking" class="v">43#data-figure="rules-blocking" class="v">68#' "$home"
expect_finding 10 "^FAIL figures +the figure 'blocking rules a machine enforces, not a reviewer' reads '68', and /doctrines/ shows rules-blocking '43'" "68 against 43"

# --- the same evidence when the rules <dt> carries markup before its link: the cell is still
#     read, never dropped from the comparison
fresh; sed -i.bak 's#<dt class="t"><a href="https://x.test/doctrines/"#<dt class="t"><span class="sr-only">1</span><a href="https://x.test/doctrines/"#' "$home"
sed -i.bak 's#data-figure="rules-blocking" class="v">43#data-figure="rules-blocking" class="v">68#' "$home"
expect_finding 10 "^FAIL figures +the figure 'blocking rules a machine enforces, not a reviewer' reads '68', and /doctrines/ shows rules-blocking '43'" "68 against 43 behind extra markup"

# --- the evidence chain's rules node says 68 while /doctrines/ shows 43
fresh; sed -i.bak 's#class="d">43#class="d">68#' "$home"
expect_finding 10 "^FAIL figures +the figure 'rules-blocking' outside the hero strip reads '68', and /doctrines/ shows rules-blocking '43'" "the chain node"

# --- a keyed figure outside the strip that links no page
fresh; sed -i.bak 's#data-figure-href="https://x.test/doctrines/" class="n"#class="n"#' "$home"
expect_finding 10 "the figure 'rules-blocking' outside the hero strip \(43\) links no page: it carries no data-figure-href" "a chain node with no link"

# --- a <dd> of the strip before any <dt>: nothing says what it counts
fresh; sed -i.bak 's#<dl class="grid" data-hero-figures>#<dl class="grid" data-hero-figures><dd data-figure="rules-blocking">68</dd>#' "$home"
expect_finding 10 "1 <dd> of the hero figure strip stand\(s\) before any <dt>" "a stray <dd>"
grep -qE "names 4 figure\(s\) by data-figure and 3 could be read" out.txt || { echo "    a stray keyed <dd> was not counted as unread"; cat out.txt; exit 1; }

# --- a <dd> the parser cannot read: opened, never closed
fresh; sed -i.bak 's#class="v">43</dd></div>#class="v">68</div>#' "$home"
expect_finding 10 "the figure 'blocking rules a machine enforces, not a reviewer' opens 1 <dd> and 0 could be read" "an unclosed <dd>"

# --- a data-figure the check cannot read at all
fresh; sed -i.bak 's#<section id="chain">#<section id="chain"><img data-figure="rules-blocking" alt="68">#' "$home"
expect_finding 10 "names 4 figure\(s\) by data-figure and 3 could be read, so 1 would go uncompared" "an unreadable data-figure"

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
CHAIN="$ROOT/site/templates/partials/evidence-chain.html"
ARG="$ROOT/site/templates/argument.html"
grep -qF 'load_data(path="data/generated/doctrines.json")' "$TPL" || { echo "    the homepage does not read the doctrine dataset"; exit 1; }
grep -qF 'set blocking = doctrines.counts.blocking' "$TPL" || { echo "    the homepage's rule figure is not the doctrine package's blocking count"; exit 1; }
grep -qF '"v": blocking, "h": "/doctrines/", "k": "rules-blocking"' "$TPL" || { echo "    the homepage's rule figure does not name rules-blocking beside its /doctrines/ link"; exit 1; }
grep -qF 'data-figure="{{ f.k }}"' "$TPL" || { echo "    the hero strip does not render a figure's key"; exit 1; }
grep -qE 'model\.rules if r\.class == "blocking" and r\.enforced' "$TPL" && { echo "    the homepage still counts this repository's resolved rules for its figure"; exit 1; }
grep -qF 'data-figure="rules-blocking" class="mt-1 text-2xl font-semibold text-heading">{{ doc.counts.blocking }}' "$DOC" \
  || { echo "    /doctrines/ does not name its blocking count rules-blocking"; exit 1; }
grep -qF '"label": "blocking doctrines that stop the command", "v": blocking, "unit": "blocking, decided by a machine", "text": m.contract.caption, "href": "/doctrines/", "k": "rules-blocking"}' "$CHAIN" \
  || { echo "    the evidence chain's rules node is not the doctrine package's blocking count named rules-blocking"; exit 1; }
[ "$(grep -cF 'data-figure="{{ n.k }}" data-figure-href="{{ get_url(path=n.href, trailing_slash=true) }}"' "$CHAIN")" = 2 ] \
  || { echo "    the evidence chain does not name its keyed node's value and detail with their link"; exit 1; }
grep -qF 'blocking | length' "$CHAIN" && { echo "    the evidence chain still counts a list for its rules node"; exit 1; }
grep -qF "<dt class=\"text-sm text-heading\">of this repository's resolved rules, stop the command</dt>" "$ARG" \
  || { echo "    /method/argument/ does not name its count as this repository's resolved rules"; exit 1; }
grep -qF '>rules that stop the command<' "$ARG" && { echo "    /method/argument/ labels another set as the chain's rules"; exit 1; }
exit 0
