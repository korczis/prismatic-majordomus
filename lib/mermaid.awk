# mermaid.awk — the issue graph as a Mermaid flowchart, one pass over model.tsv.
# usage: awk -F'\t' -v m=<milestone id or empty> -f mermaid.awk model.tsv
# Every issue of the milestone (or of the plan) as a node classed by its status, every
# dependency edge between two such issues, then the class definitions. Byte for byte what
# the per-issue shell loop printed, without a process per issue and per edge.
$1 == "I" && (m == "" || $3 == m) { keep[$2] = 1; ids[++n] = $2; st[$2] = tolower($4); t = $9; gsub(/"/, "\\&quot;", t); title[$2] = t }
$1 == "G" { ne++; from[ne] = $2; to[ne] = $3 }
END {
    printf "flowchart LR\n"
    for (i = 1; i <= n; i++) printf "    %s[\"%s<br/>%s\"]:::%s\n", ids[i], ids[i], title[ids[i]], st[ids[i]]
    for (e = 1; e <= ne; e++) if (m == "" || ((from[e] in keep) && (to[e] in keep))) printf "    %s --> %s\n", from[e], to[e]
    split("done:#16a34a:#052e16 active:#2563eb:#eff6ff verify:#7c3aed:#f5f3ff ready:#0891b2:#ecfeff blocked:#b45309:#fffbeb cancelled:#6b7280:#f9fafb", cls, " ")
    for (i = 1; i <= 6; i++) { split(cls[i], c, ":"); printf "    classDef %s stroke:%s,fill:%s,stroke-width:2px\n", c[1], c[2], c[3] }
}
