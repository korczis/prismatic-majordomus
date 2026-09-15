# For every page given, one line: "<file>\t<pre count>\t<covered count>". A <pre> is covered when a
# long line in it cannot widen the page, which is true for any of:
#   - an element whose own class carries overflow-x-auto, with only whitespace before the <pre>
#   - an enclosing div, article or section whose class carries [&_pre]:overflow-x-auto. Tailwind
#     compiles that variant to `.[&_pre]:overflow-x-auto pre{overflow-x:auto}`, so each <pre>
#     inside scrolls itself: Typography containers carry it, and so does the block a rendered doc
#     comment sits in, where a code fence's <pre> cannot be given a wrapper of its own
#   - whitespace-pre-wrap on the <pre> itself
# Judged per <pre>. A page-level flag — one wrapped <pre>, or one Typography container anywhere,
# excusing every other — let a page of unwrapped listings pass. The tags are split on "<" rather
# than read up to ">", because a class such as [&_:not(pre)>code] carries a ">" of its own.
# Used by scripts/site-check and test/cases/09_site_mobile_first.sh, so they cannot disagree.
function report() { if (file != "") printf "%s\t%d\t%d\n", file, pre, covered }
BEGIN { RS = "<" }
FNR == 1 { report(); file = FILENAME; depth = 0; open = 0; pre = 0; covered = 0; wrapped = 0; split("", flagged) }
/^[a-zA-Z]/ { direct = ($0 ~ /^[a-zA-Z][a-zA-Z0-9]*([ \t\n]+[a-zA-Z-]+="[^"]*")*[ \t\n]+class="([^"]*[ \t\n])?overflow-x-auto([ \t\n][^"]*)?"[^>]*>[ \t\n]*$/) }
/^(div|article|section)[ \t\n>]/ {
  depth++; flagged[depth] = ($0 ~ /^(div|article|section)([ \t\n]+[a-zA-Z-]+="[^"]*")*[ \t\n]+class="[^"]*\[&_pre\]:overflow-x-auto/)
  open += flagged[depth]; wrapped = direct; next }
/^\/(div|article|section)[ \t\n>]/ { if (depth > 0) { open -= flagged[depth]; depth-- }; wrapped = 0; next }
/^pre[ \t\n>]/ { pre++; if (open > 0 || wrapped || $0 ~ /^pre[^>]*whitespace-pre-wrap/) covered++; wrapped = 0; next }
{ wrapped = ($0 ~ /^[a-zA-Z]/) ? direct : 0 }
END { report() }
