# majordomus-exclusive: builds the site into site/public of this checkout
# The derived site must be mobile-first and responsive. This lints the generated HTML for
# the constructs that break narrow viewports; it is static, so it catches the causes, not
# the symptom. Rendered-width checks need a browser and are out of scope for a shell test.
. "$ROOT/test/lib.sh"
command -v zola >/dev/null || { echo "    zola absent; skipping"; exit 0; }
command -v jq >/dev/null || { echo "    jq absent; skipping"; exit 0; }
# `--no-css` skips the Node steps, which is what this case wants: it lints markup and never
# looks at a stylesheet. But the Zola build itself resolves `get_url(path="app.css")` in
# base.html, and `site/static/app.css` is generated and gitignored — absent in any fresh
# checkout. So `--no-css` succeeds only when some earlier case in the same run happened to
# leave the file behind, which made this case pass by accident of ordering and fail the day
# a new exclusive case changed it. Build the stylesheet once when it is missing, and keep the
# fast path when it is not.
if [ -f "$ROOT/site/static/app.css" ]; then
  expect_exit 0 "$ROOT/scripts/site-build" --no-css
elif [ -x "$ROOT/node_modules/.bin/tailwindcss" ]; then
  expect_exit 0 "$ROOT/scripts/site-build"
else
  echo "    node_modules absent and no stylesheet to reuse; skipping"; exit 0
fi
out="$ROOT/site/public"
# The pages linted here are the site's own. scripts/site-build also composes the other
# published static surfaces into the build at their mounts (ADR 0086) — the crate's rustdoc at
# /rustdoc — whose markup is another producer's and is judged by that surface's own check
# (`majordomus quality rustdoc`), as scripts/site-check prunes them from its page contract.
# Which mounts they are is the committed topology's answer, read through the same helper.
composed="$(. "$ROOT/lib/common.sh"; mj_web_composed "$ROOT" mounts)" \
  || { echo "    the topology cannot say which pages are the site's own"; exit 1; }
prune=()
for m in $composed; do prune+=(-path "$out$m" -prune -o); done
pages="$(find "$out" ${prune[@]+"${prune[@]}"} -name '*.html' -print)"
[ -n "$pages" ]
bad=0
for f in $pages; do
  rel="${f#"$out"/}"
  # a route that moved is published as Zola's alias stub: a title, a meta refresh and a
  # script, and deliberately none of a page's landmarks. It is a redirect, not a page.
  grep -q '<title>Redirect</title>' "$f" && continue
  # 1. viewport meta on every page
  grep -q '<meta name="viewport" content="width=device-width, initial-scale=1' "$f" || { echo "    $rel: no viewport meta"; bad=1; }
  # 2. every <pre> scrolls or wraps on its own, so a long line scrolls inside its card instead
  #    of widening the page. Counted per <pre>, and one is satisfied by any of:
  #      - an element whose own class carries overflow-x-auto, with only whitespace before it
  #      - an enclosing div, article or section whose class carries [&_pre]:overflow-x-auto.
  #        Tailwind compiles that variant to `.[&_pre]:overflow-x-auto pre{overflow-x:auto}`,
  #        so each <pre> inside scrolls itself. Typography containers (class="format ...")
  #        carry it, and so does the block a rendered doc comment sits in, where the <pre> of
  #        a code fence cannot be given a wrapper of its own
  #      - whitespace-pre-wrap on the <pre> itself: a <pre> that wraps cannot widen the page,
  #        and the shared install component uses it so that a command a person has to read is
  #        never behind a scrollbar
  #    This used to be decided per page: one "format" container anywhere exempted every <pre>
  #    on it, including those outside the container, while the same variant on a block without
  #    the "format" class was not recognised at all.
  # scripts/lib/pre-coverage.awk is the rule, shared with site-check: "<file> <pre> <covered>"
  pre_coverage="$(awk -f "$ROOT/scripts/lib/pre-coverage.awk" "$f")"
  n_pre="$(printf '%s' "$pre_coverage" | cut -f2)"; n_covered="$(printf '%s' "$pre_coverage" | cut -f3)"
  [ "$n_pre" = "$n_covered" ] || { echo "    $rel: $((n_pre - n_covered)) of $n_pre <pre> block(s) neither scroll nor wrap"; bad=1; }
  # the checks below compare on the whole file with newlines removed: a wrapper and the element
  # it wraps may sit on different lines
  flat="$(tr -d '\n' < "$f")"
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
