#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_context_docs:-}" ] && return 0 || MJ_LIB_context_docs=1
# context documents — hierarchical, directory-scoped context under the AI layer.
#
# A context document is a Markdown file under the layer's tree (the manifest's directory,
# minus the local half and minus the vendored rule package) whose front matter declares the
# contract `schema: context/v1`, `kind: context`. The file name is a convention, never the
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

mj_ctxd_tree()      { mj_rel "$MJ_AI_DIR"; }
mj_ctxd_cleanup()   { rm -f "${MJ_CTXD_FLAT:-}" "${MJ_CTXD_PROBLEMS:-}" 2>/dev/null; }
mj_ctxd_problem()   { printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "${4:-}" >> "$MJ_CTXD_PROBLEMS"; }
mj_ctxd()           { mj_yget "$MJ_CTXD_FLAT" "docs.$1.$2"; }
mj_ctxd_list()      { mj_ylist "$MJ_CTXD_FLAT" "docs.$1.$2"; }
mj_ctxd_problems()  { [ -s "$MJ_CTXD_PROBLEMS" ]; }

# the file conventions the manifest declares: names that must carry the contract
mj_ctxd_conventions() { mj_ylist "$MJ_MAN_FLAT" context.documents 2>/dev/null; }
# the provider names the policy's projections declare, space-separated
mj_ctxd_providers() {
  local j=0 out=""
  while [ -n "$(mj_pol "projections.$j.provider")" ]; do out="$out $(mj_pol "projections.$j.provider")"; j=$((j + 1)); done
  printf '%s' "${out# }"
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
# Every Markdown file under the tree, in C-collation order, minus the local half and the
# vendored package. The walk is what finds a file; the contract is what makes it a document.
mj_ctxd_files() {
  local tree local_rel vendor_rel
  tree="$(mj_ctxd_tree)"; local_rel="$(mj_rel "$MJ_AI_LOCAL_DIR")"; vendor_rel="$(mj_rel "$MJ_RULES_DIR")/vendor"
  ( cd "$MJ_ROOT" && find "$tree" -name '*.md' -type f -print 2>/dev/null ) \
    | LC_ALL=C sort | while IFS= read -r f; do
        case "$f" in "$local_rel"/*|"$vendor_rel"/*) continue ;; esac
        printf '%s\n' "$f"
      done
}

# ---------------------------------------------------------------- one file
# mj_ctxd_scan REL N -> 0 with docs.N.* appended to the flat record, 2 when the file is not
# a context document (no front matter, or another kind), 1 with a problem recorded.
mj_ctxd_scan() {
  local rel="$1" n="$2" f="$MJ_ROOT/$1" conv="$3" providers="$4" base dir depth st tmp
  base="${rel##*/}"
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.XXXXXX")"
  # stage 1: the front matter, or exit 2 (none) / 4 (never closes)
  awk 'NR == 1 && $0 != "---" { exit 2 }
       NR > 1 && $0 == "---" { c = 1; exit }
       NR > 1 { n++; print }
       END { if (!c) exit (n ? 4 : 2); if (!n) exit 2 }' "$f" > "$tmp.fm"; st=$?
  case "$st" in
    0) ;;
    4) rm -f "$tmp" "$tmp.fm"; mj_ctxd_problem invalid-front-matter "$rel" "front matter never closes (no second ---)" "head -n 20 '$rel'"; return 1 ;;
    *) rm -f "$tmp" "$tmp.fm"
       case " $conv " in *" $base "*) mj_ctxd_problem invalid-front-matter "$rel" "carries no front matter, and the manifest names $base as a context document (context.documents)" "head -n 5 '$rel'"; return 1 ;; esac
       return 2 ;;
  esac
  if ! mj_yaml_flatten "$tmp.fm" > "$tmp" 2>/dev/null; then
    local declares=0; grep -qE '^(schema: context/|kind: context$)' "$tmp.fm" && declares=1
    rm -f "$tmp" "$tmp.fm"
    case " $conv " in *" $base "*) declares=1 ;; esac
    [ "$declares" = 1 ] && { mj_ctxd_problem invalid-front-matter "$rel" "front matter does not parse: $(mj_yaml_flatten "$f" 2>&1 >/dev/null | sed 's/^ERROR://' | head -n 1)" "head -n 20 '$rel'"; return 1; }
    return 2
  fi
  rm -f "$tmp.fm"
  local kind schema; kind="$(mj_yget "$tmp" kind)"; schema="$(mj_yget "$tmp" schema)"
  if [ "$kind" != context ] && [ "${schema%%/*}" != context ]; then
    rm -f "$tmp"
    case " $conv " in *" $base "*) mj_ctxd_problem invalid-front-matter "$rel" "front matter is not the context contract (kind '${kind:-absent}'), and the manifest names $base as a context document" "head -n 20 '$rel'"; return 1 ;; esac
    return 2
  fi
  if [ "$schema" != "$MJ_CTXD_SCHEMA" ]; then
    rm -f "$tmp"; mj_ctxd_problem unsupported-schema "$rel" "schema '${schema:-absent}' is not $MJ_CTXD_SCHEMA (this executable reads $MJ_CTXD_SCHEMA)" "head -n 3 $rel"; return 1
  fi
  dir="${rel%/*}"; [ "$dir" = "$rel" ] && dir="."
  depth="$(printf 'x%s\n' "${dir#"$(mj_ctxd_tree)"}" | awk -F/ '{ print NF - 1 }')"
  local reason
  reason="$(awk -v n="$n" -v file="$rel" -v dir="$dir" -v depth="$depth" -v provs=" $providers " -v flat="$MJ_CTXD_FLAT" '
    function fail(m) { print m; exit 1 }
    FNR == NR { pat[++np] = $0; next }
    {
      eq = index($0, "="); if (eq == 0) next
      k = substr($0, 1, eq - 1); v = substr($0, eq + 1)
      nl++; keys[nl] = k; vals[nl] = v
      if (!(k in first)) first[k] = v
    }
    END {
      bad = ""
      for (i = 1; i <= nl; i++) {
        ok = 0; for (p = 1; p <= np; p++) if (keys[i] ~ pat[p]) { ok = 1; break }
        if (!ok) bad = (bad == "" ? keys[i] : bad " " keys[i])
      }
      if (bad != "") fail("unknown-key: front-matter key(s) nothing reads: " bad)
      nreq = split("schema id kind title description status scope composition order", req, " ")
      for (i = 1; i <= nreq; i++) if (first[req[i]] == "") fail("invalid-front-matter: lacks " req[i])
      if (first["providers.0"] == "") fail("invalid-front-matter: lacks providers (write providers: [\"*\"] for every provider)")
      if (first["kind"] != "context") fail("invalid-front-matter: kind is " first["kind"] ", not context")
      if (first["id"] !~ /^[a-z][a-z0-9-]*(\.[a-z0-9-]+)*$/) fail("invalid-front-matter: id \047" first["id"] "\047 is not a lower-case dotted identifier")
      if (first["status"] != "active" && first["status"] != "deprecated") fail("invalid-front-matter: status \047" first["status"] "\047 is neither active nor deprecated")
      if (first["scope"] != "directory" && first["scope"] != "subtree" && first["scope"] != "explicit") fail("invalid-front-matter: scope \047" first["scope"] "\047 is not directory, subtree or explicit")
      if (first["composition"] != "extend" && first["composition"] != "replace" && first["composition"] != "final") fail("invalid-front-matter: composition \047" first["composition"] "\047 is not extend, replace or final")
      if (first["order"] !~ /^-?[0-9]+$/) fail("invalid-front-matter: order \047" first["order"] "\047 is not an integer")
      if (first["scope"] == "explicit" && first["paths.0"] == "") fail("invalid-front-matter: scope explicit names no paths")
      if (first["scope"] != "explicit" && first["paths.0"] != "") fail("invalid-front-matter: paths are for scope explicit only (scope is " first["scope"] ")")
      if (first["composition"] == "replace" && first["supersedes.0"] == "") fail("invalid-front-matter: composition replace names nothing in supersedes")
      if (first["composition"] != "replace" && first["supersedes.0"] != "") fail("invalid-front-matter: supersedes is for composition replace only (composition is " first["composition"] ")")
      for (i = 1; i <= nl; i++) {
        if (keys[i] ~ /^providers\.[0-9]+$/ && vals[i] != "*" && index(provs, " " vals[i] " ") == 0)
          fail("unknown-provider: provider \047" vals[i] "\047 is not one the policy projects (have:" provs ")")
        if (keys[i] ~ /^audience\.[0-9]+$/ && vals[i] != "human" && vals[i] != "agent")
          fail("invalid-front-matter: audience \047" vals[i] "\047 is neither human nor agent")
      }
      nout = split("id title description status scope composition order", out, " ")
      for (i = 1; i <= nout; i++) print "docs." n "." out[i] "=" first[out[i]] >> flat
      print "docs." n ".path=" file >> flat
      print "docs." n ".dir=" dir >> flat
      print "docs." n ".depth=" depth >> flat
      has_aud = 0
      for (i = 1; i <= nl; i++) if (keys[i] ~ /^(paths|providers|audience|supersedes|tracks)\.[0-9]+$/) { print "docs." n "." keys[i] "=" vals[i] >> flat; if (keys[i] ~ /^audience/) has_aud = 1 }
      if (!has_aud) { print "docs." n ".audience.0=human" >> flat; print "docs." n ".audience.1=agent" >> flat }
    }' "$MJ_ALLOW_DIR/context.txt" "$tmp")"
  st=$?
  rm -f "$tmp"
  if [ "$st" != 0 ]; then
    mj_ctxd_problem "${reason%%:*}" "$rel" "${reason#*: }" "head -n 20 '$rel'"; return 1
  fi
  return 0
}

# ---------------------------------------------------------------- the tree
# Load every document once. Problems are collected, never fatal here: validate reports them
# all, resolve refuses on any. Cross-document checks run after every file is read.
mj_ctxd_load() {
  [ "$MJ_CTXD_LOADED" = 1 ] && [ -f "$MJ_CTXD_FLAT" ] && return 0
  MJ_CTXD_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.flat.XXXXXX")"
  MJ_CTXD_PROBLEMS="$(mktemp "${TMPDIR:-/tmp}/mj.ctxd.prob.XXXXXX")"
  MJ_CTXD_COUNT=0; MJ_CTXD_LOADED=1
  local conv providers f
  conv="$(mj_ctxd_conventions | tr '\n' ' ')"; providers="$(mj_ctxd_providers)"
  # (7) a symbolic link anywhere under the tree, file or directory, whatever its name: the
  # tree is read literally, and a link would let a document live outside what the manifest
  # names or appear under two paths
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    case "$f" in "$(mj_rel "$MJ_AI_LOCAL_DIR")"/*) continue ;; esac
    mj_ctxd_problem refused-path "$f" "is a symbolic link; the tree is read literally and links are not followed" "ls -l '$f'"
  done < <(cd "$MJ_ROOT" && find "$(mj_ctxd_tree)" -type l -print 2>/dev/null | LC_ALL=C sort)
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    if mj_ctxd_scan "$f" "$MJ_CTXD_COUNT" "$conv" "$providers"; then MJ_CTXD_COUNT=$((MJ_CTXD_COUNT + 1)); fi
  done < <(mj_ctxd_files)
  mj_ctxd_cross_check
  return 0
}

mj_ctxd_cross_check() {
  local noglob=0; case "$-" in *f*) noglob=1 ;; esac; set -f   # list values such as "*" are words, not globs
  local i j id other p rel tree
  tree="$(mj_ctxd_tree)"
  # one id, one file
  awk -F= '/^docs\.[0-9]+\.id=/ { print $2 }' "$MJ_CTXD_FLAT" | LC_ALL=C sort | uniq -d | while IFS= read -r id; do
    p="$(awk -F= -v id="$id" '/^docs\.[0-9]+\.id=/ && $2 == id { split($1, a, "."); n = a[2]; print n }' "$MJ_CTXD_FLAT" \
         | while IFS= read -r j; do printf '%s ' "$(mj_ctxd "$j" path)"; done)"
    mj_ctxd_problem duplicate-id "$id" "claimed by two files: ${p% }" "grep -rln '^id: $id$' $tree"
  done
  i=0
  while [ "$i" -lt "$MJ_CTXD_COUNT" ]; do
    rel="$(mj_ctxd "$i" path)"
    # explicit paths exist, are directories, and lie inside the tree
    for p in $(mj_ctxd_list "$i" paths); do
      if ! other="$(mj_norm_path "$p")"; then mj_ctxd_problem broken-reference "$rel" "paths entry '$p' is not a repository-relative path" "head -n 20 '$rel'"; continue; fi
      mj_path_contains "$tree" "$other" || { mj_ctxd_problem broken-reference "$rel" "paths entry '$p' lies outside the tree $tree/" "head -n 20 '$rel'"; continue; }
      [ -d "$MJ_ROOT/$other" ] || mj_ctxd_problem broken-reference "$rel" "paths entry '$p' is not a directory in this repository" "ls -d '$p'"
    done
    # every tracked pathspec covers at least one file the index knows
    for p in $(mj_ctxd_list "$i" tracks); do
      [ -n "$(mj_git ls-files -- "$p" 2>/dev/null | head -n 1)" ] \
        || mj_ctxd_problem broken-reference "$rel" "tracks '$p', which matches no tracked file" "git ls-files -- '$p'"
    done
    # supersedes: the target exists, is a proper ancestor-scope document, and is not final
    for id in $(mj_ctxd_list "$i" supersedes); do
      if ! j="$(mj_ctxd_index "$id")"; then mj_ctxd_problem broken-reference "$rel" "supersedes '$id', which no document declares" "majordomus context list"; continue; fi
      [ "$j" = "$i" ] && { mj_ctxd_problem cycle "$rel" "supersedes itself" "head -n 20 '$rel'"; continue; }
      if ! mj_path_contains "$(mj_ctxd "$j" dir)" "$(mj_ctxd "$i" dir)"; then
        mj_ctxd_problem broken-reference "$rel" "supersedes '$id' at $(mj_ctxd "$j" path), which is not in its ancestor chain; a document may only supersede what applies above it" "majordomus context explain $(mj_ctxd "$i" dir)"; continue
      fi
      [ "$(mj_ctxd "$j" composition)" = final ] \
        && mj_ctxd_problem illegal-override "$rel" "supersedes '$id', which is final; no descendant may weaken it" "majordomus context explain $(mj_ctxd "$i" dir)"
    done
    i=$((i + 1))
  done
  mj_ctxd_cycles
  [ "$noglob" = 1 ] || set +f
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
MJ_CTXD_TARGET=""; MJ_CTXD_TARGET_DIR=""
mj_ctxd_target() {
  local p="$1" real root
  case "$p" in "$MJ_ROOT"|"$MJ_ROOT"/*) p="${p#"$MJ_ROOT"}"; p="${p#/}" ;; esac
  [ -n "$p" ] || p="."
  if [ "$p" != "." ] && ! p="$(mj_norm_path "$p")"; then mj_die "$MJ_EX_REFUSED" "context: refused-path '$1' (absolute, or leaves the repository)"; fi
  [ -e "$MJ_ROOT/$p" ] || mj_die "$MJ_EX_MISSING" "context: no such path in $MJ_ROOT: $p"
  real="$(cd "$MJ_ROOT/$p" 2>/dev/null && pwd -P || (cd "$(dirname "$MJ_ROOT/$p")" && pwd -P))"
  root="$(cd "$MJ_ROOT" && pwd -P)"
  case "$real" in "$root"|"$root"/*) ;; *) mj_die "$MJ_EX_REFUSED" "context: refused-path '$p' resolves outside the repository ($real)" ;; esac
  MJ_CTXD_TARGET="$p"
  if [ -d "$MJ_ROOT/$p" ]; then MJ_CTXD_TARGET_DIR="$p"
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
        for p in $(mj_ctxd_list "$i" tracks); do
          for v in $(mj_git ls-files -- "$p" 2>/dev/null); do
            if mj_path_contains "$MJ_CTXD_TARGET" "$v" || mj_path_contains "$v" "$MJ_CTXD_TARGET"; then reason="tracks $p"; break; fi
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
  printf '{"index":%s,"id":"%s","path":"%s","dir":"%s","depth":%s,"scope":"%s","composition":"%s","order":%s,"status":"%s","title":"%s","providers":[' \
    "$idx" "$(mj_ctxd "$i" id)" "$(mj_json_esc "$(mj_ctxd "$i" path)")" "$(mj_json_esc "$(mj_ctxd "$i" dir)")" "$(mj_ctxd "$i" depth)" \
    "$(mj_ctxd "$i" scope)" "$(mj_ctxd "$i" composition)" "$(mj_ctxd "$i" order)" "$(mj_ctxd "$i" status)" "$(mj_json_esc "$(mj_ctxd "$i" title)")"
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
  case "$mode" in
    staged) mj_git diff --name-status -M --cached 2>/dev/null ;;
    base)   mj_git diff --name-status -M "$base" 2>/dev/null
            mj_git ls-files --others --exclude-standard 2>/dev/null | sed 's/^/A\t/' ;;
    *)      mj_git diff --name-status -M HEAD 2>/dev/null
            mj_git ls-files --others --exclude-standard 2>/dev/null | sed 's/^/A\t/' ;;
  esac | awk -F'\t' '{ s = substr($1, 1, 1); if (s == "R" || s == "C") print s "\t" $2 "\t" $3; else print s "\t" $2 "\t" }' | LC_ALL=C sort -t "$MJ_CTXD_TAB" -k2,2
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
