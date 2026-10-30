# mermaid.awk — the issue graph as a Mermaid flowchart, one pass over model.tsv.
# usage: awk -F'\t' -v m=<milestone id or empty> -f mermaid.awk model.tsv
#        awk -v classes_only=1 -f mermaid.awk /dev/null   the class definitions alone
# Every issue of the milestone (or of the plan) as a node classed by its status, every
# dependency edge between two such issues, then the class definitions.
#
# The class definitions are the one status palette of every dependency diagram: the issue
# graph prints them here and the roadmap graph (mj_project_roadmap_mermaid) asks for them
# with classes_only, so the two cannot drift. Each class names its text colour as well as
# its stroke and fill: a class that left it out let the label follow the page's theme, and
# the dark theme drew light text on these light fills (owner report, 2026-10-16). Every fill
# is light and every text colour is the same hue's darkest step, so a label reads the same
# on a light page, a dark page, GitHub and a downloaded SVG; each pair is at least 7:1.
$1 == "I" && (m == "" || $3 == m) { keep[$2] = 1; ids[++n] = $2; st[$2] = tolower($4); t = $9; gsub(/"/, "\\&quot;", t); title[$2] = t }
$1 == "G" { ne++; from[ne] = $2; to[ne] = $3 }
END {
    if (!classes_only) {
        printf "flowchart LR\n"
        for (i = 1; i <= n; i++) printf "    %s[\"%s<br/>%s\"]:::%s\n", ids[i], ids[i], title[ids[i]], st[ids[i]]
        for (e = 1; e <= ne; e++) if (m == "" || ((from[e] in keep) && (to[e] in keep))) printf "    %s --> %s\n", from[e], to[e]
    }
    # status:stroke:fill:text
    k = split("done:#16a34a:#dcfce7:#14532d active:#2563eb:#eff6ff:#1e3a8a verify:#7c3aed:#f5f3ff:#4c1d95 ready:#0891b2:#ecfeff:#164e63 planned:#0891b2:#ecfeff:#164e63 blocked:#b45309:#fffbeb:#78350f cancelled:#6b7280:#f9fafb:#374151 superseded:#6b7280:#f9fafb:#374151", cls, " ")
    for (i = 1; i <= k; i++) { split(cls[i], c, ":"); printf "    classDef %s stroke:%s,fill:%s,color:%s,stroke-width:2px\n", c[1], c[2], c[3], c[4] }
}
