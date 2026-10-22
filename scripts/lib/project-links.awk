# project-links.awk: resolves every relative Markdown link of one projected document against the
# directory of the file it came from, and rewrites it to where a reader of the site can follow it.
# A canonical document is written for GitHub, where `../share/commands.yaml` and `SCHEMAS.md` resolve
# beside the file; projected under site/content they resolve against the page's URL instead, which is
# how 197 links on the published site came to 404. Rule: project.every-link-and-control-is-tested;
# scripts/ci/link-check is what refuses a link this leaves broken.
#
#   -v src=PATH       the document's path in the repository (docs/MCP.md, docs/claims/x.md, AGENTS.md)
#   -v repo=URL       the forge URL of the repository, from site/data/generated/project.json
#   -v tracked=FILE   `git ls-files`, one path per line
#   -v docs=FILE      "<repository path>\t<route>" for every document that has a page (docs/MCP.md\t/docs/mcp/)
#   -v mode=zola|url  zola: internal links as @/ content paths (projected content, checked by the build);
#                     url: as absolute site URLs, for fragments rendered by the markdown filter
#   -v base=URL       the site base, for mode=url
#
# A link target is resolved to a repository path, then:
#   a document with a page      -> its route        (docs/MCP.md, docs/claims/<id>.md, AGENTS.md, docs/CLAIMS.yaml)
#   a tracked file              -> <repo>/blob/master/<path>
#   a tracked directory         -> <repo>/tree/master/<path>
#   anything else               -> the link text alone: git does not track it (checkout-local state), so
#                                  no forge and no page can answer for it
# Absolute URLs, @/ links, #fragments, mailto: and anything inside a fenced code block are untouched.
function dirname_of(p,   i) { i = match(p, /\/[^\/]*$/); return i ? substr(p, 1, RSTART - 1) : "" }
function normalise(p,   n, parts, out, i, k) {
  n = split(p, parts, "/"); k = 0
  for (i = 1; i <= n; i++) {
    if (parts[i] == "" || parts[i] == ".") continue
    if (parts[i] == "..") { if (k == 0) return "\001"; k--; continue }
    out[++k] = parts[i]
  }
  p = ""; for (i = 1; i <= k; i++) p = p (i > 1 ? "/" : "") out[i]
  return p
}
function internal(route, frag,   path) {
  if (mode == "url") return base route (frag != "" ? "#" frag : "")
  # a section's page is its _index.md, every other route a page of its own (zola resolves @/ content paths)
  path = route; sub(/^\//, "", path); sub(/\/$/, "", path)
  if (path == "" || (route in section)) return "@/" (path == "" ? "" : path "/") "_index.md" (frag != "" ? "#" frag : "")
  return "@/" path ".md" (frag != "" ? "#" frag : "")
}
function resolve(target,   path, frag, h, r) {
  frag = ""; h = index(target, "#")
  if (h) { frag = substr(target, h + 1); target = substr(target, 1, h - 1) }
  if (target == "") return "\002"
  r = normalise((dir != "" ? dir "/" : "") target)
  if (r == "\001") return "\002"
  if (r in route_of) return internal(route_of[r], frag)
  if (r ~ /^docs\/claims\/[^\/]+\.md$/ && (r in is_file)) { sub(/^docs\/claims\//, "", r); sub(/\.md$/, "", r); return internal("/guarantees/" r "/", frag) }
  if (r in is_file) return repo "/blob/master/" r (frag != "" ? "#" frag : "")
  if (r in is_dir) return repo "/tree/master/" r
  return "\003"
}
BEGIN {
  dir = dirname_of(src)
  while ((getline line < tracked) > 0) {
    if (line == "") continue
    is_file[line] = 1
    n = split(line, seg, "/"); p = ""
    for (i = 1; i < n; i++) { p = p (i > 1 ? "/" : "") seg[i]; is_dir[p] = 1 }
  }
  close(tracked)
  FS_saved = FS
  while ((getline line < docs) > 0) { t = index(line, "\t"); if (t) route_of[substr(line, 1, t - 1)] = substr(line, t + 1) }
  close(docs)
  route_of["AGENTS.md"] = "/docs/contract/"
  route_of["docs/CLAIMS.yaml"] = "/guarantees/"
  route_of["docs/README.md"] = "/docs/"
  # the routes above that are sections rather than pages
  section["/docs/"] = 1; section["/guarantees/"] = 1
  fence = 0
}
/^[ \t]*(```|~~~)/ { fence = !fence; print; next }
fence { print; next }
{
  out = ""; rest = $0
  while (match(rest, /\]\([^)[:space:]]+\)/)) {
    target = substr(rest, RSTART + 2, RLENGTH - 3)
    head = substr(rest, 1, RSTART - 1); tail = substr(rest, RSTART + RLENGTH)
    if (target ~ /^([a-zA-Z][a-zA-Z0-9+.-]*:|#|@\/|\/)/) { out = out head "](" target ")"; rest = tail; continue }
    to = resolve(target)
    if (to == "\002") { out = out head "](" target ")"; rest = tail; continue }
    if (to == "\003") {
      # unlink: drop the matching "[" before the label and the "(target)" after it
      depth = 0; cut = 0
      for (i = length(head); i >= 1; i--) {
        c = substr(head, i, 1)
        if (c == "]") depth++
        else if (c == "[") { if (depth == 0) { cut = i; break } depth-- }
      }
      if (cut) { out = out substr(head, 1, cut - 1) substr(head, cut + 1); rest = tail; continue }
      out = out head "](" target ")"; rest = tail; continue
    }
    out = out head "](" to ")"; rest = tail
  }
  print out rest
}
