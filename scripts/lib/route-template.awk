# route-template.awk — map every built route to the Zola template that rendered it.
#
# Two pages that share a template share their layout, so measuring one of them measures both.
# That is what makes a sampled probe sound, and it holds only if the sample follows the
# template. Sampling by section name looks equivalent and is not: the day a section grows a
# second template, a section-name sample silently stops covering it.
#
# Usage: awk -f route-template.awk site/content/*.md site/content/*/*.md
#   emits `route<TAB>template`, resolved in Zola's own order: the page's own `template`, else
#   its section's `page_template`, else page.html. A section's own index uses its `template`.
#
# One pass, resolved at the end, because section defaults may be read after the pages that
# need them, and because ENDFILE is a GNU extension this repository does not have.

function rel(p) { sub(/^.*\/site\/content\//, "", p); return p }
function fmv(line,   v) { v = line; sub(/^[a-z_]+[ \t]*=[ \t]*"/, "", v); sub(/".*$/, "", v); return v }

function flush(   dir) {
  if (cur == "") return
  if (cur ~ /_index\.md$/) {
    dir = cur; sub(/_index\.md$/, "", dir)
    section_tpl[dir] = ptpl
    sect[++ns] = dir; sect_own[dir] = tpl
  } else {
    files[++nf] = cur; own[cur] = tpl
  }
  cur = ""; tpl = ""; ptpl = ""
}

FNR == 1 { flush(); cur = rel(FILENAME) }

/^template[ \t]*=[ \t]*"/      { tpl = fmv($0) }
/^page_template[ \t]*=[ \t]*"/ { ptpl = fmv($0) }

END {
  flush()
  for (i = 1; i <= ns; i++) {
    d = sect[i]
    print "/" d "\t" (sect_own[d] != "" ? sect_own[d] : "section.html")
  }
  for (i = 1; i <= nf; i++) {
    f = files[i]
    dir = f; sub(/[^\/]*$/, "", dir)
    name = f; sub(/\.md$/, "", name)
    t = own[f]
    if (t == "") t = section_tpl[dir]
    if (t == "") t = "page.html"
    print "/" name "/\t" t
  }
}
