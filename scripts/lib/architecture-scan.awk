# architecture-scan.awk — static reference scan of the CLI's shell sources.
#
# The architecture graph on the site is not drawn by hand. It is what this scanner can
# prove by reading bin/majordomus and lib/*.sh: which module sources which, and which
# files of the repository's AI layer each module names, reads or writes. Anything it cannot
# prove is absent from the graph rather than guessed at.
#
# Usage: awk -f architecture-scan.awk -v table="VAR=path VAR=path ..." vars=1 <files...> vars=0 <same files...>
#   table  the layout as the tool resolves it (`mj_layout_table`): each path variable the
#          sources use, with the repository-relative directory or file it names. The scanner
#          knows no path of its own; a layout change reaches the graph through this table.
#   pass 1 collects the three ways this codebase names a state path — the shell variable
#   (`MJ_CUR="$MJ_STATE_DIR/current.yaml"`), the local (`local dir="$MJ_STATE_DIR/..."`) and
#   the accessor function (`mj_question_file() { printf '%s' "$MJ_STATE_DIR/..."; }`);
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
  # the layout table: base[VAR] = repository-relative path; roots = its top-level directories
  nt = split(table, tt, " ")
  for (i = 1; i <= nt; i++) {
    if (tt[i] == "") continue
    eq = index(tt[i], "="); if (eq == 0) continue
    base[substr(tt[i], 1, eq - 1)] = substr(tt[i], eq + 1)
    r = substr(tt[i], eq + 1); sub(/\/.*$/, "", r); roots[r] = 1
  }
}

function rel(p) { sub(/^.*\/prismatic-majordomus[^\/]*\//, "", p); return p }

# a `$VAR/rest` or `${VAR}/rest` reference resolved through the table, or ""
function resolve(ref,   name, rest) {
  name = ref; sub(/^\$\{?/, "", name); sub(/\}?(\/.*)?$/, "", name)
  rest = ref; if (index(rest, "/") > 0) sub(/^[^\/]*/, "", rest); else rest = ""
  if (name in base) return base[name] rest
  return ""
}

function artifact(p,   q, r) {
  q = p
  sub(/\.mj-tmp$/, "", q)          # the staging half of a transactional write
  sub(/\/+$/, "", q)
  if (q ~ /\$|\*/) sub(/\/[^\/]*[$*].*$/, "", q)   # built from a variable: keep the fixed prefix
  r = q; sub(/\/.*$/, "", r)
  if (!(r in roots)) return ""
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

{ file = rel(FILENAME); line = $0; sub(/[[:space:]]*#.*$/, "", line)
  # the resolver is the one place that defines the layout; it touches no file, and scanning
  # it would register every branch of every layout as an artifact
  if (line ~ /^mj_resolve_layout\(\)/) in_layout = 1
  if (in_layout) { if (line ~ /^\}/) in_layout = 0; next } }

vars == 1 {
  if (match(line, /^[[:space:]]*MJ_[A-Z_]+=/)) { lhs = substr(line, RSTART, RLENGTH); gsub(/[[:space:]=]/, "", lhs); if (lhs in base) next }
  # `MJ_CUR="$MJ_STATE_DIR/current.yaml"`: a file-independent name for a layout path
  if (match(line, /^[[:space:]]*MJ_[A-Z_]+="\$\{?MJ_[A-Z_]+\}?(\/[^"]*)?"/)) {
    a = substr(line, RSTART, RLENGTH); sub(/^[[:space:]]*/, "", a)
    name = a; sub(/=.*$/, "", name)
    val = a; sub(/^[^=]*="/, "", val); sub(/"$/, "", val)
    rp = resolve(val); if (rp != "") var[name] = rp
  }
  # `local dir="$MJ_STATE_DIR/..."`, `archive="$MJ_STATE_DIR/..."`; the name is file-scoped
  if (match(line, /(^|[[:space:]])[a-z_][a-z0-9_]*="\$\{?MJ_[A-Z_]+\}?(\/[^"]*)?"/)) {
    a = substr(line, RSTART, RLENGTH); sub(/^[[:space:]]/, "", a)
    name = a; sub(/=.*$/, "", name)
    val = a; sub(/^[^=]*="/, "", val); sub(/"$/, "", val)
    rp = resolve(val); if (rp != "") var[file SUBSEP name] = rp
  }
  # `mj_question_file() { printf '%s' "$MJ_STATE_DIR/open-questions.md"; }`
  if (match(line, /^[a-z_][a-z0-9_]*\(\)[[:space:]]*\{[[:space:]]*printf[^"]*"\$\{?MJ_[A-Z_]+\}?(\/[^"]*)?"/)) {
    name = line; sub(/\(\).*$/, "", name)
    val = line; sub(/^[^"]*"/, "", val); sub(/".*$/, "", val)
    rp = resolve(val); if (rp != "") fn[name] = rp
  }
  next
}

line != "" {
  emit_node(file, file == "bin/majordomus" ? "entry" : "module")
  # the layout definitions themselves: a line assigning a table variable describes the
  # layout rather than touching a file, and would otherwise register every branch of it
  if (match(line, /^[[:space:]]*MJ_[A-Z_]+=/)) { lhs = substr(line, RSTART, RLENGTH); gsub(/[[:space:]=]/, "", lhs); if (lhs in base) next }

  if (match(line, /\.[[:space:]]+"\$MJ_LIB_DIR\/[A-Za-z0-9_]+\.sh"/)) {
    dep = substr(line, RSTART, RLENGTH)
    sub(/^.*MJ_LIB_DIR\//, "", dep); sub(/"$/, "", dep)
    emit_node("lib/" dep, "module")
    emit_edge(file, "lib/" dep, "sources")
  }

  rest = line
  while (match(rest, /\$\{?MJ_[A-Z_]+\}?"?\/[A-Za-z0-9_.\/${}*<>-]+|\$\{?[A-Za-z_][A-Za-z0-9_]*\}?|\$\([a-z_][a-z0-9_]*\)/)) {
    ref = substr(rest, RSTART, RLENGTH)
    before = substr(rest, 1, RSTART - 1)
    rest = substr(rest, RSTART + RLENGTH)

    path = ""
    if (ref ~ /^\$\{?MJ_[A-Z_]+\}?"?\//) { q = ref; sub(/"\//, "/", q); path = resolve(q) }
    else if (ref ~ /^\$\(/) { name = ref; gsub(/[$()]/, "", name); if (name in fn) path = fn[name] }
    else {
      name = ref; gsub(/[${}]/, "", name)
      if ((file SUBSEP name) in var) path = var[file SUBSEP name]
      else if (name in var) path = var[name]
      else if (name in base) path = base[name]
    }
    path = artifact(path)
    if (path == "") continue

    record(file, path, direction(before))
  }
}
