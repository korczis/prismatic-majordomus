# majordomus-covers: none
# The design is one declaration and no source carries a second one: the gate, by mutation.
#
# `scripts/ci/design-check` asks whether any file a person writes makes a design decision of
# its own. This case builds a fixture out of the real projections — the generated sheets and
# the canonical marks, copied — and then breaks it one way at a time: each break is refused
# and names the file, and each documented exemption passes. The generator itself is proved
# through the executable in 109_design_system.sh; this case never runs it.
. "$ROOT/test/lib.sh"
# question 11 reads the licence evidence from the environment; this case hands it in itself
unset MAJORDOMUS_KIT_EVIDENCE_FLOWBITE_PRO 2>/dev/null || true

DC="$ROOT/scripts/ci/design-check"

# --- the fixture: the projections as generated, in the layout the gate expects
mkdir -p share/design/brand apps/majordomus-cli/src/web apps/majordomus-cli/src/cockpit share/cockpit share/ui site/static/images site
cp "$ROOT"/share/design/*.css share/design/
cp "$ROOT"/share/design/tokens.yaml share/design/
cp "$ROOT"/share/design/brand/logo-mark.svg "$ROOT"/share/design/brand/logo.svg "$ROOT"/share/design/brand/social-card.png share/design/brand/
cp "$ROOT"/apps/majordomus-cli/src/web/tokens.css apps/majordomus-cli/src/web/
cp "$ROOT"/apps/majordomus-cli/src/cockpit/logo-mark.svg apps/majordomus-cli/src/cockpit/
cp "$ROOT"/share/cockpit/favicon.svg share/cockpit/
cp "$ROOT"/share/ui/favicon.svg share/ui/
cp "$ROOT"/site/static/favicon.svg site/static/
cp "$ROOT"/site/static/images/logo-mark.svg "$ROOT"/site/static/images/logo.svg "$ROOT"/site/static/images/social-card.png site/static/images/
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
expect_grep 'a fixture is not asked about projection drift'
expect_grep 'one declaration, and every surface projects it'

# --- 1. a colour chosen outside the declaration
printf '.thing { color: #bada55; }\n' > site/rogue.css
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/rogue.css:1  a colour chosen outside share/design/tokens.yaml'
# the same file, naming a token instead of inventing one, is fine
printf '.thing { color: var(--mj-fg); }\n' > site/rogue.css
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
# an href is not a colour, a pull request is not a colour, composition over tokens is not
# a new colour; a literal inside the composition still is
printf '<a href="#features">f</a>\n' > site/page.html
mkdir -p src && printf 'assert_eq!(c.subject, "Merge pull request #117 from korczis/int");\n' > src/commits.rs
printf '.x { background: color-mix(in oklch, var(--mj-fg) 40%%, transparent); }\n' > site/mix.css
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
printf '.x { background: color-mix(in oklch, #123456 40%%, transparent); }\n' > site/mix.css
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
rm -f site/mix.css site/page.html src/commits.rs

# --- 2. a raw palette utility is a status decided again
printf '.mj-thing { @apply text-green-700 dark:text-green-300; }\n' > site/raw.css
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/raw.css:1  a raw palette utility'
printf '<span class="border-l-amber-500">x</span>\n' > site/raw.html
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/raw.html:1  a raw palette utility'
rm -f site/raw.css site/raw.html

# --- 3. a bracketed size is a step nobody named
printf '.mj-thing { @apply text-[11px] tracking-[0.12em]; }\n' > site/size.css
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/size.css:1  a bracketed size'
printf '.mj-thing { @apply text-label tracking-caps; }\n' > site/size.css
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
rm -f site/size.css

# --- 4. a second declaration is a second value
printf ':root { --mj-fg: red; }\n' > site/second.css
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/second.css:1  declares a canonical custom property'
printf ':root { --text-meta: 9px; }\n' > site/second.css
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
# the three indirection properties are set per state by the generated status sheet and
# may be set by a primitive that composes them; they are not a second value
printf '.mj-thing { --mj-status-fg: var(--mj-ok); }\n' > site/second.css
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
rm -f site/second.css

# --- 5. a token nothing emits is the empty string for ever
printf '.a { color: var(--mj-invented); }\n' > site/reads.css
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'reads --mj-invented, which the declaration does not emit'
# a prefix in prose is not a token
printf '/* the --mj-* names are the roles */\n.a { color: var(--mj-fg); }\n' > site/reads.css
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
rm -f site/reads.css

# --- 6. every copy of the brand is the canonical file
cp share/cockpit/favicon.svg "$T/favicon.bak"
printf '<svg xmlns="http://www.w3.org/2000/svg"><rect fill="#2563eb"/></svg>\n' > share/cockpit/favicon.svg
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'share/cockpit/favicon.svg  differs from share/design/brand/logo-mark.svg'
cp "$T/favicon.bak" share/cockpit/favicon.svg
# and a mark kept anywhere else is a copy nobody regenerates
cp share/design/brand/logo-mark.svg site/another-mark.svg
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/another-mark.svg  a mark outside share/design/brand'
rm -f site/another-mark.svg
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"

# --- 7. the theme contract is read from the page
printf "localStorage.getItem('color-theme');\n" > site/theme.js
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/theme.js:1  names the theme key or class'
printf "document.documentElement.classList.toggle('dark');\n" > site/theme.js
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
printf "var KEY = document.documentElement.dataset.themeKey; localStorage.getItem(KEY);\n" > site/theme.js
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
rm -f site/theme.js

# --- the design model implements the declaration and is not a surface: its fixtures are
# declarations in miniature and its examples say what a value resolves to, so both are made
# of literals, and one test adds a role to a fixture to prove a new token needs no
# registration — a read the canonical declaration does not answer. The same two lines
# anywhere else are refused.
mkdir -p apps/majordomus-cli/src/design
printf 'const FIXTURE: &str = "bg: oklch(21%% 0.034 264.665)";\nfn t() { assert!(css.contains("--mj-overlay")); }\n' > apps/majordomus-cli/src/design/mod.rs
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
cp apps/majordomus-cli/src/design/mod.rs site/elsewhere.rs
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/elsewhere.rs'
expect_grep 'reads --mj-overlay, which the declaration does not emit'
rm -f site/elsewhere.rs
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"

# --- the other documented exemptions hold: third-party bytes and compiled artifacts, and
# the colour arithmetic of the audit
mkdir -p share/cockpit/vendor scripts/lib
printf '.x{color:#abcdef}\n' > share/cockpit/vendor/lib.min.css
printf '.x{color:#abcdef}\n' > share/cockpit/cockpit.css
printf 'export const BLACK = "#000000";\n' > scripts/lib/ui-contrast.mjs
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"

# --- the graph viewer is NOT one of them any more. It was exempted by path, with the reason
# "a categorical scale over data, generated at run time; there is no literal palette to
# share" — and the second half was untrue: a categorical series is exactly what a
# declaration can hold, and the hue it rotated moved every node kind's colour whenever one
# kind was added. share/design/tokens.yaml now declares the series, the viewer reads
# `--mj-series-<n>`, and this file is read like every other surface. The assertion is the
# same one, inverted: what used to prove the exemption now proves its absence.
printf 'const hue = "#ff0000";\n' > share/cockpit/graph.js
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'share/cockpit/graph.js'
expect_grep 'a colour chosen outside share/design/tokens.yaml'
rm -f share/cockpit/graph.js
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"

# --- a read that names a token *family* is answered by the family, not by one token. The
# viewer asks for `--mj-series-1`, `--mj-series-2` and so on until the page stops answering,
# because how many colours the series has is the declaration's decision and not a number in
# a script. The declaration must still emit at least one member: a surface cannot invent a
# family any more than it can invent a token.
# the series the declaration does emit: a read of the family is answered by the family
printf 'const c = getComputedStyle(e).getPropertyValue("--mj-series-" + n);\n' > share/cockpit/graph.js
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
# a family the declaration does not emit is refused exactly as a single unknown token is
printf 'const c = getComputedStyle(e).getPropertyValue("--mj-nosuch-" + n);\n' > share/cockpit/graph.js
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'reads the family --mj-nosuch-\*, of which the declaration emits nothing'
rm -f share/cockpit/graph.js
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"

# --- 10. a surface adapted from a Flowbite Pro block names the block and brings none of its
# values (ADR 0036, amendment of 2026-11-07; I4316). The review surfaces are such surfaces
# from the day they exist: /quality/ is site/templates/quality.html.
mkdir -p site/templates
printf '<section class="bg-neutral-primary-soft text-heading">q</section>\n' > site/templates/quality.html
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/templates/quality.html  a review surface that names no Flowbite Pro block'
# naming its block, on the tokens: the adaptation the amendment asks for
{
  printf '{# flowbite-pro: application-ui/tables/table-with-search #}\n'
  printf '<section class="bg-neutral-primary-soft text-heading border-default">q</section>\n'
} > site/templates/quality.html
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
expect_grep '1 file\(s\) name the Flowbite Pro block they adapt'
# the block's blue, kept raw: refused, naming the file and the class
printf '<button class="bg-blue-600 text-sm">Go</button>\n' >> site/templates/quality.html
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/templates/quality.html:3  a raw palette utility \(bg-blue-600\)'
# Pro's own primary scale and its white label: what question 2 does not name, this one does
sed -i.bak '3s/.*/<button class="bg-primary-700 hover:bg-primary-800 text-white">Go<\/button>/' site/templates/quality.html
rm -f site/templates/quality.html.bak
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/templates/quality.html:3  a Flowbite Pro palette value \(bg-primary-700\)'
expect_grep 'site/templates/quality.html:3  a Flowbite Pro palette value \(hover:bg-primary-800\)|\(bg-primary-800\)'
expect_grep 'site/templates/quality.html:3  a Flowbite Pro palette value \(text-white\)'
# a type value of its own, as a bracket or as a literal in a style
sed -i.bak '3s/.*/<span class="text-[13px]" style="font-size: 12px">x<\/span>/' site/templates/quality.html
rm -f site/templates/quality.html.bak
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'site/templates/quality.html:3  a type value of its own \(text-\[13px\]\)'
expect_grep 'site/templates/quality.html:3  a type value of its own \(font-size: 1'
# a bracket that reads a token, and a named step, are reads of the declaration
sed -i.bak '3s/.*/<span class="text-small text-[color:var(--mj-fg)] bg-brand">x<\/span>/' site/templates/quality.html
rm -f site/templates/quality.html.bak
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
# a page that is no review surface and names no block is not asked about Pro's values
printf '<p class="text-white">x</p>\n' > site/templates/other.html
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"
expect_grep '1 file\(s\) name the Flowbite Pro block they adapt'
rm -f site/templates/other.html
git add -A >/dev/null 2>&1

# --- 11. no tracked file is a licensed kit's file, byte for byte. The "licensed" bytes are
# this case's own: no Flowbite Pro byte is in this repository, and none is needed to prove
# the comparison. The evidence lives outside the fixture tree, as the owner's does.
EVD="$(mktemp -d "${TMPDIR:-/tmp}/mj107.XXXXXX")"
trap 'rm -rf "$EVD"' EXIT
printf '<div class="fixture-pro-table">\n  <nav>block</nav>\n</div>\n' > "$EVD/table.html"
exact="$(sha256_of_file "$EVD/table.html")"
norm="$(LC_ALL=C tr '\t\r\n' '   ' < "$EVD/table.html" | LC_ALL=C tr -s ' ' | mj_sha256_hex)"
printf '# kit: flowbite-pro\n# kit-evidence/v1\n%s\t%s\tfixture-pro/blocks/table.html\n' "$exact" "$norm" > "$EVD/evidence.tsv"
# no evidence: said not to be judged, never said to pass
expect_exit 0 "$DC" --root "$T"
expect_grep '^NOTE not judged here: no kit evidence'
# evidence, and no copy in the tree
expect_exit 0 "$DC" --root "$T" --evidence "$EVD/evidence.tsv"
expect_grep 'tracked file\(s\) compared with 1 licensed file\(s\) of flowbite-pro; none is a copy'
# a licensed file committed as shipped: refused, naming the file and the licensed source
cp "$EVD/table.html" site/table.html
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T" --evidence "$EVD/evidence.tsv"
expect_grep '^FAIL site/table.html  is byte for byte fixture-pro/blocks/table.html \(flowbite-pro\), a licensed file'
# the same evidence as CI holds it, base64(gzip(text)) in the secret
SECRET="$(gzip -c "$EVD/evidence.tsv" | base64 | tr -d '\n')"
MAJORDOMUS_KIT_EVIDENCE_FLOWBITE_PRO="$SECRET" expect_exit 10 "$DC" --root "$T"
expect_grep '^FAIL site/table.html  is byte for byte fixture-pro/blocks/table.html'
# a secret that is not evidence is named and never printed
MAJORDOMUS_KIT_EVIDENCE_FLOWBITE_PRO="not-evidence-0f1e2d3c" expect_exit 10 "$DC" --root "$T"
expect_grep 'MAJORDOMUS_KIT_EVIDENCE_FLOWBITE_PRO does not hold base64\(gzip\(kit-evidence/v1\)\)'
expect_no_grep 'not-evidence-0f1e2d3c'
# an evidence file with a line that is no fingerprint is named by the line, not its content
printf '# kit-evidence/v1\nsecret-looking-line\n' > "$EVD/bad.tsv"
expect_exit 10 "$DC" --root "$T" --evidence "$EVD/bad.tsv"
expect_grep 'the kit evidence is not kit-evidence/v1 \(line 2\)'
expect_no_grep 'secret-looking-line'
# adapted, the file is no longer the licensed one: byte identity is the gate's boundary, and
# the amendment leaves a lightly edited copy to the review
printf '<div class="bg-neutral-primary-soft">\n  <nav>block</nav>\n</div>\n' > site/table.html
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T" --evidence "$EVD/evidence.tsv"
rm -f site/table.html site/templates/quality.html
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T" --evidence "$EVD/evidence.tsv"
