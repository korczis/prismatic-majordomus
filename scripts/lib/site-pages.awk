# site-pages.awk — every per-page assertion of scripts/site-check, in one process.
#
# The checks it carries used to be five shell loops over site/public/**/*.html, each spawning
# grep, sed and awk per page: about eighteen processes for every route, seven thousand for the
# site, and most of site-check's wall clock spent on fork and exec rather than on reading HTML.
# The assertions are unchanged — this file is a transcription, not a redesign — and site-check
# still reports them in its own order, from the rows below.
#
#   awk -v pub=<site/public> -f site-pages.awk <page>...
#
# One row per finding, TAB separated:
#
#   <page relative to pub>  <check>  <detail>
#
# checks: viewport description main h1 placeholder inlinestyle initflowbite mermaid
#         pre gridcols fixedwidth nojs
#
# A page with nothing wrong produces no row. The detail is what site-check prints after the
# route; a check whose message needs no detail leaves it empty. Rows come in the order the
# pages were given, so site-check's output is the order it has always had.

function flush() {
  if (file == "") return
  if (!has_viewport)    print rel "\tviewport\t"
  if (!has_description) print rel "\tdescription\t"
  if (!has_main)        print rel "\tmain\t"
  if (n_h1 != 1)        print rel "\th1\t" n_h1
  if (placeholder)      print rel "\tplaceholder\t"
  if (inline_style)     print rel "\tinlinestyle\t"
  if (n_initflowbite != 1) print rel "\tinitflowbite\t" n_initflowbite
  if (has_mermaid && !has_mermaid_js) print rel "\tmermaid\t"
  if (n_pre > 0 && !wrapped) print rel "\tpre\t"
  if (gridcols)   print rel "\tgridcols\t"
  if (fixedwidth) print rel "\tfixedwidth\t"
  if (nojs > 0)   print rel "\tnojs\t" nojs
}

# the number of times needle occurs in s, which is what `grep -o ... | wc -l` counted
function count(s, needle,   n, i) {
  n = 0; i = index(s, needle)
  while (i > 0) { n++; s = substr(s, i + length(needle)); i = index(s, needle) }
  return n
}

FNR == 1 {
  flush()
  file = FILENAME; rel = file
  sub("^" pub "/", "", rel)
  has_viewport = has_description = has_main = 0
  n_h1 = n_initflowbite = n_pre = nojs = 0
  placeholder = inline_style = gridcols = fixedwidth = 0
  has_mermaid = has_mermaid_js = 0
  in_pre = 0; wrapped = 0; seen_format = 0; prev = ""
}

{
  line = $0

  if (index(line, "<meta name=\"viewport\""))    has_viewport = 1
  if (index(line, "<meta name=\"description\"")) has_description = 1
  if (index(line, "<main"))                      has_main = 1
  if (index(line, "class=\"mermaid"))            has_mermaid = 1
  if (index(line, "js/mermaid.min.js"))          has_mermaid_js = 1
  if (index(line, " style=\""))                  inline_style = 1
  n_h1 += count(line, "<h1")
  n_initflowbite += count(line, "initFlowbite()")
  n_pre += count(line, "<pre")

  # An unrendered template delimiter, outside <pre> blocks and outside code spans: what Zola
  # produced inside a <pre> or a <code> is rendered output, not a template it failed to expand.
  if (index(line, "<pre")) in_pre = 1
  else if (!in_pre) {
    stripped = line
    gsub(/<code[^>]*>[^<]*<\/code>/, "", stripped)
    if (stripped ~ /\{\{|\{%|\[\[[A-Z_]+\]\]/) placeholder = 1
  }
  if (index(line, "</pre>")) in_pre = 0

  # A <pre> needs an overflow container. The original test ran over the page with its newlines
  # removed, so a wrapper and the <pre> it wraps count as adjacent across a line break: hence
  # the same-line match, the previous-line match, and the Typography container, which the
  # plugin's own rule makes scroll wherever the <pre> falls after it.
  if (index(line, "<pre")) {
    if (line ~ /overflow-x-auto[^>]*>[ \t\r]*<pre/) wrapped = 1
    if (prev ~ /overflow-x-auto[^>]*>[ \t\r]*$/ && line ~ /^[ \t\r]*<pre/) wrapped = 1
    if (line ~ /class="format[^"]*"[^>]*>.*<pre/) wrapped = 1
    if (seen_format) wrapped = 1
  }
  if (line ~ /class="format[^"]*"[^>]*>/) seen_format = 1

  # A grid of three or more columns with no breakpoint prefix, over the class attributes only.
  rest = line
  while (match(rest, /class="[^"]*"/)) {
    tok = substr(rest, RSTART, RLENGTH)
    if (tok ~ /(class="|[ \t])grid-cols-([3-9]|1[0-2])([ \t]|")/) gridcols = 1
    rest = substr(rest, RSTART + RLENGTH)
  }

  # A width fixed above the narrowest phone this site supports.
  rest = line
  while (match(rest, /(^|[^A-Za-z0-9_])(min-)?w-\[[0-9][0-9][0-9]+px\]/)) {
    tok = substr(rest, RSTART, RLENGTH)
    px = tok; sub(/^.*\[/, "", px); sub(/px\].*$/, "", px)
    if (px + 0 > 360) fixedwidth = 1
    rest = substr(rest, RSTART + RLENGTH)
  }

  # `tr '<' '\n' | grep x-show | grep -cE ...`: the unit is the page split on "<" and on its
  # own newlines, so a fragment never crosses a line and the count is exact per line. An
  # element hidden by a runtime *and* by a static rule is unreachable without JavaScript.
  n = split(line, parts, "<")
  for (i = 1; i <= n; i++)
    if (parts[i] ~ /x-show/ && parts[i] ~ /x-cloak|class="[^"]*[ ]hidden[ "]/) nojs++

  prev = line
}

END { flush() }
