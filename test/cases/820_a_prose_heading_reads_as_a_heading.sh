# majordomus-covers: none
# A prose heading reads as a heading, not as a link.
#
# site/config.toml sets `insert_anchor_links = "heading"`, so Zola wraps the text of every
# Markdown heading in `<a class="zola-anchor">`. Inside a Flowbite Typography `format`
# container that anchor is styled like any other link: underlined, in the link colour. Every
# prose page drew its section headings as grey underlined links until each container gave the
# anchor the heading's own colour and no underline.
#
# This case holds every template that renders page content inside a `format` container to
# those utilities, so a new prose template cannot bring the link-styled headings back. It
# first proves on a fixture that a container without them is refused, and it refuses a tree
# in which it finds no such template at all, because an empty set would pass anything.
. "$ROOT/test/lib.sh"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-820.XXXXXX")"
trap 'rm -rf "$S"' EXIT

# unstyled <templates-dir>: prints `file:line` for each `format` container, in a template that
# renders page content, whose class list lacks the anchor colour or the anchor underline rule;
# prints `checked N` last, N being the number of such containers it read
unstyled() {
  find "$1" -name '*.html' -type f | sort | while IFS= read -r f; do
    grep -q 'content | safe' "$f" || continue
    # every class attribute on a line, not only the first: a wrapper and its container can
    # share one line, and reading only the wrapper would leave the container unchecked
    awk '
      {
        s = $0
        while (match(s, /class="[^"]*"/)) {
          n = split(substr(s, RSTART + 7, RLENGTH - 8), w, " ")
          s = substr(s, RSTART + RLENGTH)
          fmt = colour = plain = 0
          for (i = 1; i <= n; i++) {
            if (w[i] == "format") fmt = 1
            if (w[i] == "[&_.zola-anchor]:text-inherit") colour = 1
            if (w[i] == "[&_.zola-anchor]:no-underline") plain = 1
          }
          if (fmt) { print "container"; if (!(colour && plain)) print FILENAME ":" FNR }
        }
      }' "$f"
  done | awk '$0 == "container" { n++; next } { print } END { print "checked " n + 0 }'
}

# the instrument refuses what it exists for
mkdir -p "$S/bad"
cat > "$S/bad/page.html" <<'HTML'
<article class="format max-w-3xl">{{ page.content | safe }}</article>
HTML
out="$(unstyled "$S/bad")"
printf '%s\n' "$out" | grep -q '^.*/page.html:1$' \
  || { echo "    a format container without the anchor utilities was not refused"; echo "$out" | sed 's/^/      /'; exit 1; }

# ... also when a wrapper's class attribute comes first on the same line
mkdir -p "$S/wrapped"
cat > "$S/wrapped/page.html" <<'HTML'
<div class="min-w-0"><article class="format">{{ page.content | safe }}</article></div>
HTML
out="$(unstyled "$S/wrapped")"
printf '%s\n' "$out" | grep -q '^.*/page.html:1$' \
  || { echo "    a format container after a wrapper on the same line was not read"; echo "$out" | sed 's/^/      /'; exit 1; }

# and accepts the shape the site uses
mkdir -p "$S/good"
cat > "$S/good/page.html" <<'HTML'
<article class="format [&_.zola-anchor]:text-inherit [&_.zola-anchor]:no-underline">{{ page.content | safe }}</article>
HTML
out="$(unstyled "$S/good")"
[ "$out" = "checked 1" ] || { echo "    a styled container was refused: $out"; exit 1; }

# the site itself
out="$(unstyled "$ROOT/site/templates")"
checked="$(printf '%s\n' "$out" | sed -n 's/^checked //p')"
[ "${checked:-0}" -gt 0 ] || { echo "    no template renders page content inside a format container: nothing was checked"; exit 1; }
bad="$(printf '%s\n' "$out" | grep -v '^checked ' || true)"
if [ -n "$bad" ]; then
  echo "    a prose container draws its heading anchors as links (add [&_.zola-anchor]:text-inherit [&_.zola-anchor]:no-underline):"
  printf '%s\n' "$bad" | sed "s#^$ROOT/#      #"
  exit 1
fi
echo "    $checked prose container(s) draw their heading anchors in the heading's colour"
