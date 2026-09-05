# majordomus-exclusive: builds the site into site/public of this checkout
# The derived site must be mobile-first and responsive. This lints the generated HTML for
# the constructs that break narrow viewports; it is static, so it catches the causes, not
# the symptom. Rendered-width checks need a browser and are out of scope for a shell test.
. "$ROOT/test/lib.sh"
command -v zola >/dev/null || { echo "    zola absent; skipping"; exit 0; }
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
expect_exit 0 "$ROOT/scripts/site-build" --no-css
out="$ROOT/site/public"
pages="$(find "$out" -name '*.html')"
[ -n "$pages" ]
bad=0
for f in $pages; do
  rel="${f#"$out"/}"
  # 1. viewport meta on every page
  grep -q '<meta name="viewport" content="width=device-width, initial-scale=1' "$f" || { echo "    $rel: no viewport meta"; bad=1; }
  # 2. every <pre> sits directly inside an overflow-x-auto wrapper, so long lines scroll
  #    inside the card instead of widening the page
  #    (compared on the whole file with newlines removed: wrapper and <pre> may sit on different lines)
  flat="$(tr -d '\n' < "$f")"
  #    Typography containers (class="format ... [&_pre]:overflow-x-auto") scroll their own <pre>; a page
  #    without such a container must wrap every <pre> in an overflow-x-auto element
  n_pre="$(printf '%s' "$flat" | grep -o '<pre' | wc -l | tr -d ' ')"
  n_wrapped="$(printf '%s' "$flat" | grep -oE 'overflow-x-auto[^>]*>[[:space:]]*<pre' | wc -l | tr -d ' ')"
  if printf '%s' "$flat" | grep -q 'class="format [^"]*\[&_pre\]:overflow-x-auto'; then :
  elif [ "$n_pre" != "$n_wrapped" ]; then echo "    $rel: $((n_pre - n_wrapped)) of $n_pre <pre> block(s) not wrapped in overflow-x-auto"; bad=1; fi
  # 3. every <table> likewise (Typography containers carry [&_table]:overflow-x-auto)
  n_tab="$(printf '%s' "$flat" | grep -o '<table' | wc -l | tr -d ' ')"
  n_tw="$(printf '%s' "$flat" | grep -oE 'overflow-x-auto[^>]*>[[:space:]]*<table' | wc -l | tr -d ' ')"
  if printf '%s' "$flat" | grep -q 'class="format [^"]*\[&_table\]:overflow-x-auto'; then :
  elif [ "$n_tab" != "$n_tw" ]; then echo "    $rel: $((n_tab - n_tw)) of $n_tab <table>(s) not wrapped in overflow-x-auto"; bad=1; fi
  # 4. mobile-first grids: three or more columns only behind a breakpoint prefix
  if grep -oE 'class="[^"]*"' "$f" | grep -qE '(class="|[[:space:]])grid-cols-([3-9]|1[0-2])([[:space:]]|")'; then
    echo "    $rel: grid-cols-3+ without a breakpoint prefix (sm:/md:/lg:)"; bad=1; fi
  # 5. no fixed pixel widths wider than a phone
  if grep -oE '\b(min-)?w-\[[0-9]{3,}px\]' "$f" | awk -F'[][]' '{gsub(/px/,"",$2); if ($2+0 > 360) found=1} END{exit !found}'; then
    echo "    $rel: fixed width over 360px"; bad=1; fi
  # 6. grid items that hold code or tables declare min-w-0, otherwise their content sets
  #    the column width (CSS grid items default to min-width:auto)
  if grep -qE 'grid[^"]*(sm|md|lg):grid-cols' "$f" && grep -q '<pre' "$f"; then
    grep -q 'min-w-0' "$f" || { echo "    $rel: grid with columns contains <pre> but no min-w-0 item"; bad=1; }
  fi
done
[ "$bad" = 0 ]
