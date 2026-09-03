# project-markdown.awk — projects GitHub-native Markdown into site components. Runs only on the
# DERIVED copy in site/content/docs/; canonical files stay readable on GitHub untouched.
#   ```mermaid fences                     -> <pre class="mermaid"> (client-side render; source visible without JS)
#   > [!NOTE|TIP|IMPORTANT|WARNING|CAUTION] -> Flowbite alert (flowbite.com/docs/components/alerts/)
#   | tables |                             -> wrapped in <div class="overflow-x-auto">
function flush_table() { if (intable) { print ""; print "</div>"; print ""; intable = 0 } }
function alert_open(kind,   cls, label) {
  if (kind == "NOTE")           { cls = "border-brand-subtle bg-brand-softer text-fg-brand-strong"; label = "Note" }
  else if (kind == "TIP")       { cls = "border-success-subtle bg-success-soft text-fg-success-strong"; label = "Tip" }
  else if (kind == "IMPORTANT") { cls = "border-brand-subtle bg-brand-softer text-fg-brand-strong"; label = "Important" }
  else if (kind == "WARNING")   { cls = "border-warning-subtle bg-warning-soft text-fg-warning"; label = "Warning" }
  else                          { cls = "border-danger-subtle bg-danger-soft text-fg-danger-strong"; label = "Caution" }
  print "<div class=\"not-format my-6 flex gap-3 rounded-base border p-4 text-sm " cls "\" role=\"note\">"
  print "<span class=\"font-mono text-xs font-semibold uppercase tracking-wide\">" label "</span>"
  print "<div class=\"min-w-0 flex-1\">"
  print ""
}
BEGIN { infence = 0; inmermaid = 0; intable = 0; inalert = 0 }
/^```mermaid[ \t]*$/ && !infence { flush_table(); print "<pre class=\"mermaid\">"; inmermaid = 1; infence = 1; next }
/^```/ && infence { if (inmermaid) { print "</pre>"; print ""; inmermaid = 0 } else print; infence = 0; next }
/^```/ && !infence { flush_table(); infence = 1; print; next }
infence { if (inmermaid) { gsub(/&/, "\\&amp;"); gsub(/</, "\\&lt;"); gsub(/>/, "\\&gt;") } print; next }
/^> \[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\][ \t]*$/ { flush_table(); match($0, /\[![A-Z]+\]/); alert_open(substr($0, RSTART + 2, RLENGTH - 3)); inalert = 1; next }
inalert && /^> ?/ { sub(/^> ?/, ""); print; next }
inalert { print ""; print "</div></div>"; print ""; inalert = 0 }
/^\|/ { if (!intable) { print "<div class=\"overflow-x-auto\">"; print ""; intable = 1 } print; next }
intable && !/^\|/ { flush_table() }
{ print }
END { if (inalert) { print ""; print "</div></div>" } flush_table(); if (inmermaid) print "</pre>" }
