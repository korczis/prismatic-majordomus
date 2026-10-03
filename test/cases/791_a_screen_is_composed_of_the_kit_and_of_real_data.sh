# majordomus-covers: none
# A screen is composed of the kit and of real data: by audit, then by mutation.
#
# A screen (site/templates/screens/<name>.html, rendered at /kit/<name>/) renders one of the
# owner's page designs from nothing but the kit's components and this repository's datasets.
# The designs carried values nobody measured — a version that was never released, counts of
# objects that do not exist, latencies, an issue number, a port — and the first thing a screen
# composed against a picture gets wrong is to copy them. The audit below holds every screen to
# the frame and the data: it extends templates/screen.html, it has its page under content/kit/
# with an icon the set holds, it chooses no look of its own (no style, no script, no colour,
# no class the kit does not define, no component nobody declared), and it shows none of the
# designs' invented values. Then a fixture of the same files is broken one way at a time, and
# each break must be named.
. "$ROOT/test/lib.sh"

# The designs' invented values, as they were drawn. A screen that shows one of them copied the
# picture instead of reading a dataset.
INVENTED='v0\.8\.2|#184|482 passed|127\.0\.0\.1:1111|Rust 1\.91|Node 22\.20|60[-–]80%|3[-–]5[x×]|All systems operational'

# screens_audit <root>: every finding on its own line; exit 10 when there is one.
screens_audit() {
  local r="$1" found=0 f n c src defined
  defined="$(cat "$r"/site/templates/*.html "$r"/site/templates/kit/*.html 2>/dev/null | sed -n 's/^{% component \([a-z_]*\)(.*/\1/p' | LC_ALL=C sort -u)"
  for f in "$r"/site/templates/screens/*.html; do
    n="$(basename "$f" .html)"
    head -1 "$f" | grep -qF '{% extends "screen.html" %}' || { printf 'screen %s does not extend screen.html\n' "$n"; found=1; }
    grep -qF '{% block screen %}' "$f" || { printf 'screen %s fills no screen block\n' "$n"; found=1; }
    src="$r/site/content-src/kit/$n.md"
    if [ ! -f "$src" ]; then printf 'screen %s has no page under site/content-src/kit/\n' "$n"; found=1
    else
      grep -qF "template = \"screens/$n.html\"" "$src" || { printf 'site/content-src/kit/%s.md does not render screens/%s.html\n' "$n" "$n"; found=1; }
      c="$(sed -n 's/^icon = "\([a-z-]*\)"$/\1/p' "$src")"
      grep -qE "^${c:-none} = '" "$r/site/data/kit/icons.toml" || { printf 'the page of screen %s names the icon "%s", which the set does not hold\n' "$n" "$c"; found=1; }
      [ -f "$r/site/content/kit/$n.md" ] || { printf 'screen %s has no projection under site/content/kit/\n' "$n"; found=1; }
    fi
    grep -qE '<style|<script' "$f" && { printf 'screen %s carries a style or script block of its own\n' "$n"; found=1; }
    grep -qE ' style="' "$f" && { printf 'screen %s sets a style attribute; the site refuses inline style\n' "$n"; found=1; }
    grep -qE '#[0-9a-fA-F]{6}\b|\b(rgba?|hsla?|oklch)\(' "$f" && { printf 'screen %s chooses a colour\n' "$n"; found=1; }
    grep -qE '\b(text|bg|border|ring|fill|stroke)-(red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose|slate|gray|zinc|neutral|stone)-[0-9]{2,3}\b' "$f" && { printf 'screen %s reaches for a raw palette utility\n' "$n"; found=1; }
    for c in $(grep -oE 'mj-[a-z0-9-]*[a-z0-9](--\{\{)?' "$f" | grep -v '{{$' | LC_ALL=C sort -u); do
      case "$c" in mj-tone--*|mj-badge--*|mj-status--*) grep -qF -- ".${c} " "$r/share/design/status.css" && continue ;; esac
      grep -qE "\.${c}([^a-z0-9-]|$)" "$r/share/design/kit.css" "$r/share/design/primitives.css" "$r/share/design/status.css" && continue
      printf 'screen %s writes the class %s, which no stylesheet defines\n' "$n" "$c"; found=1
    done
    for c in $(grep -oE '\{%? ?<[a-z_]+[ >/]|\{\{ <[a-z_]+[ >/]' "$f" | sed -E 's/.*<([a-z_]+).*/\1/' | LC_ALL=C sort -u); do
      printf '%s\n' "$defined" | grep -qx "$c" || { printf 'screen %s calls the component %s, which nothing declares\n' "$n" "$c"; found=1; }
    done
    grep -qE "$INVENTED" "$f" && { printf 'screen %s shows a value the design invented: %s\n' "$n" "$(grep -oE "$INVENTED" "$f" | head -1)"; found=1; }
  done
  for src in "$r"/site/content-src/kit/*.md; do
    n="$(basename "$src" .md)"; [ "$n" = "_index" ] && continue
    [ -f "$r/site/templates/screens/$n.html" ] || { printf 'site/content-src/kit/%s.md names a screen that has no template\n' "$n"; found=1; }
  done
  [ "$found" = 0 ] || return 10
  printf 'screens: every screen framed, paged, composed of the kit, and free of invented values\n'
}

# --- the repository as it is: every screen
expect_exit 0 screens_audit "$ROOT"
expect_grep 'every screen framed'
[ "$(ls "$ROOT"/site/templates/screens/*.html | wc -l | tr -d ' ')" -ge 8 ] || { printf '    expected the eight screens\n'; exit 1; }

# --- a fixture of the same files, broken one way at a time
mkdir -p site/templates/kit site/templates/screens site/content-src/kit site/content/kit site/data/kit share/design
cp "$ROOT"/site/templates/*.html site/templates/
cp "$ROOT"/site/templates/kit/*.html site/templates/kit/
cp "$ROOT"/site/templates/screens/*.html site/templates/screens/
cp "$ROOT"/site/content-src/kit/*.md site/content-src/kit/
cp "$ROOT"/site/content/kit/*.md site/content/kit/
cp "$ROOT"/site/data/kit/icons.toml site/data/kit/
cp "$ROOT"/share/design/kit.css "$ROOT"/share/design/primitives.css "$ROOT"/share/design/status.css share/design/
expect_exit 0 screens_audit "$T"
S=site/templates/screens/landing.html
reset() { cp "$ROOT/$S" "$S"; }

# 1. a value the design invented
printf '<p>v0.8.2</p>\n' >> "$S"
expect_exit 10 screens_audit "$T"; expect_grep 'screen landing shows a value the design invented: v0.8.2'; reset

# 2. a colour of its own
printf '<p class="text-blue-600">x</p>\n' >> "$S"
expect_exit 10 screens_audit "$T"; expect_grep 'screen landing reaches for a raw palette utility'; reset
printf '<p style="color: #bada55">x</p>\n' >> "$S"
expect_exit 10 screens_audit "$T"; expect_grep 'screen landing chooses a colour'; reset
printf '<p style="margin: 0">x</p>\n' >> "$S"
expect_exit 10 screens_audit "$T"; expect_grep 'screen landing sets a style attribute'; reset

# 3. a class the kit does not define, a component nobody declared
printf '<p class="mj-invented-look">x</p>\n' >> "$S"
expect_exit 10 screens_audit "$T"; expect_grep 'writes the class mj-invented-look'; reset
printf '{{ <hologram size="xl" /> }}\n' >> "$S"
expect_exit 10 screens_audit "$T"; expect_grep 'calls the component hologram, which nothing declares'; reset

# 4. a screen outside the frame, a screen without a page, a page without a screen
sed -i.bak '1s/.*/{% extends "base.html" %}/' "$S" && rm -f "$S.bak"
expect_exit 10 screens_audit "$T"; expect_grep 'screen landing does not extend screen.html'; reset
printf '{%% extends "screen.html" %%}\n{%% block screen %%}{%% endblock screen %%}\n' > site/templates/screens/orphan.html
expect_exit 10 screens_audit "$T"; expect_grep 'screen orphan has no page'; rm -f site/templates/screens/orphan.html
printf '+++\ntitle = "Ghost"\ntemplate = "screens/ghost.html"\n+++\nx\n' > site/content-src/kit/ghost.md
expect_exit 10 screens_audit "$T"; expect_grep 'ghost.md names a screen that has no template'; rm -f site/content-src/kit/ghost.md

# 5. a page whose icon the set does not hold
sed -i.bak 's/^icon = ".*"$/icon = "unicorn"/' site/content-src/kit/landing.md && rm -f site/content-src/kit/landing.md.bak
expect_exit 10 screens_audit "$T"; expect_grep 'names the icon "unicorn"'
cp "$ROOT"/site/content-src/kit/landing.md site/content-src/kit/
expect_exit 0 screens_audit "$T"
