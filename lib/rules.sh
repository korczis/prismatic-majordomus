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
# The front matter is cut out, flattened by the parser every other record uses, and
# validated and emitted by one awk program: three processes per rule, in one pipeline,
# instead of a few dozen. The messages are the contract 67_rule_dag asserts.
#
# mj_rule_scan FILE PROV N TMP -> 0 with TMP/N.flat written, the graph row appended to
# TMP/graph.tsv and MJ_RULE_KEY set; or 1 with the reason in MJ_RULE_REASON
MJ_RULE_KEY=""; MJ_RULE_REASON=""
mj_rule_scan() {
  local f="$1" prov="$2" n="$3" tmp="$4" rel="${1#"$MJ_ROOT/"}" st
  MJ_RULE_KEY=""; MJ_RULE_REASON=""
  # stage 1: the front matter, or exit 2 (none) / 4 (a fence that never closes)
  if awk 'NR == 1 && $0 != "---" { exit 2 }
          NR > 1 && $0 == "---" { c = 1; exit }
          NR > 1 { n++; print }
          END { if (!c) exit (n ? 4 : 2); if (!n) exit 2 }' "$f" \
    | mj_yaml_flatten - 2>/dev/null \
    | awk -v prov="$prov" -v file="$rel" -v path="$f" -v n="$n" -v ns="$MJ_RULES_VENDOR_NS" \
          -v flat="$tmp/$n.flat" -v graph="$tmp/graph.tsv" '
      function fail(m) { print m > "/dev/stderr"; exit 1 }
      FNR == NR { pat[++np] = $0; next }
      {
        eq = index($0, "="); if (eq == 0) next
        k = substr($0, 1, eq - 1); v = substr($0, eq + 1)
        nl++; keys[nl] = k; vals[nl] = v
        if (!(k in first)) first[k] = v
      }
      END {
        nreq = split("id version kind title description statement status class", req, " ")
        for (i = 1; i <= nreq; i++) if (first[req[i]] == "") fail("front matter lacks " req[i])
        has_dep = 0; for (i = 1; i <= nl; i++) if (index(keys[i], "depends_on") == 1) has_dep = 1
        if (!has_dep) fail("front matter lacks depends_on (write depends_on: [] when there are none)")
        if (first["kind"] != "rule") fail("kind is " first["kind"] ", not rule")
        if (first["version"] !~ /^[0-9]+$/) fail("version " first["version"] " is not an integer")
        if (first["status"] != "active" && first["status"] != "deprecated") fail("status \047" first["status"] "\047 is neither active nor deprecated")
        if (first["class"] != "blocking" && first["class"] != "advisory") fail("class \047" first["class"] "\047 is neither blocking nor advisory")
        if (first["id"] ~ /[^A-Za-z0-9._-]/) fail("id \047" first["id"] "\047 is not a dotted identifier")
        bad = ""
        for (i = 1; i <= nl; i++) {
          ok = 0; for (p = 1; p <= np; p++) if (keys[i] ~ pat[p]) { ok = 1; break }
          if (!ok) bad = (bad == "" ? keys[i] : bad " " keys[i])
        }
        if (bad != "") fail("unknown front-matter key(s): " bad)
        deps = ""
        for (i = 1; i <= nl; i++) if (keys[i] ~ /^depends_on\.[0-9]+$/) {
          m = split(vals[i], tok, /[ \t]+/)
          for (j = 1; j <= m; j++) if (tok[j] != "" && tok[j] !~ /@[0-9]/) fail("depends_on \047" tok[j] "\047 is not an exact id@version reference")
          deps = (deps == "" ? vals[i] : deps "," vals[i])
        }
        enforced = 0; for (i = 1; i <= nl; i++) if (index(keys[i], "x-majordomus.") == 1) enforced = 1
        if (enforced) {
          if (first["x-majordomus.validator"] == "") fail("x-majordomus lacks validator")
          if (first["x-majordomus.category"] == "") fail("x-majordomus lacks category")
          if (first["x-majordomus.exit_code"] == "") fail("x-majordomus lacks exit_code")
          if (first["x-majordomus.enforced_by.0"] == "") fail("x-majordomus names no enforcing command")
          if (first["x-majordomus.tests.0"] == "") fail("x-majordomus names no test")
        }
        # the flat record, in the order the registry is read in; @ is the resolved index
        nout = split("id version title description statement status class", out, " ")
        for (i = 1; i <= nout; i++) print "rules.@." out[i] "=" first[out[i]] > flat
        print "rules.@.provenance=" prov > flat
        print "rules.@.file=" file > flat
        for (i = 1; i <= nl; i++) if (keys[i] ~ /^depends_on\./ || keys[i] ~ /^tags\./) print "rules.@." keys[i] "=" vals[i] > flat
        if (enforced) {
          print "rules.@.enforced=1" > flat
          for (i = 1; i <= nl; i++) if (index(keys[i], "x-majordomus.") == 1) print "rules.@." substr(keys[i], 14) "=" vals[i] > flat
        } else print "rules.@.enforced=0" > flat
        key = first["id"] "@" first["version"]
        print n "\t" key "\t" deps "\t" first["status"] "\t" prov "\t" path >> graph
        print key
      }' "$MJ_ALLOW_DIR/rule.txt" - > "$tmp/$n.key" 2> "$tmp/$n.reason"
  then st=("${PIPESTATUS[@]}"); else st=("${PIPESTATUS[@]}"); fi
  case "${st[0]}" in
    0) ;;
    4) MJ_RULE_REASON='front matter never closes (no second ---)'; return 1 ;;
    *) MJ_RULE_REASON='no front matter'; return 1 ;;
  esac
  [ "${st[1]}" = 0 ] || { MJ_RULE_REASON='front matter does not parse'; return 1; }
  if [ "${st[2]}" != 0 ]; then IFS= read -r MJ_RULE_REASON < "$tmp/$n.reason" || true; return 1; fi
  IFS= read -r MJ_RULE_KEY < "$tmp/$n.key" || true
  return 0
}

# ---------------------------------------------------------------- the effective set
# Sources, in this order: the vendored package in its manifest order, then project rules
# in file-name order. Each file is validated, then the graph is resolved.
mj_rules_sources() {
  local vend proj f
  vend="$MJ_RULES_DIR/vendor/$MJ_RULES_VENDOR_NS"; proj="$MJ_RULES_DIR/project"
  if [ -f "$vend/manifest.yaml" ]; then
    mj_yaml_flatten "$vend/manifest.yaml" 2>/dev/null \
      | awk -v v="vendor:$MJ_RULES_VENDOR_NS" -v d="$vend" '
          index($0, "rules.") == 1 {
            eq = index($0, "="); k = substr($0, 1, eq - 1)
            if (k ~ /^rules\.[0-9]+\.file$/) { f = substr($0, eq + 1); if (f == "") exit; print v "\t" d "/" f }
          }'
  fi
  if [ -d "$proj" ]; then
    for f in $(ls -1 "$proj"/*.md 2>/dev/null | LC_ALL=C sort); do
      [ "${f##*/}" = README.md ] && continue
      printf 'project\t%s\n' "$f"
    done
  fi
  return 0
}

mj_rules_load() {
  [ "$MJ_RULES_LOADED" = 1 ] && [ -f "$MJ_RULES_FLAT" ] && return 0
  MJ_RULES_ERROR=""
  [ -n "$MJ_RULES_DIR" ] || { MJ_RULES_ERROR="this layout has no rules section"; return 1; }
  local tmp graph prov f n=0 seen=" " d
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.rl.XXXXXX")"
  graph="$tmp/graph.tsv"; : > "$graph"
  while IFS=$'\t' read -r prov f; do
    [ -n "$f" ] || continue
    [ -f "$f" ] || { MJ_RULES_ERROR="${f#"$MJ_ROOT/"}: listed by the vendor manifest but absent"; rm -rf "$tmp"; return 1; }
    if ! mj_rule_scan "$f" "$prov" "$n" "$tmp"; then MJ_RULES_ERROR="${f#"$MJ_ROOT/"}: $MJ_RULE_REASON"; rm -rf "$tmp"; return 1; fi
    case "$seen" in *" $MJ_RULE_KEY "*) MJ_RULES_ERROR="$MJ_RULE_KEY is claimed twice (${f#"$MJ_ROOT/"} and an earlier file)"; rm -rf "$tmp"; return 1 ;; esac
    if [ "$prov" = project ]; then
      case "$MJ_RULE_KEY" in "$MJ_RULES_VENDOR_NS".*) MJ_RULES_ERROR="${f#"$MJ_ROOT/"}: a project rule may not claim the $MJ_RULES_VENDOR_NS namespace (${MJ_RULE_KEY%@*}); there is no override"; rm -rf "$tmp"; return 1 ;; esac
    fi
    seen="$seen$MJ_RULE_KEY "
    n=$((n+1))
  done < <(mj_rules_sources)
  [ "$n" -gt 0 ] || { MJ_RULES_ERROR="no rules under ${MJ_RULES_DIR#"$MJ_ROOT/"} (vendor/$MJ_RULES_VENDOR_NS/ or project/)"; rm -rf "$tmp"; return 1; }

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

  # Emit the effective set in resolved order as a flat registry: one pass over the
  # per-rule records, numbering each by its position in the order.
  MJ_RULES_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.rules.XXXXXX")"
  set --
  for d in $order; do set -- "$@" "$tmp/$d.flat"; done
  awk 'FNR == 1 { i++ } { sub(/^rules\.@\./, "rules." (i - 1) "."); print }' "$@" > "$MJ_RULES_FLAT"
  rm -rf "$tmp"
  MJ_RULES_LOADED=1
  return 0
}

mj_rule()       { mj_yget "$MJ_RULES_FLAT" "rules.$1.$2"; }
mj_rule_list()  { mj_ylist "$MJ_RULES_FLAT" "rules.$1.$2"; }
mj_rule_count() { awk 'index($0, "rules.") == 1 && $0 ~ /^rules\.[0-9]+\.id=/ { n++ } END { printf "%s", n + 0 }' "$MJ_RULES_FLAT"; }
mj_rule_index() {
  awk -v id="$1" 'index($0, "rules.") == 1 && $0 ~ /^rules\.[0-9]+\.id=/ && substr($0, index($0, "=") + 1) == id { split($0, p, "."); printf "%s", p[2]; f = 1; exit } END { exit !f }' "$MJ_RULES_FLAT"
}
# one pass over the registry: every rule's row for `rules list`, text or JSON
mj_rules_render() {
  awk -v json="$1" '
    function jesc(s,  o, i, c) { o = ""; for (i = 1; i <= length(s); i++) { c = substr(s, i, 1); if (c == "\\") o = o "\\\\"; else if (c == "\"") o = o "\\\""; else if (c != "\n") o = o c } return o }
    {
      eq = index($0, "="); k = substr($0, 1, eq - 1); v = substr($0, eq + 1)
      if (split(k, p, ".") < 3 || p[1] != "rules") next
      i = p[2] + 0; if (i + 1 > n) n = i + 1
      f = substr(k, length("rules." p[2] ".") + 1)
      if (f == "id") id[i] = v; else if (f == "version") ver[i] = v; else if (f == "class") cls[i] = v
      else if (f == "status") st[i] = v; else if (f == "provenance") prov[i] = v; else if (f == "file") file[i] = v
      else if (f == "enforced") enf[i] = v
      else if (f ~ /^enforced_by\.[0-9]+$/) eb[i] = (i in eb ? eb[i] "," v : v)
      else if (f ~ /^depends_on\.[0-9]+$/) dep[i] = (i in dep ? dep[i] ",\"" v "\"" : "\"" v "\"")
    }
    END {
      if (json) {
        printf "{\"schema\":1,\"rules\":["
        for (i = 0; i < n; i++) printf "%s{\"id\":\"%s\",\"version\":%s,\"class\":\"%s\",\"status\":\"%s\",\"provenance\":\"%s\",\"file\":\"%s\",\"enforced\":%s,\"depends_on\":[%s]}", (i ? "," : ""), id[i], ver[i], cls[i], st[i], prov[i], jesc(file[i]), (enf[i] == 1 ? "true" : "false"), dep[i]
        printf "]}\n"
      } else
        for (i = 0; i < n; i++) printf "%-42s v%-2s %-9s %-16s %s\n", id[i], ver[i], cls[i], prov[i], (enf[i] == 1 ? "enforced by " eb[i] : "not machine-enforced")
    }' "$MJ_RULES_FLAT"
}

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
  local pkg="$1" bad=0 f file sha id ver fid h have="" tmp fmt
  [ -f "$pkg/manifest.yaml" ] || { printf 'manifest.yaml is absent\n'; return 1; }
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.mc.XXXXXX")"
  mj_yaml_flatten "$pkg/manifest.yaml" > "$tmp/flat" 2>/dev/null || { rm -rf "$tmp"; printf 'manifest.yaml does not parse\n'; return 1; }
  # the format line, then one row per entry in manifest order — k, file, sha256, id,
  # version — stopping at the first entry without a file, as the reader always has
  awk '
    { eq = index($0, "="); k = substr($0, 1, eq - 1); v = substr($0, eq + 1)
      if (k == "format" && !("format" in seen)) { seen["format"] = 1; fmt = v }
      else if (k ~ /^rules\.[0-9]+\.(file|sha256|id|version)$/) { split(k, p, "."); if (!((p[2] "." p[3]) in seen)) { seen[p[2] "." p[3]] = 1; val[p[2] "." p[3]] = v; if (p[2] + 0 > max) max = p[2] + 0; any = 1 } } }
    END {
      print "format=" fmt
      for (i = 0; any && i <= max; i++) { if (val[i ".file"] == "") exit; printf "%s\t%s\t%s\t%s\t%s\n", i, val[i ".file"], val[i ".sha256"], val[i ".id"], val[i ".version"] }
    }' "$tmp/flat" > "$tmp/entries"
  IFS= read -r fmt < "$tmp/entries" || fmt="format="
  fmt="${fmt#format=}"
  [ "$fmt" = "$MJ_RULES_FORMAT" ] || { printf 'manifest format %s is not %s\n' "$fmt" "$MJ_RULES_FORMAT"; bad=1; }
  # one hash run over every listed file that exists, in manifest order
  : > "$tmp/existing"
  while IFS=$'\t' read -r _ file sha id ver; do
    [ -f "$pkg/$file" ] && printf '%s\n' "$pkg/$file" >> "$tmp/existing"
  done < <(tail -n +2 "$tmp/entries")
  : > "$tmp/hashes"; : > "$tmp/idents"
  if [ -s "$tmp/existing" ]; then
    if command -v sha256sum >/dev/null 2>&1; then xargs sha256sum < "$tmp/existing" > "$tmp/hashes"
    elif command -v shasum >/dev/null 2>&1; then xargs shasum -a 256 < "$tmp/existing" > "$tmp/hashes"
    else rm -rf "$tmp"; mj_die "$MJ_EX_MISSING" "need sha256sum or shasum"; fi
    # and one pipeline over their front matter for the identity each declares: the same
    # cut and the same parser as everywhere else, one marker line per file
    if ! xargs awk 'FNR == 1 { print "mjfile: " (++k); fm = ($0 == "---"); next } fm && $0 == "---" { fm = 0; next } fm { print }' < "$tmp/existing" \
        | mj_yaml_flatten - 2>/dev/null \
        | awk '{ eq = index($0, "="); k = substr($0, 1, eq - 1); v = substr($0, eq + 1)
                 if (k == "mjfile") { if (n) print id "@" ver; n++; id = ""; ver = ""; i = 0; w = 0 }
                 else if (k == "id" && !i) { id = v; i = 1 } else if (k == "version" && !w) { ver = v; w = 1 } }
               END { if (n) print id "@" ver }' > "$tmp/idents"; then
      # a front matter the parser refuses would have cut the batch short: fall back to one
      # pipeline per file, which is what the check did before it was batched
      : > "$tmp/idents"
      while IFS= read -r f; do
        { mj_record_front "$f" 2>/dev/null | mj_yaml_flatten - 2>/dev/null | awk '{ eq = index($0, "="); k = substr($0, 1, eq - 1); v = substr($0, eq + 1); if (k == "id" && !i) { id = v; i = 1 } else if (k == "version" && !w) { ver = v; w = 1 } } END { print id "@" ver }' || true; } >> "$tmp/idents"
      done < "$tmp/existing"
    fi
  fi
  exec 3< "$tmp/hashes" 4< "$tmp/idents"
  while IFS=$'\t' read -r _ file sha id ver; do
    if [ ! -f "$pkg/$file" ]; then printf '%s is listed but absent\n' "$file"; bad=1; continue; fi
    IFS= read -r h <&3 || h=""; h="${h%% *}"
    IFS= read -r fid <&4 || fid="@"
    if [ "$h" != "$sha" ]; then printf '%s differs from its manifest hash (hand-edited?)\n' "$file"; bad=1
    elif [ "$fid" != "$id@$ver" ]; then printf '%s declares %s@%s, the manifest says %s@%s\n' "$file" "${fid%@*}" "${fid#*@}" "$id" "$ver"; bad=1; fi
  done < <(tail -n +2 "$tmp/entries")
  exec 3<&- 4<&-
  have="$(awk -F'\t' 'NR > 1 { printf " %s", $2 }' "$tmp/entries") "
  for f in "$pkg"/rules/*.md; do
    [ -f "$f" ] || continue
    case "$have" in *" rules/${f##*/} "*) ;; *) printf 'rules/%s is present but not in the manifest\n' "${f##*/}"; bad=1 ;; esac
  done
  rm -rf "$tmp"
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
  mj_rules_render "$([ "$MJ_JSON" = 1 ] && printf 1 || printf 0)"
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
