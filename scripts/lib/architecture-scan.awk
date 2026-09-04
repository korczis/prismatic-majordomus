# architecture-scan.awk — static reference scan of the CLI's shell sources.
#
# The architecture graph on the site is not drawn by hand. It is what this scanner can
# prove by reading bin/majordomus and lib/*.sh: which module sources which, and which
# files under .majordomus/ each module names, read or written. Anything it cannot prove
# is absent from the graph rather than guessed at.
#
# Usage: awk -f architecture-scan.awk vars=1 <files...> vars=0 <same files...>
#   pass 1 collects the three ways this codebase names a state path — the shell variable
#   (`MJ_CUR="$MJ_DIR/state/current.yaml"`), the local (`local dir="$MJ_DIR/..."`) and the
#   accessor function (`mj_question_file() { printf '%s' "$MJ_DIR/..."; }`);
#   pass 2 emits TSV: `edge<TAB>from<TAB>to<TAB>kind` and `node<TAB>id<TAB>kind`.
#
# Direction is decided from the reference's own line: the redirection it sits behind, or the
# command word that introduces it. A reference the scanner cannot place either way — a path
# handed to a helper, `mj_publish_record "$dir"` — is emitted as `uses` rather than guessed
# at, because a misread edge would be a claim the sources do not support.

BEGIN {
  split("mv cp rm mkdir touch chmod install ln", cw, " "); for (i in cw) writers[cw[i]] = 1
  split("cat grep sed awk head tail wc sort jq cut test [ read source . ls find diff cmp shasum", cr, " ")
  for (i in cr) readers[cr[i]] = 1
  rank["uses"] = 1; rank["reads"] = 2; rank["writes"] = 3; rank["sources"] = 4
}

function rel(p) { sub(/^.*\/prismatic-majordomus[^\/]*\//, "", p); return p }

function artifact(p,   q) {
  q = p
  sub(/\.mj-tmp$/, "", q)          # the staging half of a transactional write
  sub(/\/+$/, "", q)
  if (q ~ /\$|\*/) sub(/\/[^\/]*[$*].*$/, "", q)   # built from a variable: keep the fixed prefix
  if (q !~ /^\.majordomus\//) return ""
  return q
}

# Nodes and edges are buffered so that the strongest direction observed for a pair is the
# one reported: a module that both guards on a file and rewrites it writes it.
function emit_node(id, kind) { if (!(id in nodes)) { nodes[id] = kind; order[++n] = id } }
function emit_edge(a, b, k,   key) {
  key = a SUBSEP b
  if (!(key in edges)) { edges[key] = k; epair[++m] = key }
  else if (rank[k] > rank[edges[key]]) edges[key] = k
}
function record(id, ref, kind) { emit_node(ref, "artifact"); emit_edge(id, ref, kind) }

END {
  for (i = 1; i <= n; i++) printf "node\t%s\t%s\n", order[i], nodes[order[i]]
  for (i = 1; i <= m; i++) {
    split(epair[i], p, SUBSEP)
    printf "edge\t%s\t%s\t%s\n", p[1], p[2], edges[epair[i]]
  }
}

# the command word a reference sits behind, e.g. `if ! grep -q x "$MJ_CUR"` -> `grep`
function head_word(before,   t) {
  t = before
  gsub(/\$\([^)]*\)/, " ", t)                        # a substitution is not the caller
  sub(/^.*[;|&(){}][[:space:]]*/, "", t)
  sub(/^[[:space:]]*(!|then|do|else|elif|if|while|until|local|export)[[:space:]]+/, "", t)
  sub(/^[[:space:]]+/, "", t)
  sub(/[[:space:]].*$/, "", t)
  return t
}

function direction(before,   w) {
  if (before ~ /(>|>>)[[:space:]]*"?$/) return "writes"
  if (before ~ /<[[:space:]]*"?$/) return "reads"
  w = head_word(before)
  if (w in writers) return "writes"
  if (w in readers) return "reads"
  return "uses"
}

{ file = rel(FILENAME); line = $0; sub(/[[:space:]]*#.*$/, "", line) }

vars == 1 {
  if (line ~ /^[[:space:]]*MJ_[A-Z_]+="\$MJ_DIR\//) {
    name = line; sub(/^[[:space:]]*/, "", name); sub(/=.*$/, "", name)
    val = line; sub(/^[^=]*="\$MJ_DIR\//, "", val); sub(/".*$/, "", val)
    var[name] = ".majordomus/" val
  }
  # `local dir="$MJ_DIR/..."`, `archive="$MJ_DIR/..."`; the name is file-scoped
  if (match(line, /(^|[[:space:]])[a-z_][a-z0-9_]*="\$MJ_DIR\/[^"]*"/)) {
    a = substr(line, RSTART, RLENGTH); sub(/^[[:space:]]/, "", a)
    name = a; sub(/=.*$/, "", name)
    val = a; sub(/^[^=]*="\$MJ_DIR\//, "", val); sub(/"$/, "", val)
    var[file SUBSEP name] = ".majordomus/" val
  }
  # `mj_question_file() { printf '%s' "$MJ_DIR/state/open-questions.md"; }`
  if (match(line, /^[a-z_][a-z0-9_]*\(\)[[:space:]]*\{[[:space:]]*printf[^"]*"\$MJ_DIR\/[^"]*"/)) {
    name = line; sub(/\(\).*$/, "", name)
    val = line; sub(/^.*"\$MJ_DIR\//, "", val); sub(/".*$/, "", val)
    fn[name] = ".majordomus/" val
  }
  next
}

line != "" {
  emit_node(file, file == "bin/majordomus" ? "entry" : "module")

  if (match(line, /\.[[:space:]]+"\$MJ_LIB_DIR\/[A-Za-z0-9_]+\.sh"/)) {
    dep = substr(line, RSTART, RLENGTH)
    sub(/^.*MJ_LIB_DIR\//, "", dep); sub(/"$/, "", dep)
    emit_node("lib/" dep, "module")
    emit_edge(file, "lib/" dep, "sources")
  }

  rest = line
  while (match(rest, /\$\{?MJ_DIR\}?\/[A-Za-z0-9_.\/${}*<>-]+|\$\{?[A-Za-z_][A-Za-z0-9_]*\}?|\$\([a-z_][a-z0-9_]*\)/)) {
    ref = substr(rest, RSTART, RLENGTH)
    before = substr(rest, 1, RSTART - 1)
    rest = substr(rest, RSTART + RLENGTH)

    path = ""
    if (ref ~ /^\$\{?MJ_DIR\}?\//) { path = ref; sub(/^\$\{?MJ_DIR\}?\//, ".majordomus/", path) }
    else if (ref ~ /^\$\(/) { name = ref; gsub(/[$()]/, "", name); if (name in fn) path = fn[name] }
    else {
      name = ref; gsub(/[${}]/, "", name)
      if ((file SUBSEP name) in var) path = var[file SUBSEP name]
      else if (name in var) path = var[name]
    }
    path = artifact(path)
    if (path == "") continue

    record(file, path, direction(before))
  }
}
