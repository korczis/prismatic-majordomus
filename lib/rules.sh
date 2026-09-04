#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_rules:-}" ] && return 0 || MJ_LIB_rules=1
# rules — the portable rule layer: the vendored baseline, the project's own rules, and the
# resolved effective set the dispatcher runs from.
#
# A rule is a Markdown file with YAML front matter under the repository's rules section.
# Identity is the front matter's id and version, never the file name. The effective set is
# additive — every active vendored rule plus every active project rule — resolved as a
# dependency graph in a deterministic order. A missing dependency, a cycle, two rules
# claiming one identity, or a project rule reusing a vendored namespace is an error, and
# an unresolved set is not applied partially: the command that needed it stops.
#
# Rules with an x-majordomus block are enforced: the dispatcher in doctrine.sh reads them
# as its registry. Rules without one are normative for whoever reads them and enforced by
# nobody, which is a fact `rules list` shows rather than hides.

MJ_RULES_FLAT=""          # rules.N.<field> for the effective set, resolved order
MJ_RULES_ERROR=""         # why the last load failed
MJ_RULES_LOADED=0
MJ_RULES_FORMAT="ai-rules/v1"
MJ_RULES_VENDOR_NS="majordomus"   # the namespace a project rule may not claim

mj_rules_vendor_dir()  { printf '%s' "$MJ_RULES_DIR/vendor/$MJ_RULES_VENDOR_NS"; }
mj_rules_project_dir() { printf '%s' "$MJ_RULES_DIR/project"; }

# ---------------------------------------------------------------- one rule file
# mj_rule_front FILE FLAT -> 0, or 1 with the reason on stdout
mj_rule_front() {
  local f="$1" flat="$2" fm k v
  fm="$(mktemp "${TMPDIR:-/tmp}/mj.rf.XXXXXX")"
  if ! mj_record_front "$f" > "$fm" 2>/dev/null || [ ! -s "$fm" ]; then rm -f "$fm"; printf 'no front matter'; return 1; fi
  # a fence that opens and never closes would read the whole file as front matter
  awk 'NR > 1 && $0 == "---" { f = 1; exit } END { exit !f }' "$f" || { rm -f "$fm"; printf 'front matter never closes (no second ---)'; return 1; }
  if ! mj_yaml_flatten "$fm" > "$flat" 2>/dev/null; then rm -f "$fm"; printf 'front matter does not parse'; return 1; fi
  rm -f "$fm"
  for k in id version kind title description statement status class; do
    [ -n "$(mj_yget "$flat" "$k")" ] || { printf 'front matter lacks %s' "$k"; return 1; }
  done
  grep -q '^depends_on' "$flat" || { printf 'front matter lacks depends_on (write depends_on: [] when there are none)'; return 1; }
  [ "$(mj_yget "$flat" kind)" = rule ] || { printf 'kind is %s, not rule' "$(mj_yget "$flat" kind)"; return 1; }
  case "$(mj_yget "$flat" version)" in ''|*[!0-9]*) printf 'version %s is not an integer' "$(mj_yget "$flat" version)"; return 1 ;; esac
  case "$(mj_yget "$flat" status)" in active|deprecated) ;; *) printf "status '%s' is neither active nor deprecated" "$(mj_yget "$flat" status)"; return 1 ;; esac
  case "$(mj_yget "$flat" class)" in blocking|advisory) ;; *) printf "class '%s' is neither blocking nor advisory" "$(mj_yget "$flat" class)"; return 1 ;; esac
  case "$(mj_yget "$flat" id)" in *[!A-Za-z0-9._-]*|"") printf "id '%s' is not a dotted identifier" "$(mj_yget "$flat" id)"; return 1 ;; esac
  v="$(mj_yaml_unknown_keys "$flat" "$MJ_ALLOW_DIR/rule.txt" || true)"
  [ -z "$v" ] || { printf 'unknown front-matter key(s): %s' "$(printf '%s' "$v" | tr '\n' ' ')"; return 1; }
  for v in $(mj_ylist "$flat" depends_on); do
    case "$v" in *@[0-9]*) ;; *) printf "depends_on '%s' is not an exact id@version reference" "$v"; return 1 ;; esac
  done
  if grep -q '^x-majordomus\.' "$flat"; then
    for k in validator category exit_code; do
      [ -n "$(mj_yget "$flat" "x-majordomus.$k")" ] || { printf 'x-majordomus lacks %s' "$k"; return 1; }
    done
    [ -n "$(mj_yget "$flat" x-majordomus.enforced_by.0)" ] || { printf 'x-majordomus names no enforcing command'; return 1; }
    [ -n "$(mj_yget "$flat" x-majordomus.tests.0)" ] || { printf 'x-majordomus names no test'; return 1; }
  fi
  return 0
}

# ---------------------------------------------------------------- the effective set
# Sources, in this order: the vendored package in its manifest order, then project rules
# in file-name order. Each file is validated, then the graph is resolved.
mj_rules_sources() {
  local vend proj f k n
  vend="$(mj_rules_vendor_dir)"; proj="$(mj_rules_project_dir)"
  if [ -f "$vend/manifest.yaml" ]; then
    local mf; mf="$(mktemp "${TMPDIR:-/tmp}/mj.vm.XXXXXX")"
    if mj_yaml_flatten "$vend/manifest.yaml" > "$mf" 2>/dev/null; then
      k=0
      while [ -n "$(mj_yget "$mf" "rules.$k.file")" ]; do
        printf 'vendor:%s\t%s\n' "$MJ_RULES_VENDOR_NS" "$vend/$(mj_yget "$mf" "rules.$k.file")"; k=$((k+1))
      done
    fi
    rm -f "$mf"
  fi
  if [ -d "$proj" ]; then
    for f in $(ls -1 "$proj"/*.md 2>/dev/null | LC_ALL=C sort); do
      [ "$(basename "$f")" = README.md ] && continue
      printf 'project\t%s\n' "$f"
    done
  fi
  return 0
}

mj_rules_load() {
  [ "$MJ_RULES_LOADED" = 1 ] && [ -f "$MJ_RULES_FLAT" ] && return 0
  MJ_RULES_ERROR=""
  [ -n "$MJ_RULES_DIR" ] || { MJ_RULES_ERROR="this layout has no rules section"; return 1; }
  local tmp graph prov f flat reason id ver key n=0 seen=" " d
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.rl.XXXXXX")"
  graph="$tmp/graph.tsv"; : > "$graph"
  while IFS="$(printf '\t')" read -r prov f; do
    [ -n "$f" ] || continue
    [ -f "$f" ] || { MJ_RULES_ERROR="$(mj_rel "$f"): listed by the vendor manifest but absent"; rm -rf "$tmp"; return 1; }
    flat="$tmp/$n.flat"
    if ! reason="$(mj_rule_front "$f" "$flat")"; then MJ_RULES_ERROR="$(mj_rel "$f"): $reason"; rm -rf "$tmp"; return 1; fi
    id="$(mj_yget "$flat" id)"; ver="$(mj_yget "$flat" version)"; key="$id@$ver"
    case "$seen" in *" $key "*) MJ_RULES_ERROR="$key is claimed twice ($(mj_rel "$f") and an earlier file)"; rm -rf "$tmp"; return 1 ;; esac
    if [ "$prov" = project ]; then
      case "$id" in "$MJ_RULES_VENDOR_NS".*) MJ_RULES_ERROR="$(mj_rel "$f"): a project rule may not claim the $MJ_RULES_VENDOR_NS namespace ($id); there is no override"; rm -rf "$tmp"; return 1 ;; esac
    fi
    seen="$seen$key "
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$n" "$key" "$(mj_ylist "$flat" depends_on | paste -sd, -)" "$(mj_yget "$flat" status)" "$prov" "$f" >> "$graph"
    n=$((n+1))
  done < <(mj_rules_sources)
  [ "$n" -gt 0 ] || { MJ_RULES_ERROR="no rules under $(mj_rel "$MJ_RULES_DIR") (vendor/$MJ_RULES_VENDOR_NS/ or project/)"; rm -rf "$tmp"; return 1; }

  # Resolve: every dependency of an active rule is an active rule, and the graph has no
  # cycle. Kahn's algorithm over the input order, so the result is deterministic and
  # respects the declared order wherever the graph leaves a choice.
  local order
  order="$(awk -F'\t' '
    { idx[$2] = $1; deps[$2] = $3; st[$2] = $4; keys[++n] = $2 }
    END {
      for (i = 1; i <= n; i++) {
        k = keys[i]; if (st[k] != "active") continue
        m = split(deps[k], d, ",")
        for (j = 1; j <= m; j++) { if (d[j] == "") continue
          if (!(d[j] in idx)) { printf "ERROR:%s depends on %s, which no rule provides\n", k, d[j]; exit 3 }
          if (st[d[j]] != "active") { printf "ERROR:%s depends on %s, which is %s\n", k, d[j], st[d[j]]; exit 3 }
          indeg[k]++; adj[d[j]] = adj[d[j]] " " k }
      }
      left = 0
      for (i = 1; i <= n; i++) if (st[keys[i]] == "active") left++
      while (left > 0) {
        picked = ""
        for (i = 1; i <= n; i++) { k = keys[i]; if (st[k] != "active" || done[k]) continue; if (indeg[k] + 0 == 0) { picked = k; break } }
        if (picked == "") {
          c = ""; for (i = 1; i <= n; i++) { k = keys[i]; if (st[k] == "active" && !done[k]) c = c " " k }
          printf "ERROR:dependency cycle among%s\n", c; exit 3 }
        done[picked] = 1; left--; print idx[picked]
        m = split(adj[picked], nx, " ")
        for (j = 1; j <= m; j++) if (nx[j] != "") indeg[nx[j]]--
      }
    }' "$graph")" || { MJ_RULES_ERROR="$(printf '%s' "$order" | sed -n 's/^ERROR://p' | head -n 1)"; rm -rf "$tmp"; return 1; }

  # Emit the effective set in resolved order as a flat registry.
  MJ_RULES_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.rules.XXXXXX")"
  local i=0 k
  for d in $order; do
    flat="$tmp/$d.flat"; f="$(awk -F'\t' -v i="$d" '$1==i {print $6}' "$graph")"; prov="$(awk -F'\t' -v i="$d" '$1==i {print $5}' "$graph")"
    {
      for k in id version title description statement status class; do printf 'rules.%s.%s=%s\n' "$i" "$k" "$(mj_yget "$flat" "$k")"; done
      printf 'rules.%s.provenance=%s\nrules.%s.file=%s\n' "$i" "$prov" "$i" "$(mj_rel "$f")"
      sed -n "s/^depends_on\./rules.$i.depends_on./p; s/^tags\./rules.$i.tags./p" "$flat"
      if grep -q '^x-majordomus\.' "$flat"; then
        printf 'rules.%s.enforced=1\n' "$i"
        sed -n "s/^x-majordomus\./rules.$i./p" "$flat"
      else printf 'rules.%s.enforced=0\n' "$i"; fi
    } >> "$MJ_RULES_FLAT"
    i=$((i+1))
  done
  rm -rf "$tmp"
  MJ_RULES_LOADED=1
  return 0
}

mj_rule()       { mj_yget "$MJ_RULES_FLAT" "rules.$1.$2"; }
mj_rule_list()  { mj_ylist "$MJ_RULES_FLAT" "rules.$1.$2"; }
mj_rule_count() { local i=0; while [ -n "$(mj_rule "$i" id)" ]; do i=$((i+1)); done; printf '%s' "$i"; }
mj_rule_index() { local i=0; while [ -n "$(mj_rule "$i" id)" ]; do [ "$(mj_rule "$i" id)" = "$1" ] && { printf '%s' "$i"; return 0; }; i=$((i+1)); done; return 1; }

# ---------------------------------------------------------------- package manifests
# The manifest names every rule file in a package with the hash of that file. It is the
# evidence doctor compares a vendored copy against, and the record `rules vendor` diffs.
# mj_rules_manifest_write PKG_DIR REVISION
mj_rules_manifest_write() {
  local pkg="$1" rev="$2" f fm fl tmp
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.pm.XXXXXX")"
  {
    printf '# The package manifest: every rule in this package, its identity, its file and the hash of\n'
    printf '# that file. Written by the tool, compared by doctor; a hand edit to a rule changes its\n'
    printf '# hash and is refused until the package is updated explicitly.\n'
    printf 'vendor: %s\npackage: %s-standard-rules\nversion: 1\nformat: %s\nsource_revision: %s\nrules:\n' \
      "$MJ_RULES_VENDOR_NS" "$MJ_RULES_VENDOR_NS" "$MJ_RULES_FORMAT" "$rev"
    for f in $(ls -1 "$pkg"/rules/*.md 2>/dev/null | LC_ALL=C sort); do
      fm="$(mktemp "${TMPDIR:-/tmp}/mj.pf.XXXXXX")"; fl="$(mktemp "${TMPDIR:-/tmp}/mj.pl.XXXXXX")"
      mj_record_front "$f" > "$fm" 2>/dev/null; mj_yaml_flatten "$fm" > "$fl" 2>/dev/null || true
      printf '  - id: %s\n    version: %s\n    file: rules/%s\n    sha256: %s\n' \
        "$(mj_yget "$fl" id)" "$(mj_yget "$fl" version)" "$(basename "$f")" "$(mj_sha256 "$f")"
      rm -f "$fm" "$fl"
    done
  } > "$tmp"
  mv "$tmp" "$pkg/manifest.yaml"
}

# mj_rules_manifest_check PKG_DIR -> prints one problem per line; exit 0 when the manifest
# describes the directory exactly
mj_rules_manifest_check() {
  local pkg="$1" mf k f listed=" " bad=0 fl fm
  [ -f "$pkg/manifest.yaml" ] || { printf 'manifest.yaml is absent\n'; return 1; }
  mf="$(mktemp "${TMPDIR:-/tmp}/mj.mc.XXXXXX")"
  mj_yaml_flatten "$pkg/manifest.yaml" > "$mf" 2>/dev/null || { rm -f "$mf"; printf 'manifest.yaml does not parse\n'; return 1; }
  [ "$(mj_yget "$mf" format)" = "$MJ_RULES_FORMAT" ] || { printf 'manifest format %s is not %s\n' "$(mj_yget "$mf" format)" "$MJ_RULES_FORMAT"; bad=1; }
  k=0
  while [ -n "$(mj_yget "$mf" "rules.$k.file")" ]; do
    f="$pkg/$(mj_yget "$mf" "rules.$k.file")"; listed="$listed$(mj_yget "$mf" "rules.$k.file") "
    if [ ! -f "$f" ]; then printf '%s is listed but absent\n' "$(mj_yget "$mf" "rules.$k.file")"; bad=1
    elif [ "$(mj_sha256 "$f")" != "$(mj_yget "$mf" "rules.$k.sha256")" ]; then printf '%s differs from its manifest hash (hand-edited?)\n' "$(mj_yget "$mf" "rules.$k.file")"; bad=1
    else
      fm="$(mktemp "${TMPDIR:-/tmp}/mj.mf.XXXXXX")"; fl="$(mktemp "${TMPDIR:-/tmp}/mj.ml.XXXXXX")"
      mj_record_front "$f" > "$fm" 2>/dev/null; mj_yaml_flatten "$fm" > "$fl" 2>/dev/null || true
      [ "$(mj_yget "$fl" id)@$(mj_yget "$fl" version)" = "$(mj_yget "$mf" "rules.$k.id")@$(mj_yget "$mf" "rules.$k.version")" ] \
        || { printf '%s declares %s@%s, the manifest says %s@%s\n' "$(mj_yget "$mf" "rules.$k.file")" "$(mj_yget "$fl" id)" "$(mj_yget "$fl" version)" "$(mj_yget "$mf" "rules.$k.id")" "$(mj_yget "$mf" "rules.$k.version")"; bad=1; }
      rm -f "$fm" "$fl"
    fi
    k=$((k+1))
  done
  for f in "$pkg"/rules/*.md; do
    [ -f "$f" ] || continue
    case "$listed" in *" rules/$(basename "$f") "*) ;; *) printf 'rules/%s is present but not in the manifest\n' "$(basename "$f")"; bad=1 ;; esac
  done
  rm -f "$mf"
  return $bad
}

# the manifest's version line: "<version> (<source_revision>)"
mj_rules_manifest_rev() {
  local mf; mf="$(mktemp "${TMPDIR:-/tmp}/mj.mr.XXXXXX")"
  mj_yaml_flatten "$1/manifest.yaml" > "$mf" 2>/dev/null || { rm -f "$mf"; printf 'unreadable'; return 0; }
  printf '%s (%s)' "$(mj_yget "$mf" version)" "$(mj_yget "$mf" source_revision)"; rm -f "$mf"
}

# ---------------------------------------------------------------- command
mj_cmd_rules() {
  local sub="${1:-list}"
  case "$sub" in
    --help|-h|help) mj_rules_usage; return 0 ;;
    list|show|vendor) shift || true ;;
    *) mj_die "$MJ_EX_USAGE" "rules: unknown subcommand '$sub' (see: majordomus rules --help)" ;;
  esac
  mj_require_installed
  [ -n "$MJ_RULES_DIR" ] || mj_die "$MJ_EX_MISSING" "this layout has no rules section (run: majordomus migrate)"
  case "$sub" in
    list)   mj_rules_list "$@" ;;
    show)   mj_rules_show "${1:-}" ;;
    vendor) mj_rules_vendor "$@" ;;
  esac
}

mj_rules_usage() {
  cat <<H
usage: majordomus rules list [--json]           the effective set in resolved order      (read-only)
       majordomus rules show <id>               one rule, front matter and body          (read-only)
       majordomus rules vendor status           the vendored baseline against the tool's (read-only)
       majordomus rules vendor diff             what an update would change              (read-only)
       majordomus rules vendor update [--force] replace the vendored baseline with the tool's package

  The effective set is every active rule vendored under rules/vendor/ plus every active
  rule under rules/project/, resolved as a dependency graph. A missing dependency, a cycle
  or a duplicate identity is an error and nothing is applied partially.
  vendor update is the only way the baseline changes: a newer executable reports it,
  never applies it. It refuses over a hand-edited vendor directory unless --force, and
  it never touches rules/project/.
H
}

mj_rules_list() {
  [ $# = 0 ] || mj_die "$MJ_EX_USAGE" "rules list: unknown option $1"
  mj_rules_load || mj_die "$MJ_EX_CONTRACT" "rules do not resolve: $MJ_RULES_ERROR"
  local i=0 first=1
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"rules":['
    while [ -n "$(mj_rule "$i" id)" ]; do
      [ "$first" = 1 ] || printf ','; first=0
      printf '{"id":"%s","version":%s,"class":"%s","status":"%s","provenance":"%s","file":"%s","enforced":%s,"depends_on":[%s]}' \
        "$(mj_rule "$i" id)" "$(mj_rule "$i" version)" "$(mj_rule "$i" class)" "$(mj_rule "$i" status)" \
        "$(mj_rule "$i" provenance)" "$(mj_json_esc "$(mj_rule "$i" file)")" "$([ "$(mj_rule "$i" enforced)" = 1 ] && printf true || printf false)" \
        "$(mj_rule_list "$i" depends_on | sed 's/^/"/; s/$/"/' | paste -sd, -)"
      i=$((i+1))
    done
    printf ']}\n'; return 0
  fi
  while [ -n "$(mj_rule "$i" id)" ]; do
    printf '%-42s v%-2s %-9s %-16s %s\n' "$(mj_rule "$i" id)" "$(mj_rule "$i" version)" "$(mj_rule "$i" class)" \
      "$(mj_rule "$i" provenance)" "$([ "$(mj_rule "$i" enforced)" = 1 ] && printf 'enforced by %s' "$(mj_rule_list "$i" enforced_by | paste -sd, -)" || printf 'not machine-enforced')"
    i=$((i+1))
  done
}

mj_rules_show() {
  local id="$1" i
  [ -n "$id" ] || mj_die "$MJ_EX_USAGE" "rules show needs an id (see: majordomus rules list)"
  mj_rules_load || mj_die "$MJ_EX_CONTRACT" "rules do not resolve: $MJ_RULES_ERROR"
  i="$(mj_rule_index "$id")" || mj_die "$MJ_EX_MISSING" "no rule '$id' in the effective set (see: majordomus rules list)"
  printf '# %s\n\n' "$(mj_rule "$i" file)"
  cat "$MJ_ROOT/$(mj_rule "$i" file)"
}

# ---------------------------------------------------------------- vendor
mj_rules_vendor() {
  local sub="${1:-status}" force=0 a
  shift || true
  for a in "$@"; do case "$a" in --force) force=1 ;; *) mj_die "$MJ_EX_USAGE" "rules vendor: unknown option $a" ;; esac; done
  local vend dist; vend="$(mj_rules_vendor_dir)"; dist="$MJ_STD_RULES_DIR"
  [ -f "$dist/manifest.yaml" ] || mj_die "$MJ_EX_INTERNAL" "the distribution ships no standard rule package at $dist"
  case "$sub" in
    status)
      if [ ! -f "$vend/manifest.yaml" ]; then
        printf 'vendored: none\ndistribution: %s\nnext: majordomus rules vendor update\n' "$(mj_rules_manifest_rev "$dist")"; exit "$MJ_EX_MISSING"; fi
      local probs; probs="$(mj_rules_manifest_check "$vend" || true)"
      printf 'vendored:     %s\ndistribution: %s\n' "$(mj_rules_manifest_rev "$vend")" "$(mj_rules_manifest_rev "$dist")"
      if [ -n "$probs" ]; then printf 'integrity:    %s\n' "$(printf '%s' "$probs" | head -n 1)"; exit "$MJ_EX_CONTRACT"; fi
      if diff -rq "$vend" "$dist" >/dev/null 2>&1; then printf 'state:        current\n'
      else printf 'state:        the distribution ships a different package; review with: majordomus rules vendor diff\n'; exit "$MJ_EX_DRIFT"; fi ;;
    diff)
      [ -d "$vend" ] || mj_die "$MJ_EX_MISSING" "nothing vendored yet (run: majordomus rules vendor update)"
      diff -ru "$vend" "$dist" && printf 'no difference\n'; return 0 ;;
    update)
      if [ -d "$vend" ] && [ "$force" != 1 ]; then
        local probs; probs="$(mj_rules_manifest_check "$vend" || true)"
        [ -z "$probs" ] || mj_die "$MJ_EX_REFUSED" "the vendored package was hand-edited ($(printf '%s' "$probs" | head -n 1)); review with: majordomus rules vendor diff, then --force"
      fi
      mj_rules_vendor_install "$dist" "$vend"
      mj_ledger_append rules.vendored "\"package\":\"$(mj_json_esc "$(mj_rules_manifest_rev "$vend")")\""
      printf 'vendored %s into %s\n' "$(mj_rules_manifest_rev "$vend")" "$(mj_rel "$vend")" ;;
    *) mj_die "$MJ_EX_USAGE" "rules vendor: unknown subcommand '$sub' (status|diff|update)" ;;
  esac
}

# copy a package into place atomically: stage beside the target under one temporary
# directory, swap, and remove only that directory
mj_rules_vendor_install() {
  local src="$1" dst="$2" tmp
  mkdir -p "$(dirname "$dst")"
  tmp="$(mktemp -d "$(dirname "$dst")/.vendor.XXXXXX")"
  mkdir -p "$tmp/new"
  cp -R "$src/." "$tmp/new/"
  [ -d "$dst" ] && mv "$dst" "$tmp/old"
  if mv "$tmp/new" "$dst"; then rm -rf "$tmp"
  else
    [ -d "$tmp/old" ] && mv "$tmp/old" "$dst"
    rm -rf "$tmp"; mj_die "$MJ_EX_INTERNAL" "could not install the package into $(mj_rel "$dst")"
  fi
}
