# majordomus-covers: none
# A value the site embeds as JSON cannot close the script element that holds it (I2151,
# evidence site_script_escape_case).
#
# The static site writes catalogue data into <script type="application/json"> elements with
# `json_encode() | safe`. JSON encoding leaves `</` alone, and an HTML parser ends a script
# element at the first `</script` it meets, so a value carrying that sequence would end the
# element early and the rest would be parsed as markup. Each such template replaces `</`
# with `<\/` before marking the value safe (21ea43629ce). Held here:
#
#   1  every template line that marks a json_encode() value safe escapes `</` between the two,
#      and the survey finds the three templates that do it today, so a template that moved or
#      was renamed cannot leave this case reading nothing;
#   2  the filter chain each of those lines uses, rendered by zola over a value that carries
#      `</script><script>`, leaves exactly one closing tag in the element, and the JSON reads
#      back as the value that went in (skipped where zola is absent; part 1 still runs).
#
# Read from the tracked templates and a throwaway site; nothing is built from the repository.
. "$ROOT/test/lib.sh"
fail=0
note() { printf '    %s\n' "$1"; fail=1; }

TEMPLATES="$ROOT/site/templates"
[ -d "$TEMPLATES" ] || { echo "    $TEMPLATES is missing"; exit 1; }

# 1. every json_encode() ... | safe on one line carries the escape between the two
grep -rn 'json_encode()' "$TEMPLATES" | grep '| *safe' > embeds.txt || true
count="$(wc -l < embeds.txt | tr -d ' ')"
[ "$count" -ge 3 ] || note "found $count template line(s) that mark a json_encode() value safe, expected at least 3 (partials/graph.html, api.html, why-section.html)"
for t in partials/graph.html api.html why-section.html; do
  grep -q "^$TEMPLATES/$t:" embeds.txt || note "$t no longer marks a json_encode() value safe on one line; the survey lost it"
done
while IFS= read -r line; do
  case "$line" in
    *'json_encode() | replace(from="</", to="<\\/") | safe'*) ;;
    *) note "a JSON value is marked safe without escaping </: ${line#"$ROOT/"}" ;;
  esac
done < embeds.txt

# 2. the chain, rendered
if command -v zola >/dev/null 2>&1; then
  site="$PWD/escape-site"
  mkdir -p "$site/templates" "$site/content"
  cat > "$site/config.toml" <<'EOF'
base_url = "http://127.0.0.1/"
compile_sass = false
build_search_index = false

[extra]
payload = "before</script><script>alert(1)</script>after"
EOF
  printf '+++\ntitle = "x"\n+++\n' > "$site/content/_index.md"
  # the same chain the templates use, character for character
  chain="$(grep -o 'json_encode() | replace(from="</", to="<\\\\/") | safe' "$TEMPLATES/api.html" | head -1)"
  [ -n "$chain" ] || note "could not read the filter chain from api.html"
  printf '<script type="application/json" id="d">{{ config.extra.payload | %s }}</script>\n' "$chain" \
    > "$site/templates/index.html"
  if (cd "$site" && zola build --output-dir public >zola.log 2>&1); then
    out="$site/public/index.html"
    closes="$(grep -o '</script>' "$out" | wc -l | tr -d ' ')"
    [ "$closes" = 1 ] || { note "the rendered element closes $closes time(s), expected once:"; sed 's/^/      /' "$out"; }
    body="$(sed -n 's/.*<script type="application\/json" id="d">\(.*\)<\/script>.*/\1/p' "$out")"
    back="$(printf '%s' "$body" | jq -r . 2>/dev/null)"
    [ "$back" = "before</script><script>alert(1)</script>after" ] \
      || note "the JSON in the element does not read back as the value that went in: '$back'"
  else
    note "zola could not build the fixture site:"; sed 's/^/      /' "$site/zola.log"
  fi
else
  echo "    zola absent: the rendered half is not run; the template survey above still holds"
fi

[ "$fail" = 0 ]
