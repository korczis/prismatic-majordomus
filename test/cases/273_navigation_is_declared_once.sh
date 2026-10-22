# majordomus-covers: none
# The site's navigation is declared once — and the gate can tell a projection from a copy.
#
# `scripts/ci/nav-check` is worth having only if it decides the shapes that actually went
# wrong, and refuses nothing else. Both failures it exists for were real: nav.toml claimed
# the footer rendered from it while the footer was five links typed by hand, and before that
# `/docs/cli/` and `/docs/api/` each sat in two groups under two different labels.
#
# So this case builds a fixture site for each shape rather than trusting what this repository
# happens to contain today:
#
#   clean            five groups, a repeated group href as its first item, a CTA that resolves
#   refused          the same href in two groups; seven groups; a CTA pointing at no page;
#                    a template that types a route nav.toml does not declare; a template
#                    that renders navigation without reading nav.groups
#   unusable         no nav.toml at all
#
# The allowed repeat is the one that looks like the defect and is not: a dropdown's first
# item is the group's own landing page, which is a rendering decision and not a second entry.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/nav-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "    python3 absent; skipping"; exit 0; }
python3 -c 'import tomllib' 2>/dev/null || { echo "    python3 has no tomllib; skipping"; exit 0; }

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-273.XXXXXX")"
trap 'rm -rf "$S"' EXIT

# A fixture site: the declaration, the two projections of it, and the page the CTA points at.
# Every part is the shape the real site has, so a case changes exactly one thing.
fixture() {  # fixture <dir>
  mkdir -p "$1/site/data" "$1/site/templates/partials" "$1/site/content"
  : > "$1/site/content/getting-started.md"
  cat > "$1/site/data/nav.toml" <<'TOML'
[[groups]]
label = "Why"
href = "/why/"

[[groups]]
label = "Product"
href = "/features/"
items = [
  { label = "Features", href = "/features/" },
  { label = "Use cases", href = "/use-cases/" },
]

[[groups]]
label = "Docs"
href = "/docs/"
items = [
  { label = "All documents", href = "/docs/" },
  { label = "Command reference", href = "/docs/cli/" },
]

[cta]
label = "Get started"
href = "/getting-started/"
TOML
  cat > "$1/site/templates/partials/navbar.html" <<'HTML'
{% set nav = load_data(path="data/nav.toml") %}
<nav>
  <a href="{{ get_url(path="/", trailing_slash=true) }}">{{ project.short_name }}</a>
  <a href="{{ project.repository_url }}">GitHub</a>
  <ul>
    {% for g in nav.groups %}
    <li><a href="{{ get_url(path=g.href, trailing_slash=true) }}">{{ g.label }}</a>
      <ul>{% for i in g.items or [] %}<li><a href="{{ get_url(path=i.href, trailing_slash=true) }}">{{ i.label }}</a></li>{% endfor %}</ul>
    </li>
    {% endfor %}
  </ul>
</nav>
HTML
  cat > "$1/site/templates/partials/footer.html" <<'HTML'
{% set nav = load_data(path="data/nav.toml") %}
<footer>
  <a href="{{ get_url(path="/", trailing_slash=true) }}">{{ project.name }}</a>
  <a href="{{ project.repository_url }}/blob/master/LICENSE">{{ project.license }} licence</a>
  {% for g in nav.groups %}
  <div><h2>{{ g.label }}</h2>
    <ul>{% for i in g.items or [] %}<li><a href="{{ get_url(path=i.href, trailing_slash=true) }}">{{ i.label }}</a></li>{% endfor %}</ul>
  </div>
  {% endfor %}
</footer>
HTML
}

# ---------------------------------------------------------------------------- what is clean
# Two things are asserted at once here, and both are deliberate. The home link `/` is typed
# in both templates and is exempt: it is no group's route and every page carries it. And
# `/features/` and `/docs/` each appear twice in the declaration — as a group's href and as
# that group's first item — which is the dropdown's landing link and the one repeat allowed.
C="$S/clean"; fixture "$C"
expect_exit 0 env MJ_ROOT="$C" "$GATE"
expect_grep 'every href in site/data/nav\.toml is declared once'
expect_grep 'navbar\.html renders nav\.groups'
expect_grep 'footer\.html renders nav\.groups'
expect_grep "call to action 'Get started' points at /getting-started/"

# ------------------------------------------------------------- a route declared twice is refused
# The measured defect: /docs/cli/ under two labels in two groups. A visitor meets the same
# page twice and has to guess which label is the real one.
D="$S/duplicate"; fixture "$D"
cat >> "$D/site/data/nav.toml" <<'TOML'

[[groups]]
label = "Reference"
href = "/reference/"
items = [
  { label = "CLI", href = "/docs/cli/" },
]
TOML
expect_exit 10 env MJ_ROOT="$D" "$GATE"
expect_grep '/docs/cli/ is declared twice'
expect_grep "'Docs' → 'Command reference'"
expect_grep "'Reference' → 'CLI'"

# and the same route twice inside one group is the same defect, not a lesser one
D1="$S/duplicate-within"; fixture "$D1"
cat >> "$D1/site/data/nav.toml" <<'TOML'

[[groups]]
label = "Evidence"
href = "/guarantees/"
items = [
  { label = "Guarantees", href = "/guarantees/" },
  { label = "The guarantees page", href = "/guarantees/" },
]
TOML
expect_exit 10 env MJ_ROOT="$D1" "$GATE"
expect_grep '/guarantees/ is declared twice'

# --------------------------------------------------------------- a navbar is not a sitemap
G="$S/sprawl"; fixture "$G"
for n in 1 2 3 4; do
  cat >> "$G/site/data/nav.toml" <<TOML

[[groups]]
label = "Extra $n"
href = "/extra-$n/"
TOML
done
expect_exit 10 env MJ_ROOT="$G" "$GATE"
expect_grep 'declares 7 groups and the limit is 6'
expect_grep 'a navbar is not a sitemap'

# --------------------------------------------------------- the one action may not be dead
K="$S/deadcta"; fixture "$K"
rm -f "$K/site/content/getting-started.md"
expect_exit 10 env MJ_ROOT="$K" "$GATE"
expect_grep 'no page answers it'

# and a declaration with no call to action at all is refused, not ignored
K2="$S/nocta"; fixture "$K2"
python3 - "$K2/site/data/nav.toml" <<'PY'
import sys
p = sys.argv[1]
text = open(p, encoding="utf-8").read()
open(p, "w", encoding="utf-8").write(text.split("[cta]")[0])
PY
expect_exit 10 env MJ_ROOT="$K2" "$GATE"
expect_grep 'declares no \[cta\]'

# ------------------------------------------------------- a route typed beside the declaration
# The failure the gate is named for: a projection that stops projecting. A hard-coded route
# is a second navigation, and editing nav.toml cannot reach it.
T="$S/typed"; fixture "$T"
cat >> "$T/site/templates/partials/footer.html" <<'HTML'
<nav><a href="/economics/">Economics</a></nav>
HTML
expect_exit 10 env MJ_ROOT="$T" "$GATE"
expect_grep 'footer\.html types the site route /economics/'

# the same route through Zola's resolver is the same hand-typed route
T2="$S/typed-geturl"; fixture "$T2"
cat >> "$T2/site/templates/partials/navbar.html" <<'HTML'
<a href="{{ get_url(path="/economics/", trailing_slash=true) }}">Economics</a>
HTML
expect_exit 10 env MJ_ROOT="$T2" "$GATE"
expect_grep 'navbar\.html types the site route /economics/'

# a declared route typed literally is not a finding — it is what the declaration is for —
# and neither is an external URL built from project.repository_url
T3="$S/typed-declared"; fixture "$T3"
cat >> "$T3/site/templates/partials/footer.html" <<'HTML'
<a href="{{ get_url(path="/docs/", trailing_slash=true) }}">Docs</a>
<a href="/why/">Why</a>
<a href="{{ project.repository_url }}/blob/master/SECURITY.md">Security</a>
<a href="{{ get_url(path=extra.href, trailing_slash=true) }}">Computed</a>
HTML
expect_exit 0 env MJ_ROOT="$T3" "$GATE"
expect_no_grep 'types the site route'

# ------------------------------------------------- a projection that does not read nav.groups
# The first measured defect, in a fixture: the footer said it rendered the declaration and
# rendered five links of its own instead.
N="$S/handwritten"; fixture "$N"
cat > "$N/site/templates/partials/footer.html" <<'HTML'
<footer>
  <a href="{{ get_url(path="/", trailing_slash=true) }}">{{ project.name }}</a>
  <ul><li><a href="{{ get_url(path="/docs/", trailing_slash=true) }}">Docs</a></li></ul>
</footer>
HTML
expect_exit 10 env MJ_ROOT="$N" "$GATE"
expect_grep 'footer\.html renders site navigation without reading nav\.groups'

# ------------------------------------------------------------ no declaration at all is unusable
U="$S/nonav"; mkdir -p "$U/site/templates/partials"
expect_exit 12 env MJ_ROOT="$U" "$GATE"
expect_grep 'is missing'

# and a declaration that does not parse is unusable rather than clean
P="$S/malformed"; fixture "$P"
printf '[[groups\nlabel = "broken"\n' >> "$P/site/data/nav.toml"
expect_exit 12 env MJ_ROOT="$P" "$GATE"
expect_grep 'does not parse as TOML'

# ------------------------------------------------ this repository's own navigation is declared once
# The gate is run against the tree it ships in, so a route added to a template by hand, or a
# sixth category, is caught here as well as in CI.
expect_exit 0 "$GATE"
