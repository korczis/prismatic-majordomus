#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_context_docs:-}" ] && return 0 || MJ_LIB_context_docs=1
# context documents — hierarchical, directory-scoped context under the AI layer.
#
# A context document is a Markdown file under the layer's tree (the manifest's directory,
# minus the half the manifest declares untracked, minus the subtrees a contract exempts)
# whose front matter declares the contract `schema: context/v1`, `kind: context`. The file name is a convention, never the
# identity: `id` is, and it survives a move. The manifest may name file conventions
# (`context.documents`) that must carry the contract wherever they appear in the tree, so a
# README.md that quietly stops being context is an error rather than an omission.
#
# The effective context for a path is computed, never stored: every applicable document
# from the tree root down to the target directory, least specific first, ordered by depth,
# then the declared `order`, then the path — never by the order a filesystem enumerates
# files in. A document applies to its own directory (`scope: directory`), to its directory
# and everything below (`subtree`), or to the directories it lists (`explicit`). Siblings
# never see each other. A path outside the tree gets the tree root's documents plus every
# document whose `tracks` pathspecs cover it. Composition is an ordered list, not a merge of
# Markdown: `extend` appends, `replace` names the ancestor documents it supersedes, and
# `final` marks a document no descendant may supersede. Anything ambiguous — two files with
# one id, a reference to nothing, a cycle, a superseded `final` — fails validation by name;
# nothing picks a winner silently.
#
# Nothing here is provider-specific: the provider names a document may list are the ones the
# policy's projections declare, and the projections themselves are bootstraps that point at
# this resolution rather than carriers of it.

MJ_CTXD_SCHEMA="context/v1"
MJ_CTXD_FLAT=""        # docs.N.<key>=<value>, one document per index
MJ_CTXD_PROBLEMS=""    # class <TAB> subject <TAB> message <TAB> reproduce
MJ_CTXD_LOADED=0
MJ_CTXD_COUNT=0
MJ_CTXD_TAB="$(printf '\t')"
MJ_CTXD_NL='
'

mj_ctxd_tree()      { mj_rel "$MJ_AI_DIR"; }
mj_ctxd_cleanup()   { rm -f "${MJ_CTXD_FLAT:-}" "${MJ_CTXD_PROBLEMS:-}" 2>/dev/null; }
mj_ctxd_problem()   { printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "${4:-}" >> "$MJ_CTXD_PROBLEMS"; }
# the documents are loaded into variables once (mj_yload); a field read is an expansion
mj_ctxd()           { mj_yv ctxd "docs.$1.$2"; }
mj_ctxd_list()      { mj_yvlist ctxd "docs.$1.$2"; }
mj_ctxd_problems()  { [ -s "$MJ_CTXD_PROBLEMS" ]; }
# The same two readings for a caller in this shell, without the command substitution that
# forks it: MJ_V is set to the value, or to the items one per line, and is empty when absent.
mj_ctxd_get() { local k="ctxd__docs__$1__${2//-/___}"; k="${k//./__}"; MJ_V="${!k:-}"; }
mj_ctxd_getlist() {
  local k="ctxd__docs__$1__${2//-/___}" i=0 n; k="${k//./__}"; MJ_V=""
  while n="${k}__$i"; [ -n "${!n:-}" ]; do MJ_V="$MJ_V${!n}$MJ_CTXD_NL"; i=$((i + 1)); done
}
# the index of the document with id $2, into the variable named $1, or 1
mj_ctxd_index_into() {
  local _i=0 _k
  while [ "$_i" -lt "$MJ_CTXD_COUNT" ]; do
    _k="ctxd__docs__${_i}__id"; [ "${!_k:-}" = "$2" ] && { printf -v "$1" '%s' "$_i"; return 0; }
    _i=$((_i + 1))
  done
  return 1
}

# the file conventions the manifest declares: names that must carry the contract
mj_ctxd_conventions() { mj_ylist "$MJ_MAN_FLAT" context.documents 2>/dev/null; }
# the provider names the policy's projections declare, space-separated
# projections.0.provider, projections.1.provider ... up to the first absent or empty one, the
# first line of each key winning as mj_pol reads it; one pass, not two lookups per projection
mj_ctxd_providers() {
  [ -n "${MJ_POL_FLAT:-}" ] || return 0
  awk '/^projections\.[0-9]+\.provider=/ { k = substr($0, 1, index($0, "=") - 1)
      if (!(k in v)) v[k] = substr($0, index($0, "=") + 1) }
    END { for (j = 0; v["projections." j ".provider"] != ""; j++) out = out " " v["projections." j ".provider"]
      printf "%s", substr(out, 2) }' "$MJ_POL_FLAT"
}
# the index of the document with id $1, or 1
mj_ctxd_index() {
  local i=0
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do [ "$(mj_ctxd "$i" id)" = "$1" ] && { printf '%s' "$i"; return 0; }; i=$((i + 1)); done
  return 1
}
# the index of the document at repository-relative path $1, or 1
mj_ctxd_index_at() {
  local i=0
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do [ "$(mj_ctxd "$i" path)" = "$1" ] && { printf '%s' "$i"; return 0; }; i=$((i + 1)); done
  return 1
}

# ---------------------------------------------------------------- discovery
# Every Markdown file under the tree, in C-collation order, minus the local half the
# manifest declares untracked. The walk is what finds a file; the contract is what makes it
# a document, so a file of another kind is passed over rather than listed here by name.
mj_ctxd_files() {
  local tree local_rel
  tree="$(mj_ctxd_tree)"; local_rel="$(mj_rel "$MJ_AI_LOCAL_DIR")"
  # the local half dropped by one awk, not a shell loop that reads the pipe a byte at a time
  ( cd "$MJ_ROOT" && find "$tree" -name '*.md' -type f -print 2>/dev/null ) \
    | LC_ALL=C sort | MJ_CTXD_LOCAL="$local_rel/" awk 'index($0, ENVIRON["MJ_CTXD_LOCAL"]) != 1'
}

# Every subtree a contract exempts (children.exempt), one repository-relative path per
# line. The layer's own contracts say which subtrees they carry but do not author; nothing
# here knows the name of any particular one. ADR 0011.
mj_ctxd_exempt_dirs() {
  local i=0 p
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do
    for p in $(mj_ctxd_list "$i" children_exempt); do printf '%s\n' "$p"; done
    i=$((i + 1))
  done
}

# Is directory $1 at or below a declared exemption?
mj_ctxd_exempt() {
  local d="$1" p
  while IFS= read -r p; do
    [ -n "$p" ] || continue
    case "$d" in "$p"|"$p"/*) return 0 ;; esac
  done <<EOF
$(mj_ctxd_exempt_dirs)
EOF
  return 1
}

# ---------------------------------------------------------------- the files
# mj_ctxd_scan CONV PROVIDERS: every Markdown file under the tree read, and every context
# document among them appended to the flat record as docs.N.*, in file order. A file that
# is not a context document is passed over; one that claims to be and is not is a problem.
#
# The files are read in a fixed number of processes whatever their number: one awk splits
# off every front matter that could be a context document, one flattens them all, and one
# validates every document against the contract. The per-file shape this replaced cost a
# mktemp, a split, a flattener and a validator per document, and its outcome is kept here
# step for step; only a front matter that does not parse is flattened again on its own, to
# name the line, because that is a failure and not the common case (I0820).
#
# Which files could be documents is the question the stages themselves ask: a file whose
# first line is not `---` has no front matter, and one whose front matter declares neither
# `kind: context` nor a `context/` schema in its first two hundred lines is another kind. In
# both cases nothing is recorded — unless the manifest names the file as a context document,
# which is the case that must still be reported, so those are never passed over.
mj_ctxd_scan() {
  local conv="$1" providers="$2" tree work rel st decl m base k=0 bad=" " line kind schema
  local dir rest slashes depth reason total fms=() rels=() pc=() ps=() pm=() pr=()
  tree="$(mj_ctxd_tree)"
  work="$(mktemp -d "${TMPDIR:-/tmp}/mj.ctxd.scan.XXXXXX")"
  mkdir -p "$work/fm" "$work/flat"
  mj_ctxd_files > "$work/files"
  # stage 1: per candidate "<rel> TAB <status> TAB <declares in the front matter> TAB <m>",
  # status 0 with the front matter written to fm/<m>.yaml, 2 none, 4 never closes
  ( cd "$MJ_ROOT" && MJ_CTXD_CONV=" $conv " awk -v work="$work" '
    BEGIN { FS = "\n"; conv = ENVIRON["MJ_CTXD_CONV"] }
    {
      rel = $0; base = rel; sub(/.*\//, "", base); cn = index(conv, " " base " ") > 0
      lines = 0; closed = 0; n = 0; declares = 0; fmdecl = 0; pass = 0
      while ((r = (getline line < rel)) > 0) {
        lines++
        if (lines == 1) { if (line != "---") break; continue }
        if (line == "---") { closed = 1; break }
        fm[++n] = line
        if (line ~ /^(schema: context\/|kind: context$)/) fmdecl = 1
        if (!cn && !declares && lines <= 201 && (line == "kind: context" || index(line, "kind: context ") == 1 || index(line, "schema: context/") == 1)) declares = 1
        if (!cn && !declares && lines >= 201) { pass = 1; break }
      }
      close(rel)
      if (pass || !(cn || declares)) next
      st = (lines == 0 || n == 0) ? 2 : (closed ? 0 : 4)
      if (st == 0) {
        f = work "/fm/" (++m) ".yaml"
        for (i = 1; i <= n; i++) print fm[i] > f
        close(f)
      }
      printf "%s\t%s\t%s\t%s\n", rel, st, fmdecl, (st == 0 ? m : 0)
    }' "$work/files" ) > "$work/cand"
  # stage 2: every front matter flattened by one process, failures listed by number
  while IFS="$MJ_TAB" read -r rel st decl m; do
    [ "$st" = 0 ] && fms+=("$work/fm/$m.yaml")
  done < "$work/cand"
  [ "${#fms[@]}" -gt 0 ] && mj_yaml_flatten_many "$work/flat" --numbered "${fms[@]}"
  if [ -f "$work/flat/.errors" ]; then
    while IFS="$MJ_TAB" read -r m line; do bad="$bad$m "; done < "$work/flat/.errors"
  fi
  # stage 3: the decisions each file's own stages made, in file order. A problem is kept
  # as its four fields; what survives these stages goes to the contract check below.
  : > "$work/pending"
  while IFS="$MJ_TAB" read -r rel st decl m; do
    k=$((k + 1)); base="${rel##*/}"; rels[k]="$rel"; pc[k]=""
    case "$st" in
      0) ;;
      4) mj_ctxd_scan_problem "$k" invalid-front-matter "$rel" "front matter never closes (no second ---)" "head -n 20 '$rel'"; continue ;;
      *) case " $conv " in *" $base "*) mj_ctxd_scan_problem "$k" invalid-front-matter "$rel" "carries no front matter, and the manifest names $base as a context document (context.documents)" "head -n 5 '$rel'" ;; esac
         continue ;;
    esac
    case "$bad" in
      *" $m "*)
        case " $conv " in *" $base "*) decl=1 ;; esac
        [ "$decl" = 1 ] && mj_ctxd_scan_problem "$k" invalid-front-matter "$rel" "front matter does not parse: $(mj_yaml_flatten "$MJ_ROOT/$rel" 2>&1 >/dev/null | sed 's/^ERROR://' | head -n 1)" "head -n 20 '$rel'"
        continue ;;
    esac
    # two keys of a small flat file, read in this shell
    kind=""; schema=""
    while IFS= read -r line; do
      case "$line" in kind=*) [ -n "$kind" ] || kind="${line#kind=}" ;; schema=*) [ -n "$schema" ] || schema="${line#schema=}" ;; esac
    done < "$work/flat/$m"
    if [ "$kind" != context ] && [ "${schema%%/*}" != context ]; then
      case " $conv " in *" $base "*) mj_ctxd_scan_problem "$k" invalid-front-matter "$rel" "front matter is not the context contract (kind '${kind:-absent}'), and the manifest names $base as a context document" "head -n 20 '$rel'" ;; esac
      continue
    fi
    if [ "$schema" != "$MJ_CTXD_SCHEMA" ]; then
      mj_ctxd_scan_problem "$k" unsupported-schema "$rel" "schema '${schema:-absent}' is not $MJ_CTXD_SCHEMA (this executable reads $MJ_CTXD_SCHEMA)" "head -n 3 $rel"; continue
    fi
    dir="${rel%/*}"; [ "$dir" = "$rel" ] && dir="."
    rest="x${dir#"$tree"}"; slashes="${rest//[!\/]/}"; depth="${#slashes}"
    printf '%s\t%s\t%s\t%s\t%s\n' "$k" "$work/flat/$m" "$rel" "$dir" "$depth" >> "$work/pending"
  done < "$work/cand"
  total="$k"
  # stage 4: the contract, for every document that reached it, in file order. A document
  # that passes takes the next index; one that fails is reported as "<k> TAB <reason>".
  awk -F'\t' -v n="$MJ_CTXD_COUNT" -v provs=" $providers " -v flat="$MJ_CTXD_FLAT" -v allow="$MJ_ALLOW_DIR/context.txt" '
    BEGIN { while ((getline line < allow) > 0) if (line !~ /^#/ && line != "") pat[++np] = line   # the allow-list banner is comments
            close(allow) }
    function check(   i, p, ok, bad, nreq, req) {
      bad = ""
      for (i = 1; i <= nl; i++) {
        ok = 0; for (p = 1; p <= np; p++) if (keys[i] ~ pat[p]) { ok = 1; break }
        if (!ok) bad = (bad == "" ? keys[i] : bad " " keys[i])
      }
      if (bad != "") return "unknown-key: front-matter key(s) nothing reads: " bad
      nreq = split("schema id kind title description status scope composition order", req, " ")
      for (i = 1; i <= nreq; i++) if (first[req[i]] == "") return "invalid-front-matter: lacks " req[i]
      if (first["providers.0"] == "") return "invalid-front-matter: lacks providers (write providers: [\"*\"] for every provider)"
      if (first["kind"] != "context") return "invalid-front-matter: kind is " first["kind"] ", not context"
      if (first["id"] !~ /^[a-z][a-z0-9-]*(\.[a-z0-9-]+)*$/) return "invalid-front-matter: id \047" first["id"] "\047 is not a lower-case dotted identifier"
      if (first["status"] != "active" && first["status"] != "deprecated") return "invalid-front-matter: status \047" first["status"] "\047 is neither active nor deprecated"
      if (first["scope"] != "directory" && first["scope"] != "subtree" && first["scope"] != "explicit") return "invalid-front-matter: scope \047" first["scope"] "\047 is not directory, subtree or explicit"
      if (first["composition"] != "extend" && first["composition"] != "replace" && first["composition"] != "final") return "invalid-front-matter: composition \047" first["composition"] "\047 is not extend, replace or final"
      if (first["order"] !~ /^-?[0-9]+$/) return "invalid-front-matter: order \047" first["order"] "\047 is not an integer"
      if (first["scope"] == "explicit" && first["paths.0"] == "") return "invalid-front-matter: scope explicit names no paths"
      if (first["scope"] != "explicit" && first["paths.0"] != "") return "invalid-front-matter: paths are for scope explicit only (scope is " first["scope"] ")"
      if (first["composition"] == "replace" && first["supersedes.0"] == "") return "invalid-front-matter: composition replace names nothing in supersedes"
      if (first["composition"] != "replace" && first["supersedes.0"] != "") return "invalid-front-matter: supersedes is for composition replace only (composition is " first["composition"] ")"
      if (first["children.require_contract"] != "") {
        if (first["children.require_contract"] != "true" && first["children.require_contract"] != "false") return "invalid-front-matter: children.require_contract \047" first["children.require_contract"] "\047 is neither true nor false"
        if (first["scope"] != "subtree") return "invalid-front-matter: children.require_contract states what descendants owe and is for scope subtree only (scope is " first["scope"] ")"
      }
      if (first["children.exempt.0"] != "") {
        if (first["scope"] != "subtree") return "invalid-front-matter: children.exempt names subtrees below this document and is for scope subtree only (scope is " first["scope"] ")"
      }
      for (i = 1; i <= nl; i++) {
        if (keys[i] ~ /^providers\.[0-9]+$/ && vals[i] != "*" && index(provs, " " vals[i] " ") == 0)
          return "unknown-provider: provider \047" vals[i] "\047 is not one the policy projects (have:" provs ")"
        if (keys[i] ~ /^audience\.[0-9]+$/ && vals[i] != "human" && vals[i] != "agent")
          return "invalid-front-matter: audience \047" vals[i] "\047 is neither human nor agent"
      }
      return ""
    }
    {
      k = $1; src = $2; file = $3; dir = $4; depth = $5
      nl = 0; delete keys; delete vals; delete first
      while ((getline line < src) > 0) {
        eq = index(line, "="); if (eq == 0) continue
        key = substr(line, 1, eq - 1); v = substr(line, eq + 1)
        nl++; keys[nl] = key; vals[nl] = v
        if (!(key in first)) first[key] = v
      }
      close(src)
      reason = check()
      if (reason != "") { print k "\t" reason; next }
      nout = split("id title description status scope composition order", out, " ")
      for (i = 1; i <= nout; i++) print "docs." n "." out[i] "=" first[out[i]] >> flat
      print "docs." n ".path=" file >> flat
      print "docs." n ".dir=" dir >> flat
      print "docs." n ".depth=" depth >> flat
      if (first["children.require_contract"] != "") print "docs." n ".children_require_contract=" first["children.require_contract"] >> flat
      has_aud = 0
      for (i = 1; i <= nl; i++) if (keys[i] ~ /^(paths|providers|audience|supersedes|tracks)\.[0-9]+$/) { print "docs." n "." keys[i] "=" vals[i] >> flat; if (keys[i] ~ /^audience/) has_aud = 1 }
      for (i = 1; i <= nl; i++) if (keys[i] ~ /^children\.exempt\.[0-9]+$/) { key = keys[i]; sub(/^children\./, "children_", key); print "docs." n "." key "=" vals[i] >> flat }
      if (!has_aud) { print "docs." n ".audience.0=human" >> flat; print "docs." n ".audience.1=agent" >> flat }
      print k "\t"; n++
    }' "$work/pending" > "$work/checked"
  while IFS= read -r line; do
    k="${line%%"$MJ_TAB"*}"; reason="${line#*"$MJ_TAB"}"
    if [ -z "$reason" ]; then MJ_CTXD_COUNT=$((MJ_CTXD_COUNT + 1)); continue; fi
    mj_ctxd_scan_problem "$k" "${reason%%:*}" "${rels[k]}" "${reason#*: }" "head -n 20 '${rels[k]}'"
  done < "$work/checked"
  # every problem in file order, as when the files were read one at a time
  k=1
  while [ "$k" -le "$total" ]; do
    [ -z "${pc[k]:-}" ] || mj_ctxd_problem "${pc[k]}" "${ps[k]}" "${pm[k]}" "${pr[k]}"
    k=$((k + 1))
  done
  rm -rf "$work"
  return 0
}
# a problem of the k-th candidate, held until every candidate has been decided
mj_ctxd_scan_problem() { pc[$1]="$2"; ps[$1]="$3"; pm[$1]="$4"; pr[$1]="$5"; }

# ---------------------------------------------------------------- the tree
# Load every document once. Problems are collected, never fatal here: validate reports them
# all, resolve refuses on any. Cross-document checks run after every file is read.
mj_ctxd_load() {
  [ "$MJ_CTXD_LOADED" = 1 ] && [ -f "$MJ_CTXD_FLAT" ] && return 0
  MJ_CTXD_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.flat.XXXXXX")"
  MJ_CTXD_PROBLEMS="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.prob.XXXXXX")"
  MJ_CTXD_COUNT=0; MJ_CTXD_LOADED=1
  local conv providers f t0
  t0="$(mj_phase_begin ctxd:conventions)"
  conv="$(mj_ctxd_conventions | tr '\n' ' ')"; providers="$(mj_ctxd_providers)"
  mj_phase_end ctxd:conventions "$t0"; t0="$(mj_phase_begin ctxd:links)"
  # (7) a symbolic link anywhere under the tree, file or directory, whatever its name: the
  # tree is read literally, and a link would let a document live outside what the manifest
  # names or appear under two paths
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    case "$f" in "$(mj_rel "$MJ_AI_LOCAL_DIR")"/*) continue ;; esac
    mj_ctxd_problem refused-path "$f" "is a symbolic link; the tree is read literally and links are not followed" "ls -l '$f'"
  done < <(cd "$MJ_ROOT" && find "$(mj_ctxd_tree)" -type l -print 2>/dev/null | LC_ALL=C sort)
  mj_phase_end ctxd:links "$t0"; t0="$(mj_phase_begin ctxd:scan)"
  mj_ctxd_scan "$conv" "$providers"
  mj_phase_end ctxd:scan "$t0"; t0="$(mj_phase_begin ctxd:load)"
  # every field of every document becomes a variable once; the cross checks and every
  # later reader expand them instead of running one awk per field
  mj_yload "$MJ_CTXD_FLAT" ctxd
  mj_phase_end ctxd:load "$t0"; t0="$(mj_phase_begin ctxd:cross_check)"
  mj_ctxd_cross_check
  mj_phase_end ctxd:cross_check "$t0"
  return 0
}

mj_ctxd_cross_check() {
  local noglob=0; case "$-" in *f*) noglob=1 ;; esac; set -f   # list values such as "*" are words, not globs
  local i j id other p rel tree dir MJ_V unmatched
  tree="$(mj_ctxd_tree)"
  # one id, one file
  awk -F= '/^docs\.[0-9]+\.id=/ { print $2 }' "$MJ_CTXD_FLAT" | LC_ALL=C sort | uniq -d | while IFS= read -r id; do
    p="$(awk -F= -v id="$id" '/^docs\.[0-9]+\.id=/ && $2 == id { split($1, a, "."); n = a[2]; print n }' "$MJ_CTXD_FLAT" \
         | while IFS= read -r j; do printf '%s ' "$(mj_ctxd "$j" path)"; done)"
    mj_ctxd_problem duplicate-id "$id" "claimed by two files: ${p% }" "grep -rln '^id: $id$' $tree"
  done
  # Every field below is read by expansion, not by a command substitution: the checks read
  # each document several times, and a fork per reading was most of their time (I0820).
  mj_ctxd_untracked_specs unmatched
  i=0
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do
    mj_ctxd_get "$i" path; rel="$MJ_V"; mj_ctxd_get "$i" dir; dir="$MJ_V"
    # explicit paths exist, are directories, and lie inside the tree
    mj_ctxd_getlist "$i" paths
    for p in $MJ_V; do
      if ! mj_norm_path_into other "$p"; then mj_ctxd_problem broken-reference "$rel" "paths entry '$p' is not a repository-relative path" "head -n 20 '$rel'"; continue; fi
      mj_path_contains "$tree" "$other" || { mj_ctxd_problem broken-reference "$rel" "paths entry '$p' lies outside the tree $tree/" "head -n 20 '$rel'"; continue; }
      [ -d "$MJ_ROOT/$other" ] || mj_ctxd_problem broken-reference "$rel" "paths entry '$p' is not a directory in this repository" "ls -d '$p'"
    done
    # an exempted subtree exists and lies inside the scope of the document that exempts it:
    # a contract may release the directories it governs and no others, or the narrowing
    # rule could be escaped by exempting a subtree from the side
    mj_ctxd_getlist "$i" children_exempt
    for p in $MJ_V; do
      if ! mj_norm_path_into other "$p"; then mj_ctxd_problem broken-reference "$rel" "children.exempt entry '$p' is not a repository-relative path" "head -n 20 '$rel'"; continue; fi
      mj_path_contains "$dir" "$other" || { mj_ctxd_problem illegal-override "$rel" "children.exempt entry '$p' lies outside the subtree this document governs ($dir/); a contract releases the directories it governs and no others" "majordomus context explain $other"; continue; }
      [ "$other" = "$dir" ] && { mj_ctxd_problem illegal-override "$rel" "children.exempt entry '$p' is this document's own directory; write children.require_contract: false to exempt the whole subtree" "head -n 20 '$rel'"; continue; }
      [ -d "$MJ_ROOT/$other" ] || mj_ctxd_problem broken-reference "$rel" "children.exempt entry '$p' is not a directory in this repository" "ls -d '$p'"
    done
    # every tracked pathspec covers at least one file the index knows
    mj_ctxd_getlist "$i" tracks
    for p in $MJ_V; do
      case "$unmatched" in *"$MJ_CTXD_NL$p$MJ_CTXD_NL"*)
        mj_ctxd_problem broken-reference "$rel" "tracks '$p', which matches no tracked file" "git ls-files -- '$p'" ;;
      esac
    done
    # supersedes: the target exists, is a proper ancestor-scope document, and is not final
    mj_ctxd_getlist "$i" supersedes
    for id in $MJ_V; do
      if ! mj_ctxd_index_into j "$id"; then mj_ctxd_problem broken-reference "$rel" "supersedes '$id', which no document declares" "majordomus context list"; continue; fi
      [ "$j" = "$i" ] && { mj_ctxd_problem cycle "$rel" "supersedes itself" "head -n 20 '$rel'"; continue; }
      mj_ctxd_get "$j" dir
      if ! mj_path_contains "$MJ_V" "$dir"; then
        mj_ctxd_get "$j" path
        mj_ctxd_problem broken-reference "$rel" "supersedes '$id' at $MJ_V, which is not in its ancestor chain; a document may only supersede what applies above it" "majordomus context explain $dir"; continue
      fi
      mj_ctxd_get "$j" composition
      [ "$MJ_V" = final ] \
        && mj_ctxd_problem illegal-override "$rel" "supersedes '$id', which is final; no descendant may weaken it" "majordomus context explain $dir"
    done
    i=$((i + 1))
  done
  mj_ctxd_cycles
  mj_ctxd_coverage
  [ "$noglob" = 1 ] || set +f
  return 0
}

# Every `tracks` pathspec of every document that matches no file the index knows, each
# between newlines, so that membership is one pattern match. Git is asked once for all of
# them rather than once per pathspec: `ls-files --error-unmatch` names every pathspec of the
# call that matched nothing, and the index is still what answers, so an untracked file never
# satisfies one (I0820). A pathspec with magic (`:(exclude)...`) is asked on its own, because
# an exclusion would apply to every other pathspec of the call, and so is every pathspec when
# the call fails for another reason than a pathspec that matched nothing.
mj_ctxd_untracked_specs() { # <variable to set>
  local i=0 p err rc specs=() single=() out="$MJ_CTXD_NL" line MJ_V
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do
    mj_ctxd_getlist "$i" tracks
    for p in $MJ_V; do case "$p" in :*) single+=("$p") ;; *) specs+=("$p") ;; esac; done
    i=$((i + 1))
  done
  if [ "${#specs[@]}" -gt 0 ]; then
    err="$(LC_ALL=C mj_git ls-files --error-unmatch -- "${specs[@]}" 2>&1 >/dev/null)" && rc=0 || rc=$?
    case "$rc" in
      0) ;;
      1) while IFS= read -r line; do
           case "$line" in "error: pathspec '"*"' did not match any file(s) known to git")
             line="${line#"error: pathspec '"}"; out="$out${line%"' did not match any file(s) known to git"}$MJ_CTXD_NL" ;;
           esac
         done <<EOF
$err
EOF
         ;;
      *) single+=("${specs[@]}") ;;
    esac
  fi
  for p in ${single[@]+"${single[@]}"}; do
    [ -n "$(mj_git ls-files -- "$p" 2>/dev/null | head -n 1)" ] || out="$out$p$MJ_CTXD_NL"
  done
  printf -v "$1" '%s' "$out"
}

# the index of a document whose directory is $1, or 1
mj_ctxd_index_for_dir() {
  local i=0
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do [ "$(mj_ctxd "$i" dir)" = "$1" ] && { printf '%s' "$i"; return 0; }; i=$((i + 1)); done
  return 1
}

# The children.require_contract that governs directory $1: the nearest declaration at or
# above it ($2 = strict skips a declaration in the directory itself), as "<value><TAB><path>".
# Empty when nothing declares it, which the caller reads as the default: a contract is owed.
mj_ctxd_require_at() {
  local dir="$1" strict="${2:-0}" i=0 val ddir d best_depth=-1 best_val="" best_path="" MJ_V
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do
    mj_ctxd_get "$i" children_require_contract; val="$MJ_V"
    mj_ctxd_get "$i" dir; ddir="$MJ_V"
    if [ -n "$val" ] && mj_path_contains "$ddir" "$dir" && { [ "$strict" = 0 ] || [ "$ddir" != "$dir" ]; }; then
      mj_ctxd_get "$i" depth; d="$MJ_V"
      if [ "$d" -gt "$best_depth" ]; then best_depth="$d"; best_val="$val"; mj_ctxd_get "$i" path; best_path="$MJ_V"; fi
    fi
    i=$((i + 1))
  done
  printf '%s\t%s' "$best_val" "$best_path"
}

# Every directory of the tree, with what it owes and what carries it, as
#   <dir> <TAB> <document index or -> <TAB> true|false <TAB> <governing document path or ->
# A field is never empty: a tab is IFS whitespace, so `read` would collapse two of them and
# hand the caller the wrong column.
#
# One awk over the flat records and the directory list, rather than a shell loop that reads
# every document again for every directory: this runs inside `context validate`, which runs
# inside `doctor`, which runs in the pre-commit hook, and a check on that path is measured
# before it is added (project.blocking-checks-cheap).
mj_ctxd_directories() {
  local tree local_rel
  tree="$(mj_ctxd_tree)"; local_rel="$(mj_rel "$MJ_AI_LOCAL_DIR")"
  ( cd "$MJ_ROOT" && find "$tree" -type d -print 2>/dev/null ) | LC_ALL=C sort \
  | awk -v flat="$MJ_CTXD_FLAT" -v localrel="$local_rel" '
      BEGIN {
        while ((getline line < flat) > 0) {
          eq = index(line, "="); if (eq == 0) continue
          k = substr(line, 1, eq - 1); v = substr(line, eq + 1)
          if (k !~ /^docs\.[0-9]+\./) continue
          split(k, a, ".") ; n = a[2] ; field = substr(k, length("docs." n ".") + 1)
          if      (field == "dir")   { dir[n] = v; at[v] = n }
          else if (field == "path")  path[n] = v
          else if (field == "depth") depth[n] = v
          else if (field == "children_require_contract") req[n] = v
          else if (field ~ /^children_exempt\.[0-9]+$/) exempt[v] = 1
        }
        close(flat)
      }
      function under(parent, child) { return child == parent || index(child, parent "/") == 1 }
      {
        d = $0
        if (under(localrel, d)) next               # the manifest declares this half untracked
        for (e in exempt) if (under(e, d)) next    # a contract above declares this subtree exempt
        doc = (d in at) ? at[d] : "-"
        best = -1; val = ""; gov = "-"
        for (n in req) {
          if (req[n] == "" || !under(dir[n], d)) continue
          if (depth[n] + 0 > best) { best = depth[n] + 0; val = req[n]; gov = path[n] }
        }
        if (val != "false") val = "true"          # nothing declared: a document is owed
        printf "%s\t%s\t%s\t%s\n", d, doc, val, gov
      }'
}

# Coverage: every directory of the tree carries a context document, and no descendant
# weakens an ancestor that says its children owe one. The exemption is declared by the
# contract that governs the subtree (children.require_contract: false), never by a list at
# the root: it moves with the tree it describes. ADR 0011.
mj_ctxd_coverage() {
  local tree local_rel d gov val path i dir MJ_V
  tree="$(mj_ctxd_tree)"; local_rel="$(mj_rel "$MJ_AI_LOCAL_DIR")"
  # narrowing only: false may become true below, true may not become false
  i=0
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do
    mj_ctxd_get "$i" children_require_contract
    if [ "$MJ_V" = false ]; then
      mj_ctxd_get "$i" dir; dir="$MJ_V"
      gov="$(mj_ctxd_require_at "$dir" 1)"; val="${gov%%"$MJ_CTXD_TAB"*}"; path="${gov#*"$MJ_CTXD_TAB"}"
      mj_ctxd_get "$i" path
      [ "$val" = true ] && mj_ctxd_problem illegal-override "$MJ_V" \
        "declares children.require_contract: false under $path, which requires one; a descendant may narrow an inherited constraint, never weaken it" \
        "majordomus context explain $dir"
    fi
    i=$((i + 1))
  done
  # every directory inside the tree, minus the half the manifest declares untracked and the
  # subtrees a contract above declares exempt (children.exempt)
  local doc req
  while IFS="$MJ_CTXD_TAB" read -r d doc req path; do
    [ -n "$d" ] || continue
    [ "$doc" = - ] || continue
    [ "$req" = false ] && continue
    mj_ctxd_problem missing-contract "$d" \
      "carries no context document$([ "$path" != - ] && printf ', and %s requires one of every directory below it' "$path"); a directory inside the tree says what it is for, or its governing contract exempts it (children.require_contract: false)" \
      "printf '%s\\n' '---' 'schema: context/v1' > $d/README.md"
  done < <(mj_ctxd_directories)
  return 0
}

# a supersedes chain that returns to its start
mj_ctxd_cycles() {
  awk -F= '
    /^docs\.[0-9]+\.id=/ { split($1, a, "."); id[a[2]] = $2 }
    /^docs\.[0-9]+\.supersedes\.[0-9]+=/ { split($1, a, "."); e[a[2]] = e[a[2]] " " $2 }
    END {
      for (n in id) byid[id[n]] = n
      for (n in id) {
        # walk from n; a return to id[n] is a cycle
        delete seen; if (walk(n, id[n])) print id[n]
      }
    }
    function walk(n, start,   m, k, parts, t) {
      m = split(e[n], parts, " ")
      for (k = 1; k <= m; k++) {
        t = parts[k]; if (t == "") continue
        if (t == start) return 1
        if (t in seen) continue
        seen[t] = 1
        if ((t in byid) && walk(byid[t], start)) return 1
      }
      return 0
    }' "$MJ_CTXD_FLAT" | LC_ALL=C sort -u | while IFS= read -r id; do
      mj_ctxd_problem cycle "$id" "supersedes chain returns to itself" "majordomus context list"
    done
}

# ---------------------------------------------------------------- the target
# A repository-relative directory: a file resolves to its directory; `..`, absolute paths
# and anything whose real location leaves the repository are refused, not normalised away.
#
# A path that does not exist yet resolves too. The bootstraps say to resolve a path before
# working under it, and the work is often creating it; refusing "no such path" left a worker
# about to write src/new/thing.rs with no answer at all. Documents apply by directory, and a
# directory that does not exist holds none, so the chain of a path not yet created is its
# ancestors' chain; where it leads is judged at its nearest existing ancestor, whose real
# location must still be inside the repository. Until it exists nothing says whether it will
# be a file or a directory, and a `scope: directory` document in its parent applies to the
# one and not to the other: it is read as a file in its parent, as an existing file is,
# unless it is written with a trailing slash. MJ_CTXD_TARGET_NEW says it was not there.
MJ_CTXD_TARGET=""; MJ_CTXD_TARGET_DIR=""; MJ_CTXD_TARGET_NEW=0
# mj_ctxd_judge PATH: the same judgement without the exit. MJ_CTXD_REFUSAL is empty when the
# path is a target, and the reason (`refused-path ...`) when it is not; MJ_CTXD_JUDGED is the
# normalised path and MJ_CTXD_TARGET_NEW whether it does not exist yet. `start` asks it of
# every scope entry before recording one, and `context` of every recorded entry before
# resolving it, so that a path refused here is refused once, in one wording (I2760).
MJ_CTXD_REFUSAL=""; MJ_CTXD_JUDGED=""
mj_ctxd_judge() {
  local p="$1" norm real root near
  MJ_CTXD_REFUSAL=""; MJ_CTXD_JUDGED=""; MJ_CTXD_TARGET_NEW=0
  case "$p" in "$MJ_ROOT"|"$MJ_ROOT"/*) p="${p#"$MJ_ROOT"}"; p="${p#/}" ;; esac
  [ -n "$p" ] || p="."
  if [ "$p" != "." ]; then
    # not into `p`: the helper's own local of that name would take the value
    mj_norm_path_into norm "$p" || {
      MJ_CTXD_REFUSAL="refused-path '$1' (absolute, or leaves the repository)"; return 1
    }
    p="$norm"
  fi
  near="$p"
  while [ "$near" != "." ] && [ ! -e "$MJ_ROOT/$near" ] && [ ! -L "$MJ_ROOT/$near" ]; do
    MJ_CTXD_TARGET_NEW=1
    case "$near" in */*) near="${near%/*}" ;; *) near="." ;; esac
  done
  real="$(cd "$MJ_ROOT/$near" 2>/dev/null && pwd -P || (cd "$(dirname "$MJ_ROOT/$near")" && pwd -P))"
  root="$(cd "$MJ_ROOT" && pwd -P)"
  case "$real" in "$root"|"$root"/*) ;; *)
    MJ_CTXD_REFUSAL="refused-path '$p' resolves outside the repository ($real)"; return 1 ;;
  esac
  MJ_CTXD_JUDGED="$p"
  return 0
}
mj_ctxd_target() {
  local p
  mj_ctxd_judge "$1" || mj_die "$MJ_EX_REFUSED" "context: $MJ_CTXD_REFUSAL"
  p="$MJ_CTXD_JUDGED"
  MJ_CTXD_TARGET="$p"
  if [ -d "$MJ_ROOT/$p" ] || { [ "$MJ_CTXD_TARGET_NEW" = 1 ] && case "$1" in */) true ;; *) false ;; esac; }; then MJ_CTXD_TARGET_DIR="$p"
  else MJ_CTXD_TARGET_DIR="${p%/*}"; [ "$MJ_CTXD_TARGET_DIR" != "$p" ] || MJ_CTXD_TARGET_DIR="."; fi
  return 0
}

# ---------------------------------------------------------------- resolution
# mj_ctxd_resolve PROVIDER AUDIENCE, after mj_ctxd_target. Writes two files:
#   $MJ_CTXD_CHAIN     index <TAB> reason        applicable documents in effective order
#   $MJ_CTXD_EXCLUDED  index <TAB> reason        candidates left out, and why
MJ_CTXD_CHAIN=""; MJ_CTXD_EXCLUDED=""
mj_ctxd_resolve() {
  local noglob=0; case "$-" in *f*) noglob=1 ;; esac; set -f   # list values such as "*" are words, not globs
  local provider="$1" audience="$2" i dir depth scope reason inside=0 tree t="$MJ_CTXD_TARGET_DIR" p v ok tmp
  tree="$(mj_ctxd_tree)"
  MJ_CTXD_CHAIN="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.chain.XXXXXX")"; MJ_CTXD_EXCLUDED="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.excl.XXXXXX")"
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.sort.XXXXXX")"
  mj_path_contains "$tree" "$t" && inside=1
  i=0
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do
    dir="$(mj_ctxd "$i" dir)"; depth="$(mj_ctxd "$i" depth)"; scope="$(mj_ctxd "$i" scope)"; reason=""
    if [ "$inside" = 1 ]; then
      case "$scope" in
        directory) [ "$dir" = "$t" ] && reason="the target directory (scope directory)" ;;
        subtree)
          if [ "$dir" = "$t" ]; then reason="the target directory (scope subtree)"
          elif mj_path_contains "$dir" "$t"; then reason="ancestor at depth $depth (scope subtree)"; fi ;;
        # an explicit document is as specific as the path it names, not as its own directory
        explicit) for p in $(mj_ctxd_list "$i" paths); do
                    mj_path_contains "$p" "$t" && { reason="declared path $p (scope explicit)"; depth="$(printf 'x%s\n' "${p#"$tree"}" | awk -F/ '{ print NF - 1 }')"; break; }
                  done ;;
      esac
    else
      [ "$dir" = "$tree" ] && [ "$scope" = subtree ] && reason="the tree root; the target is outside $tree/"
      if [ -z "$reason" ]; then
        # A directory target takes the documents of the files it holds, but the repository
        # root is not such a directory: since `.` contains every path (case 3190), it would
        # take every document that tracks anything, and its chain is the layer's alone (case 69).
        for p in $(mj_ctxd_list "$i" tracks); do
          for v in $(mj_git ls-files -- "$p" 2>/dev/null); do
            if { [ "$MJ_CTXD_TARGET" != . ] && mj_path_contains "$MJ_CTXD_TARGET" "$v"; } \
              || mj_path_contains "$v" "$MJ_CTXD_TARGET"; then reason="tracks $p"; break; fi
          done
          [ -n "$reason" ] && break
        done
      fi
    fi
    if [ -z "$reason" ]; then i=$((i + 1)); continue; fi
    if [ "$(mj_ctxd "$i" status)" = deprecated ]; then printf '%s\tdeprecated; discovered and never applied\n' "$i" >> "$MJ_CTXD_EXCLUDED"; i=$((i + 1)); continue; fi
    if [ -n "$provider" ]; then
      ok=0; for v in $(mj_ctxd_list "$i" providers); do [ "$v" = '*' ] || [ "$v" = "$provider" ] && ok=1; done
      [ "$ok" = 1 ] || { printf '%s\tprovider %s is not among its providers (%s)\n' "$i" "$provider" "$(mj_ctxd_list "$i" providers | paste -sd, -)" >> "$MJ_CTXD_EXCLUDED"; i=$((i + 1)); continue; }
    fi
    if [ -n "$audience" ]; then
      ok=0; for v in $(mj_ctxd_list "$i" audience); do [ "$v" = "$audience" ] && ok=1; done
      [ "$ok" = 1 ] || { printf '%s\taudience %s is not among its audience (%s)\n' "$i" "$audience" "$(mj_ctxd_list "$i" audience | paste -sd, -)" >> "$MJ_CTXD_EXCLUDED"; i=$((i + 1)); continue; }
    fi
    printf '%s\t%s\t%s\t%s\t%s\n' "$depth" "$(mj_ctxd "$i" order)" "$(mj_ctxd "$i" path)" "$i" "$reason" >> "$tmp"
    i=$((i + 1))
  done
  # supersession among what applies: a replaced document leaves the chain with its reason
  local sup by
  LC_ALL=C sort -t "$MJ_CTXD_TAB" -k1,1n -k2,2n -k3,3 "$tmp" > "$tmp.sorted"
  while IFS="$MJ_CTXD_TAB" read -r _ _ _ i reason; do
    [ "$(mj_ctxd "$i" composition)" = replace ] || continue
    for sup in $(mj_ctxd_list "$i" supersedes); do
      by="$(mj_ctxd_index "$sup" 2>/dev/null || true)"; [ -n "$by" ] || continue
      grep -q "^[0-9]*	[0-9-]*	[^	]*	$by	" "$tmp.sorted" && printf '%s\tsuperseded by %s (%s)\n' "$by" "$(mj_ctxd "$i" id)" "$(mj_ctxd "$i" path)" >> "$MJ_CTXD_EXCLUDED"
    done
  done < "$tmp.sorted"
  while IFS="$MJ_CTXD_TAB" read -r _ _ _ i reason; do
    grep -q "^$i	superseded" "$MJ_CTXD_EXCLUDED" && continue
    printf '%s\t%s\n' "$i" "$reason" >> "$MJ_CTXD_CHAIN"
  done < "$tmp.sorted"
  rm -f "$tmp" "$tmp.sorted"
  [ "$noglob" = 1 ] || set +f
  return 0
}

# a deterministic fingerprint of one resolution: the schema, then id, path and content hash
# of every document in effective order. Two equal repository states give two equal values.
mj_ctxd_fingerprint() {
  local tmp i; tmp="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.fp.XXXXXX")"
  { printf '%s\n' "$MJ_CTXD_SCHEMA"
    while IFS="$MJ_CTXD_TAB" read -r i _; do printf '%s %s %s\n' "$(mj_ctxd "$i" id)" "$(mj_ctxd "$i" path)" "$(mj_sha256 "$MJ_ROOT/$(mj_ctxd "$i" path)")"; done < "$MJ_CTXD_CHAIN"
  } > "$tmp"
  mj_sha256 "$tmp"; rm -f "$tmp"
}

# ---------------------------------------------------------------- reporting
mj_ctxd_report_problems() {
  local cls subj msg rep
  while IFS="$MJ_CTXD_TAB" read -r cls subj msg rep; do mj_fail context "$subj" "$cls: $msg" "$rep"; done < "$MJ_CTXD_PROBLEMS"
}
mj_ctxd_require_valid() {
  mj_ctxd_load
  if mj_ctxd_problems; then
    mj_ctxd_report_problems
    mj_die "$MJ_EX_CONTRACT" "context: the tree does not validate ($(mj_lines "$MJ_CTXD_PROBLEMS") problem(s) above); nothing is resolved partially"
  fi
}
mj_ctxd_json_doc() {
  local noglob=0; case "$-" in *f*) noglob=1 ;; esac; set -f   # list values such as "*" are words, not globs
  local i="$1" reason="$2" idx="$3" first v
  printf '{"index":%s,"id":"%s","path":"%s","dir":"%s","depth":%s,"scope":"%s","composition":"%s","order":%s,"status":"%s","title":"%s","description":"%s","children_require_contract":%s,"providers":[' \
    "$idx" "$(mj_ctxd "$i" id)" "$(mj_json_esc "$(mj_ctxd "$i" path)")" "$(mj_json_esc "$(mj_ctxd "$i" dir)")" "$(mj_ctxd "$i" depth)" \
    "$(mj_ctxd "$i" scope)" "$(mj_ctxd "$i" composition)" "$(mj_ctxd "$i" order)" "$(mj_ctxd "$i" status)" "$(mj_json_esc "$(mj_ctxd "$i" title)")" \
    "$(mj_json_esc "$(mj_ctxd "$i" description)")" "$(v="$(mj_ctxd "$i" children_require_contract)"; [ -n "$v" ] && printf '%s' "$v" || printf null)"
  first=1; for v in $(mj_ctxd_list "$i" providers); do [ "$first" = 1 ] || printf ','; printf '"%s"' "$(mj_json_esc "$v")"; first=0; done
  printf '],"audience":['
  first=1; for v in $(mj_ctxd_list "$i" audience); do [ "$first" = 1 ] || printf ','; printf '"%s"' "$(mj_json_esc "$v")"; first=0; done
  printf '],"supersedes":['
  first=1; for v in $(mj_ctxd_list "$i" supersedes); do [ "$first" = 1 ] || printf ','; printf '"%s"' "$(mj_json_esc "$v")"; first=0; done
  printf '],"tracks":['
  first=1; for v in $(mj_ctxd_list "$i" tracks); do [ "$first" = 1 ] || printf ','; printf '"%s"' "$(mj_json_esc "$v")"; first=0; done
  printf '],"paths":['
  first=1; for v in $(mj_ctxd_list "$i" paths); do [ "$first" = 1 ] || printf ','; printf '"%s"' "$(mj_json_esc "$v")"; first=0; done
  printf '],"sha256":"%s"' "$(mj_sha256 "$MJ_ROOT/$(mj_ctxd "$i" path)")"
  [ -n "$reason" ] && printf ',"reason":"%s"' "$(mj_json_esc "$reason")"
  printf '}'
  [ "$noglob" = 1 ] || set +f
  return 0
}

# ---------------------------------------------------------------- changes
# The change set, one line each: status <TAB> path <TAB> new path (renames only).
#   worktree   HEAD against the working tree, plus untracked files       (the default)
#   staged     HEAD against the index
#   base REF   REF against the working tree, plus untracked files
mj_ctxd_changes() {
  local mode="$1" base="${2:-}"
  [ "$mode" != base ] || mj_ctxd_require_ref "$base"
  mj_change_set "$mode" "$base"
}

mj_ctxd_require_ref() {
  mj_git rev-parse --verify --quiet "$1^{commit}" >/dev/null 2>&1 || mj_die "$MJ_EX_USAGE" "context: --base '$1' is not a commit in this repository"
}

# mj_ctxd_affected MODE [BASE]: what a change set touches, as findings. Sets MJ_CTXD_AFFECTED
# to the number of documents named. Nothing is touched, and an unrelated change says nothing.
MJ_CTXD_AFFECTED=0
mj_ctxd_affected() {
  local noglob=0; case "$-" in *f*) noglob=1 ;; esac; set -f   # list values such as "*" are words, not globs
  local mode="$1" base="${2:-}" st old new i j dir n ids k tree man pol rep p line
  tree="$(mj_ctxd_tree)"; man="$(mj_rel "$MJ_AI_MANIFEST")"; pol="$(mj_rel "$MJ_POLICY_FILE")"
  MJ_CTXD_AFFECTED=0
  case "$mode" in base) rep="git diff --name-status -M $base" ;; staged) rep="git diff --name-status -M --cached" ;; *) rep="git status --porcelain" ;; esac
  local changes; changes="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.chg.XXXXXX")"
  mj_ctxd_changes "$mode" "$base" > "$changes"
  while IFS="$MJ_CTXD_TAB" read -r st old new; do
    p="${new:-$old}"
    if [ "$p" = "$man" ]; then
      ids=""; j=0; while [ "$j" -lt "$MJ_CTXD_COUNT" ]; do ids="$ids $(mj_ctxd "$j" id)"; j=$((j + 1)); done
      mj_info context "$p" "$(mj_ctxd_status_word "$st"); the manifest names the tree and its conventions, so every document is affected:$ids" "$rep -- $p"
      MJ_CTXD_AFFECTED=$((MJ_CTXD_AFFECTED + MJ_CTXD_COUNT)); continue
    fi
    if [ "$p" = "$pol" ]; then
      mj_info context "$p" "$(mj_ctxd_status_word "$st"); a projection rendered from the previous policy is stale-projection under check-sync and drift under watch" "$rep -- $p"; continue
    fi
    case "$p" in
      "$tree"/*.md)
        if [ "$st" = D ]; then
          local from="HEAD"; [ "$mode" = base ] && from="$base"
          mj_git show "$from:$old" 2>/dev/null | awk 'NR == 1 && $0 != "---" { exit 1 } NR > 1 && $0 == "---" { exit 1 } NR > 1' \
            | grep -qE '^(schema: context/|kind: context$)' || continue
          mj_info context "$old" "deleted; a document that superseded it is a broken-reference under validate, and its scope now inherits from above" "majordomus context validate"
          MJ_CTXD_AFFECTED=$((MJ_CTXD_AFFECTED + 1)); continue
        fi
        i="$(mj_ctxd_index_at "$p" 2>/dev/null || true)"; [ -n "$i" ] || continue
        dir="$(mj_ctxd "$i" dir)"
        # the documents below its scope: every one resolved under it changes with it
        n=0; ids=""; j=0
        if [ "$(mj_ctxd "$i" status)" = active ]; then
          while [ "$j" -lt "$MJ_CTXD_COUNT" ]; do
            if [ "$j" != "$i" ] && mj_ctxd_reaches "$i" "$(mj_ctxd "$j" dir)"; then n=$((n + 1)); ids="$ids $(mj_ctxd "$j" id)"; fi
            j=$((j + 1))
          done
        fi
        line="$(mj_ctxd_status_word "$st") $(mj_ctxd "$i" id); scope $dir/ ($(mj_ctxd "$i" scope)$([ "$(mj_ctxd "$i" status)" = deprecated ] && printf ', deprecated: applied nowhere'))"
        [ "$n" -gt 0 ] && line="$line and $n document(s) resolved below it:${ids}"
        if [ "$st" = R ]; then
          k="${old%/*}"; [ "$k" = "$old" ] && k="."
          if [ "$k" != "$dir" ]; then line="$line; moved from $k/, so its ancestry changed"; else line="$line; renamed within $dir/"; fi
          line="$line; identity $(mj_ctxd "$i" id) travels with the file"
        fi
        mj_info context "$p" "$line" "majordomus context explain $dir"
        MJ_CTXD_AFFECTED=$((MJ_CTXD_AFFECTED + 1 + n))
        # the ones it supersedes, and the ones superseding it, are re-resolved with it
        for k in $(mj_ctxd_list "$i" supersedes); do mj_info context "$k" "superseded by $(mj_ctxd "$i" id), which changed" "majordomus context explain $dir"; done
        ;;
    esac
  done < "$changes"
  # tracked sources: a document says it describes these paths; a change there is for review
  i=0
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do
    for p in $(mj_ctxd_list "$i" tracks); do
      local hits; hits="$(mj_ctxd_track_hits "$mode" "$base" "$p")"
      [ -n "$hits" ] || continue
      mj_warn context "$(mj_ctxd "$i" id)" "tracks $p ($(printf '%s' "$hits" | awk '{ s = s (s == "" ? "" : ", ") $1 " " $2 } END { print s }')); confirm $(mj_ctxd "$i" path) still describes it" "$rep -- $p"
      MJ_CTXD_AFFECTED=$((MJ_CTXD_AFFECTED + 1))
    done
    i=$((i + 1))
  done
  rm -f "$changes"
  [ "$noglob" = 1 ] || set +f
  return 0
}
# does document $1 apply to directory $2, by its own scope? (directory: only its own; subtree:
# everything below; explicit: what it names)
mj_ctxd_reaches() {
  local i="$1" d="$2" p
  case "$(mj_ctxd "$i" scope)" in
    directory) [ "$(mj_ctxd "$i" dir)" = "$d" ] ;;
    subtree) mj_path_contains "$(mj_ctxd "$i" dir)" "$d" ;;
    explicit) for p in $(mj_ctxd_list "$i" paths); do mj_path_contains "$p" "$d" && return 0; done; return 1 ;;
  esac
}
mj_ctxd_status_word() { case "$1" in A) printf 'added' ;; M) printf 'modified' ;; D) printf 'deleted' ;; R) printf 'moved' ;; C) printf 'copied' ;; *) printf 'changed' ;; esac; }
# the changed paths under one pathspec, as "status path" lines, through git's own matching
mj_ctxd_track_hits() {
  local mode="$1" base="$2" spec="$3"
  {
    case "$mode" in
      staged) mj_git diff --name-status --cached -- "$spec" 2>/dev/null ;;
      base)   mj_git diff --name-status "$base" -- "$spec" 2>/dev/null; mj_git ls-files --others --exclude-standard -- "$spec" 2>/dev/null | sed 's/^/A\t/' ;;
      *)      mj_git diff --name-status HEAD -- "$spec" 2>/dev/null; mj_git ls-files --others --exclude-standard -- "$spec" 2>/dev/null | sed 's/^/A\t/' ;;
    esac
  } | awk -F'\t' '{ print substr($1, 1, 1) " " $NF }' | LC_ALL=C sort -u
}

# ---------------------------------------------------------------- the doctrine
# majordomus.context-integrity: every document under the tree carries the contract and the
# tree composes. Under doctor a problem is a failure; under watch the same problem is drift.
mj_validate_context() {
  local cls subj msg rep
  mj_ctxd_load
  if mj_ctxd_problems; then
    while IFS="$MJ_CTXD_TAB" read -r cls subj msg rep; do mj_doctrine_fail context "$subj" "$cls: $msg" "$rep"; done < "$MJ_CTXD_PROBLEMS"
  else
    mj_doctrine_ok context "$(mj_ctxd_tree)/" "$MJ_CTXD_COUNT context document(s) carry the contract; ids unique, references resolve, no final document superseded, no cycle" "majordomus context validate"
  fi
  return 0
}
