# majordomus-covers: none
# The design is one declaration and no source carries a second one: the gate, by mutation.
#
# `scripts/ci/design-check` asks whether any file a person writes makes a design decision of
# its own. This case builds a fixture out of the real projections — the generated sheets and
# the canonical marks, copied — and then breaks it one way at a time: each break is refused
# and names the file, and each documented exemption passes. The generator itself is proved
# through the executable in 109_design_system.sh; this case never runs it.
. "$ROOT/test/lib.sh"

DC="$ROOT/scripts/ci/design-check"

# --- the fixture: the projections as generated, in the layout the gate expects
mkdir -p share/design/brand apps/majordomus-cli/src/web apps/majordomus-cli/src/cockpit share/cockpit site/static/images site
cp "$ROOT"/share/design/*.css share/design/
cp "$ROOT"/share/design/tokens.yaml share/design/
cp "$ROOT"/share/design/brand/logo-mark.svg "$ROOT"/share/design/brand/logo.svg "$ROOT"/share/design/brand/social-card.png share/design/brand/
cp "$ROOT"/apps/majordomus-cli/src/web/tokens.css apps/majordomus-cli/src/web/
cp "$ROOT"/apps/majordomus-cli/src/cockpit/logo-mark.svg apps/majordomus-cli/src/cockpit/
cp "$ROOT"/share/cockpit/favicon.svg share/cockpit/
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
