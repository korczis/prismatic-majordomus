# The design is one declaration and every surface is a projection of it.
#
# Four surfaces put HTML in front of a person here, and before this machinery each carried
# a palette of its own — seven of them once the gate was written and actually looked. The
# guarantees this case exists for are the two that are easy to lose:
#
#   a projection that is stale fails loudly, and
#   a projection that is *malformed* never gets written at all.
#
# The second is the one that cost time. A generator that emits `--font-sans: ...Emoji;`
# with an unbalanced quote produces a declaration the CSS parser drops in silence: the
# build succeeds, the drift check passes, every gate stays green, and one surface quietly
# loses its type stack. Both mutations below are bugs this generator really had.
. "$ROOT/test/lib.sh"

DT="$ROOT/scripts/design-tokens"
DC="$ROOT/scripts/ci/design-check"

# The fixture is the real layout, small enough to read: the generator is given a tree, not
# a copy of itself.
mkdir -p share/design apps/majordomus-cli/src/web
tokens() { cat > share/design/tokens.yaml; }

tokens <<'YAML'
schema: 1
font:
  sans: ui-sans-serif, system-ui, "Noto Color Emoji"
  mono: ui-monospace, monospace
accent:
  500: oklch(0.62 0.16 250)
  600: oklch(0.55 0.16 250)
surface-light:
  bg: "#fff"
  raised: "#fff"
  sunken: oklch(98.5% 0.002 247.839)
  line: oklch(92.8% 0.006 264.531)
  fg: oklch(21% 0.034 264.665)      # gray-900
  muted: oklch(55.1% 0.027 264.364)
  ok: oklch(50.8% 0.118 165.612)
  warn: oklch(55.5% 0.163 48.998)
  bad: oklch(51.4% 0.222 16.935)
surface-dark:
  bg: oklch(0.19 0.005 250)
  raised: oklch(0.23 0.006 250)
  sunken: oklch(0.16 0.005 250)
  line: oklch(0.32 0.008 250)
  fg: oklch(0.94 0.004 250)
  muted: oklch(0.68 0.008 250)
  ok: oklch(76.5% 0.177 163.223)
  warn: oklch(82.8% 0.189 84.429)
  bad: oklch(71.2% 0.194 13.428)
alias:
  --mj-bg: bg
  --mj-graph-accent: accent-600
YAML

# ---------------------------------------------------------------- the three projections
expect_exit 0 "$DT" --root "$T"
test -f share/design/theme.css
test -f share/design/surface.css
test -f apps/majordomus-cli/src/web/tokens.css

# every generated file says so, and says where it came from
for f in share/design/theme.css share/design/surface.css apps/majordomus-cli/src/web/tokens.css; do
  grep -q 'GENERATED FILE' "$f"
  grep -q 'share/design/tokens.yaml' "$f"
done

# the Tailwind theme carries type and the accent ramp and nothing else
grep -q '^@theme {' share/design/theme.css
grep -q -- '--color-accent-600: oklch(0.55 0.16 250);' share/design/theme.css
! grep -q -- '--fg:' share/design/theme.css      # the surface is not in the Tailwind theme

# the Cockpit's block is in its layer, under its selector, with its own names as aliases
grep -q '^@layer base {' share/design/surface.css
grep -q -- '--mj-bg: var(--bg);' share/design/surface.css
grep -q -- '--mj-graph-accent: var(--color-accent-600);' share/design/surface.css
grep -q ':where(.dark) {' share/design/surface.css

# the compiled-in block answers dark twice: a report opened from the filesystem has run no
# script, and the Cockpit and the site switch a class
grep -q '@media (prefers-color-scheme: dark)' apps/majordomus-cli/src/web/tokens.css
grep -q '^:root.dark {' apps/majordomus-cli/src/web/tokens.css

# ---------------------------------------------------------------- a quoted value survives
# The bug: stripping the quotes of every value took the closing quote off a font stack that
# ends in one, and emitted `--font-sans: ...Emoji;` — which CSS drops without a word.
grep -q -- '--font-sans: ui-sans-serif, system-ui, "Noto Color Emoji";' share/design/theme.css
for f in share/design/theme.css share/design/surface.css apps/majordomus-cli/src/web/tokens.css; do
  awk -F'"' 'NF > 0 && NF % 2 == 0 { print FILENAME ":" NR ": unbalanced quote: " $0; bad = 1 } END { exit bad + 0 }' "$f"
done

# ---------------------------------------------------------------- a colour is not a comment
# The other bug: stripping trailing comments ate `#fff`, and `--bg:` was written with no
# value at all. A `#` after whitespace is a comment; a `#` that opens a value is a colour.
grep -q -- '--bg: #fff;' share/design/surface.css
grep -q -- '--fg: oklch(21% 0.034 264.665);' share/design/surface.css   # the comment is gone
! grep -q 'gray-900' share/design/surface.css   # a trailing comment never reaches the output

# ---------------------------------------------------------------- drift is refused
expect_exit 0 "$DT" --root "$T" --check
printf '/* edited by hand */\n' >> share/design/theme.css
expect_exit 10 "$DT" --root "$T" --check
expect_grep 'stale against share/design/tokens.yaml'
expect_grep 'share/design/theme.css'
expect_exit 0 "$DT" --root "$T"
expect_exit 0 "$DT" --root "$T" --check

# a value changed in the canonical file reaches every projection, or the check fails
sed -i.bak 's/oklch(0.55 0.16 250)/oklch(0.55 0.16 120)/' share/design/tokens.yaml && rm -f share/design/tokens.yaml.bak
expect_exit 10 "$DT" --root "$T" --check
expect_exit 0 "$DT" --root "$T"
grep -q -- '--color-accent-600: oklch(0.55 0.16 120);' share/design/theme.css

# ---------------------------------------------------------------- an alias is a synonym
# An alias that points at nothing is a name with a value of its own, which is the thing
# this whole file exists to prevent.
printf '  --mj-nonsense: no-such-token\n' >> share/design/tokens.yaml
expect_exit 1 "$DT" --root "$T"
expect_grep 'neither a surface token nor an accent step'
sed -i.bak '/--mj-nonsense/d' share/design/tokens.yaml && rm -f share/design/tokens.yaml.bak
expect_exit 0 "$DT" --root "$T"

# ---------------------------------------------------------------- no second palette
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"

mkdir -p site
printf '.thing { color: #bada55; }\n' > site/rogue.css
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
expect_grep 'a colour chosen outside share/design/tokens.yaml'
expect_grep 'site/rogue.css'

# the same file, naming a token instead of inventing one, is fine
printf '.thing { color: var(--fg); }\n' > site/rogue.css
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"

# an href is not a colour: `#features` is three hex digits followed by more letters, and a
# gate that cannot tell them apart is a gate people turn off
printf '<a href="#features">f</a>\n' > site/page.html
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"

# composition over tokens is not a new colour
printf '.x { background: color-mix(in oklch, var(--fg) 40%%, transparent); }\n' > site/mix.css
git add -A >/dev/null 2>&1
expect_exit 0 "$DC" --root "$T"

# but a literal inside one is still a literal
printf '.x { background: color-mix(in oklch, #123456 40%%, transparent); }\n' > site/mix.css
git add -A >/dev/null 2>&1
expect_exit 10 "$DC" --root "$T"
