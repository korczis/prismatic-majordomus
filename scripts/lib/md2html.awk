# md2html.awk — renders the Markdown subset used by this repository into semantic HTML.
#
# Output carries no presentation classes: it is meant to be placed inside a Flowbite
# Typography container (`format lg:format-lg dark:format-invert`), which styles headings,
# paragraphs, lists, tables, code and blockquotes. The one exception is `<pre>`, which is
# wrapped in an overflow container so long code lines scroll instead of widening the page.
#
# Supported: ATX headings, fenced code, GFM pipe tables, blockquotes, ordered and
# unordered lists (one level of nesting), horizontal rules, paragraphs, and the inline
# forms `code`, **bold**, [text](href) and bare URLs.
#
# -v linkmap="from|to;from|to"   rewrites hrefs whose value starts with `from`
# -v hlevel=2                    shifts every heading down by (hlevel-1) levels
# -v anchors=1                   emits id="..." on headings

function esc(s) {
  gsub(/&/, "\\&amp;", s); gsub(/</, "\\&lt;", s); gsub(/>/, "\\&gt;", s)
  gsub(/"/, "\\&quot;", s)
  return s
}
function slug(s,   t) {
  t = tolower(s)
  gsub(/`/, "", t); gsub(/\*/, "", t)
  gsub(/[^a-z0-9]+/, "-", t)
  gsub(/^-+/, "", t); gsub(/-+$/, "", t)
  return t
}
function rewrite(h,   n, i, parts, kv, from, to) {
  if (linkmap == "") return h
  n = split(linkmap, parts, ";")
  for (i = 1; i <= n; i++) {
    if (parts[i] == "") continue
    split(parts[i], kv, "|")
    from = kv[1]; to = kv[2]
    if (substr(h, 1, length(from)) == from) return to substr(h, length(from) + 1)
  }
  return h
}

# inline markup on an already-trimmed line; code spans are protected from further markup
function inline(s,   out, i, c, n, seg) {
  out = ""
  while ((i = index(s, "`")) > 0) {
    seg = substr(s, 1, i - 1)
    out = out markup(seg)
    s = substr(s, i + 1)
    n = index(s, "`")
    if (n == 0) { out = out "`" ; continue }
    out = out "<code>" esc(substr(s, 1, n - 1)) "</code>"
    s = substr(s, n + 1)
  }
  return out markup(s)
}
# everything except code spans
function markup(s,   out, i, j, txt, href, rest) {
  s = esc(s)
  # links: [text](href)
  out = ""
  while ((i = index(s, "[")) > 0) {
    rest = substr(s, i)
    j = index(rest, "](")
    if (j == 0) { out = out substr(s, 1, i); s = substr(s, i + 1); continue }
    txt = substr(rest, 2, j - 2)
    rest = substr(rest, j + 2)
    j = index(rest, ")")
    if (j == 0) { out = out substr(s, 1, i); s = substr(s, i + 1); continue }
    href = substr(rest, 1, j - 1)
    out = out substr(s, 1, i - 1) "<a href=\"" rewrite(href) "\">" bolds(txt) "</a>"
    s = substr(rest, j + 1)
  }
  return out bolds(s)
}
function bolds(s,   out, i, rest, j) {
  out = ""
  while ((i = index(s, "**")) > 0) {
    rest = substr(s, i + 2)
    j = index(rest, "**")
    if (j == 0) break
    out = out substr(s, 1, i - 1) "<strong>" substr(rest, 1, j - 1) "</strong>"
    s = substr(rest, j + 2)
  }
  return out s
}

# Paragraph text is buffered so that inline markup spanning a source line break still
# renders: canonical documents wrap at 88 columns, and a **bold** phrase or a [link](x)
# routinely straddles two lines.
function add_para(line) { para = (para == "" ? line : para " " line); inpara = 1 }
function close_para() {
  if (inpara) { print "<p>" inline(para) "</p>"; inpara = 0; para = "" }
}
function flush_item() { if (item != "") { print "<li>" inline(item) "</li>"; item = "" } }
function close_list() {
  if (inlist) { flush_item(); print "</" listtag ">"; inlist = 0 }
}
function close_quote() { if (inquote) { print "</blockquote>"; inquote = 0 } }
function close_all() { close_para(); close_list(); close_quote() }

function heading(level, text,   h, id) {
  close_all()
  h = level + (hlevel > 0 ? hlevel - 1 : 0)
  if (h > 6) h = 6
  id = slug(text)
  if (anchors == 1 && id != "")
    printf "<h%d id=\"%s\">%s</h%d>\n", h, id, inline(text), h
  else
    printf "<h%d>%s</h%d>\n", h, inline(text), h
}

# a GFM table row -> array of trimmed cells; returns the count
function cells(line, arr,   n, i, t) {
  sub(/^[ \t]*\|/, "", line); sub(/\|[ \t]*$/, "", line)
  n = split(line, arr, "|")
  for (i = 1; i <= n; i++) { t = arr[i]; sub(/^[ \t]+/, "", t); sub(/[ \t]+$/, "", t); arr[i] = t }
  return n
}

BEGIN { incode = 0; inpara = 0; para = ""; inlist = 0; inquote = 0; listtag = "ul" }

# ------------------------------------------------------------------ fenced code
/^[ \t]*```/ {
  if (incode) { print "</code></pre></div>"; incode = 0; next }
  close_all()
  lang = $0; sub(/^[ \t]*```[ \t]*/, "", lang); gsub(/[^A-Za-z0-9_-]/, "", lang)
  printf "<div class=\"overflow-x-auto\"><pre><code%s>", (lang != "" ? " class=\"language-" lang "\"" : "")
  incode = 1
  next
}
incode { print esc($0); next }

# ------------------------------------------------------------------ headings
/^#{1,6} / {
  match($0, /^#+/)
  lvl = RLENGTH
  txt = substr($0, lvl + 2)
  sub(/[ \t]+$/, "", txt)
  heading(lvl, txt)
  next
}

# ------------------------------------------------------------------ rules
/^[ \t]*(---+|\*\*\*+|___+)[ \t]*$/ { close_all(); print "<hr>"; next }

# ------------------------------------------------------------------ tables
/^[ \t]*\|/ {
  hdr = $0
  if ((getline sep) <= 0) { close_all(); print "<p>" inline(hdr) "</p>"; next }
  if (sep !~ /^[ \t]*\|[ \t:|-]+\|?[ \t]*$/) {
    close_all(); print "<p>" inline(hdr) "</p>"; print "<p>" inline(sep) "</p>"; next
  }
  close_all()
  print "<div class=\"overflow-x-auto\"><table>"
  n = cells(hdr, hc)
  print "<thead><tr>"
  for (i = 1; i <= n; i++) printf "<th scope=\"col\">%s</th>\n", inline(hc[i])
  print "</tr></thead><tbody>"
  while ((getline row) > 0) {
    if (row !~ /^[ \t]*\|/) break
    m = cells(row, rc)
    print "<tr>"
    for (i = 1; i <= n; i++) printf "<td>%s</td>\n", inline(i <= m ? rc[i] : "")
    print "</tr>"
  }
  print "</tbody></table></div>"
  if (row !~ /^[ \t]*\|/ && row != "") { pending = row; pendingset = 1 }
  next
}

# ------------------------------------------------------------------ blockquote
/^[ \t]*> ?/ {
  line = $0; sub(/^[ \t]*> ?/, "", line)
  close_para(); close_list()
  if (!inquote) { print "<blockquote>"; inquote = 1 }
  if (line ~ /^[ \t]*$/) { close_para(); next }
  add_para(line)
  next
}

# ------------------------------------------------------------------ lists
/^[ \t]*([-*+]|[0-9]+\.) / {
  match($0, /^[ \t]*/)
  ind = RLENGTH
  line = $0
  ordered = (line ~ /^[ \t]*[0-9]+\. /)
  sub(/^[ \t]*([-*+]|[0-9]+\.) /, "", line)
  close_para(); close_quote()
  want = ordered ? "ol" : "ul"
  if (inlist && listtag != want) close_list()
  if (!inlist) { printf "<%s>\n", want; listtag = want; inlist = 1 }
  else flush_item()
  item = line
  next
}
# continuation of a list item, indented under it
inlist && /^[ \t]+[^ \t]/ {
  line = $0; sub(/^[ \t]+/, "", line)
  item = item " " line
  next
}

# ------------------------------------------------------------------ blank
/^[ \t]*$/ {
  close_list(); close_para(); close_quote()
  next
}

# ------------------------------------------------------------------ paragraph
{
  close_list(); close_quote()
  add_para($0)
}

END {
  if (incode) print "</code></pre></div>"
  close_all()
}
