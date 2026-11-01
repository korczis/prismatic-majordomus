#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_fleet:-}" ] && return 0 || MJ_LIB_fleet=1
# fleet — the owner's fleet pack: rules, skills and kit profiles that one person keeps for
# every repository they supervise, delivered beside the vendored baseline (I4336).
#
# The pack is a directory on the owner's machine, never in a repository:
# $MAJORDOMUS_FLEET_PACK, else $XDG_CONFIG_HOME/majordomus/fleet (an absolute XDG_CONFIG_HOME
# only), else ~/.config/majordomus/fleet; MAJORDOMUS_FLEET=off reads as no pack anywhere. Its
# manifest.yaml (ai-fleet-pack/v1) says which repositories it applies to and lists every
# file it delivers; a file the manifest does not
# list is never copied, whatever else the directory holds — the owner's licensed reference
# material may sit beside the pack and must not travel with it.
#
#   schema: ai-fleet-pack/v1
#   version: 3                          # an integer; an installed copy older than it lags
#   applies_to:
#     remotes: ["*github.com*owner/*"]  # globs on the origin URL
#     paths: ["*/dev/*"]                # globs on the repository root
#   excludes: ["*/scratch/*"]           # globs on either; any match excludes
#   entries:
#     - { path: rules/<id>.v1.md, kind: rule }
#     - { path: skills/<id>/SKILL.md, kind: skill }   # becomes skills/fleet-<id>/SKILL.md
#     - { path: kits/<kit>.yaml, kind: kit, visibility: private }
#
# Rules and kits land under rules/vendor/fleet/, skills under skills/fleet-<id>/, and the
# stamp rules/vendor/fleet/MANIFEST names the pack version and the sha256 of every file it
# installed, so a hand edit is found on a fresh clone with nothing else to compare against.
# Into a public tree (policy ui.visibility; MAJORDOMUS_VISIBILITY=public tightens it) an entry
# marked `visibility: private` is skipped, and an entry that carries a home or absolute path
# (MJ_FLEET_HOME_PATH_ERE), or whose bytes are a licensed file of the kit evidence, refuses the
# whole install: nothing is applied partially. A public tree with a kit in play and no
# evidence to judge it by is refused as not judged, never installed unjudged. No pack, or a
# pack that does not apply, writes nothing.

MJ_FLEET_FORMAT="ai-fleet-pack/v1"
MJ_FLEET_STAMP_FORMAT="ai-fleet-stamp/v1"
MJ_FLEET_NS="fleet"                     # rules/vendor/<ns>/, and the rule provenance vendor:<ns>
MJ_FLEET_EVIDENCE_FORMAT="kit-evidence/v1"
MJ_FLEET_EVIDENCE_PREFIX="MAJORDOMUS_KIT_EVIDENCE_"
# A home or absolute path in a file a public tree would carry: the owner's machine, named.
# Anywhere on a line: /Users/, /home/, $HOME, ${HOME}, and ~/ before a non-space. At the start
# of a line or after a space, quote, parenthesis, `=`, `:` or backtick (so that a URL's path
# is not one): a filesystem root that is no URL path, /root/ /opt/ /Volumes/ /private/ /mnt/
# /srv/ /var/ /tmp/ /etc/ /usr/, or a drive, C:\.
MJ_FLEET_HOME_PATH_ERE='/Users/|/home/|\$HOME|\$\{HOME\}|~/[^[:space:]]|(^|[[:space:]"'\''(=:`])(/(root|opt|Volumes|private|mnt|srv|var|tmp|etc|usr)/|[A-Za-z]:\\)'
MJ_FLEET_ERROR=""; MJ_FLEET_VERSION=""; MJ_FLEET_WHY=""

# the rule scanner judges a staged rule before it is installed
# shellcheck source=rules.sh
. "$MJ_LIB_DIR/rules.sh"

mj_fleet_dir()   { printf '%s' "$MJ_RULES_DIR/vendor/$MJ_FLEET_NS"; }
mj_fleet_stamp() { printf '%s' "$(mj_fleet_dir)/MANIFEST"; }

# where the pack is looked for; printed whether or not it exists
mj_fleet_source() {
  if [ -n "${MAJORDOMUS_FLEET_PACK:-}" ]; then printf '%s' "$MAJORDOMUS_FLEET_PACK"; return 0; fi
  case "${XDG_CONFIG_HOME:-}" in /*) printf '%s/majordomus/fleet' "$XDG_CONFIG_HOME"; return 0 ;; esac
  [ -n "${HOME:-}" ] || return 1
  printf '%s/.config/majordomus/fleet' "$HOME"
}
# whether SRC holds a pack. MAJORDOMUS_FLEET=off says there is none, wherever one sits: a
# recorder whose output is published evidence must not depend on the owner's machine (I4521)
mj_fleet_has_pack() { [ "${MAJORDOMUS_FLEET:-}" != off ] && [ -f "$1/manifest.yaml" ]; }
mj_fleet_present() { local s; s="$(mj_fleet_source)" && mj_fleet_has_pack "$s"; }

# a path as the owner's machine may be printed: under the home directory as ~/...
mj_fleet_tilde() {
  case "$1" in "${HOME:-/nonexistent}"/*) printf '%s/%s' '~' "${1#"$HOME/"}" ;; *) printf '%s' "$1" ;; esac
}

# The one optional reader of the policy's ui block. A policy written before the block
# existed declares no kit, and a tree that does not say it is private is judged public:
# public is the answer that refuses, so the absent key cannot loosen anything.
# MAJORDOMUS_VISIBILITY=public tightens a private declaration; no value loosens one.
mj_fleet_policy_ready() {
  [ -n "${MJ_POL_FLAT:-}" ] && [ -f "$MJ_POL_FLAT" ] && return 0
  [ -f "${MJ_POLICY_FILE:-}" ] && mj_load_policy 2>/dev/null
}
mj_fleet_visibility() {
  local v=""
  mj_fleet_policy_ready && v="$(mj_pol ui.visibility)"
  [ "${MAJORDOMUS_VISIBILITY:-}" = public ] && v=public
  case "$v" in private) printf private ;; *) printf public ;; esac
}
mj_fleet_kit() { mj_fleet_policy_ready && mj_pol ui.kit; return 0; }

# ---------------------------------------------------------------- the manifest
# Flow maps (`- { path: x, kind: rule }`) are the manifest's natural form and the subset
# reader keeps them as one scalar; this expands one level of them, lists inside included,
# into the same dotted lines the reader prints for the block form.
MJ_FLEET_FLOW_AWK='
  function trim(s) { sub(/^[ \t]+/, "", s); sub(/[ \t]+$/, "", s); return s }
  function unq(v) {
    v = trim(v)
    if (v ~ /^\047.*\047$/) { v = substr(v, 2, length(v) - 2); gsub(/\047\047/, "\047", v); return v }
    if (v ~ /^".*"$/) return substr(v, 2, length(v) - 2)
    return v
  }
  function emit(p, v,   n, i, parts) {
    v = trim(v)
    if (v ~ /^\[.*\]$/) {
      v = trim(substr(v, 2, length(v) - 2))
      if (v == "") { print p "=[]"; return }
      n = split(v, parts, /[ \t]*,[ \t]*/)
      for (i = 1; i <= n; i++) print p "." (i - 1) "=" unq(parts[i])
      return
    }
    print p "=" unq(v)
  }
  {
    eq = index($0, "="); k = substr($0, 1, eq - 1); v = substr($0, eq + 1)
    if (v !~ /^\{.*\}$/) { print; next }
    body = substr(v, 2, length(v) - 2); depth = 0; cur = ""; n = 0
    for (i = 1; i <= length(body); i++) {
      c = substr(body, i, 1)
      if (c == "[") depth++; else if (c == "]") depth--
      if (c == "," && depth == 0) { item[++n] = cur; cur = ""; continue }
      cur = cur c
    }
    if (trim(cur) != "") item[++n] = cur
    for (j = 1; j <= n; j++) {
      it = trim(item[j]); c2 = index(it, ":")
      if (c2 == 0) { print "ERROR:" k ": a flow entry without a key" > "/dev/stderr"; exit 3 }
      emit(k "." trim(substr(it, 1, c2 - 1)), substr(it, c2 + 1))
    }
  }'

# mj_fleet_load SRC OUT -> 0 with OUT/flat and OUT/entries (path, kind, visibility per line)
# written and MJ_FLEET_VERSION set; 1 with the reason in MJ_FLEET_ERROR
mj_fleet_load() {
  local src="$1" out="$2" bad
  MJ_FLEET_ERROR=""; MJ_FLEET_VERSION=""
  if ! { mj_yaml_flatten "$src/manifest.yaml" 2>/dev/null | awk "$MJ_FLEET_FLOW_AWK" > "$out/flat"; } 2>/dev/null \
     || ! [ -s "$out/flat" ]; then
    MJ_FLEET_ERROR="manifest.yaml does not parse"; return 1
  fi
  # a closed key set: a key this reader does not know is a pack written for another reader
  bad="$(awk '{ k = substr($0, 1, index($0, "=") - 1) }
    k !~ /^(schema|version|applies_to\.(remotes|paths)(\.[0-9]+)?|excludes(\.[0-9]+)?|entries\.[0-9]+\.(path|kind|visibility))$/ { print k; exit }' "$out/flat")"
  [ -z "$bad" ] || { MJ_FLEET_ERROR="manifest.yaml carries the unknown key $bad"; return 1; }
  [ "$(mj_yget "$out/flat" schema)" = "$MJ_FLEET_FORMAT" ] \
    || { MJ_FLEET_ERROR="manifest.yaml is not $MJ_FLEET_FORMAT (schema: $(mj_yget "$out/flat" schema))"; return 1; }
  MJ_FLEET_VERSION="$(mj_yget "$out/flat" version)"
  case "$MJ_FLEET_VERSION" in ''|*[!0-9]*) MJ_FLEET_ERROR="manifest.yaml version '$MJ_FLEET_VERSION' is not an integer"; return 1 ;; esac
  awk -F'=' '
    { eq = index($0, "="); k = substr($0, 1, eq - 1); v = substr($0, eq + 1) }
    k ~ /^entries\.[0-9]+\./ { split(k, p, "."); i = p[2] + 0; f[i, p[3]] = v; if (i > max) max = i; any = 1 }
    END { for (i = 0; any && i <= max; i++) printf "%s\t%s\t%s\n", f[i, "path"], f[i, "kind"], f[i, "visibility"] }' \
    "$out/flat" > "$out/entries"
  local path kind vis seen=" "
  while IFS=$'\t' read -r path kind vis; do
    case "$kind" in
      rule)  [[ "$path" =~ ^rules/[A-Za-z0-9._-]+\.v[0-9]+\.md$ ]] ;;
      skill) [[ "$path" =~ ^skills/[a-z0-9][a-z0-9-]*/SKILL\.md$ ]] ;;
      kit)   [[ "$path" =~ ^kits/[a-z0-9][a-z0-9_-]*\.yaml$ ]] ;;
      *) MJ_FLEET_ERROR="entry '$path' has kind '$kind' (rule | skill | kit)"; return 1 ;;
    esac || { MJ_FLEET_ERROR="entry '$path' is not where a $kind lives (rules/<id>.v<n>.md, skills/<id>/SKILL.md, kits/<kit>.yaml)"; return 1; }
    case "$path" in *..*) MJ_FLEET_ERROR="entry '$path' leaves the pack"; return 1 ;; esac
    case "$vis" in ''|any|private) ;; *) MJ_FLEET_ERROR="entry '$path' has visibility '$vis' (private, or absent for any)"; return 1 ;; esac
    case "$seen" in *" $path "*) MJ_FLEET_ERROR="entry '$path' is listed twice"; return 1 ;; esac
    seen="$seen$path "
  done < "$out/entries"
  [ -s "$out/entries" ] || { MJ_FLEET_ERROR="manifest.yaml lists no entries"; return 1; }
  return 0
}

# mj_fleet_applies FLAT -> 0 when the pack applies to this repository; 1 with MJ_FLEET_WHY
mj_fleet_applies() {
  local flat="$1" origin root phys g hit=""
  origin="$(mj_git remote get-url origin 2>/dev/null || true)"
  root="$MJ_ROOT"; phys="$(cd "$MJ_ROOT" 2>/dev/null && pwd -P)" || phys="$root"
  # the globs are patterns on purpose: unquoted in `case`, which is what makes them globs
  while IFS= read -r g; do
    [ -n "$g" ] || continue
    # shellcheck disable=SC2254
    case "$origin" in $g) [ -n "$origin" ] && { MJ_FLEET_WHY="the origin matches the exclude $g"; return 1; } ;; esac
    # shellcheck disable=SC2254
    case "$root" in $g) MJ_FLEET_WHY="the repository root matches the exclude $g"; return 1 ;; esac
    # shellcheck disable=SC2254
    case "$phys" in $g) MJ_FLEET_WHY="the repository root matches the exclude $g"; return 1 ;; esac
  done < <(mj_ylist "$flat" excludes)
  if [ -n "$origin" ]; then
    while IFS= read -r g; do
      # shellcheck disable=SC2254
      [ -n "$g" ] && case "$origin" in $g) hit=1; break ;; esac
    done < <(mj_ylist "$flat" applies_to.remotes)
  fi
  if [ -z "$hit" ]; then
    while IFS= read -r g; do
      [ -n "$g" ] || continue
      # shellcheck disable=SC2254
      case "$root" in $g) hit=1; break ;; esac
      # shellcheck disable=SC2254
      case "$phys" in $g) hit=1; break ;; esac
    done < <(mj_ylist "$flat" applies_to.paths)
  fi
  [ -n "$hit" ] && return 0
  MJ_FLEET_WHY="neither the origin ${origin:-(none)} nor the root $(mj_fleet_tilde "$root") matches applies_to"
  return 1
}

# ---------------------------------------------------------------- the kit evidence
# kit-evidence/v1: one line per distinct licensed content, `<exact>\t<norm>[\t<source>]`, each
# hash sha256 hex (64, or a 16-hex prefix) and compared on its first 16. norm is the sha256 of
# the bytes after every maximal run of space, tab, CR and LF has become one space, nothing
# trimmed. In CI, MAJORDOMUS_KIT_EVIDENCE_<KIT> holds base64(gzip(text)).
mj_fleet_evidence_var() {
  printf '%s%s' "$MJ_FLEET_EVIDENCE_PREFIX" "$(printf '%s' "$1" | LC_ALL=C tr '[:lower:]' '[:upper:]' | LC_ALL=C sed 's/[^A-Z0-9]/_/g')"
}

# mj_fleet_evidence_materialise KIT -> the path of a 0600 file holding the decoded evidence;
# 1 when the variable is unset or empty; 2 when it holds anything else, naming the variable
# and never its value. The file lives under the command's temporary directory.
mj_fleet_evidence_materialise() {
  local kit="$1" var f rc=0
  [ -n "$kit" ] || return 1
  var="$(mj_fleet_evidence_var "$kit")"
  [ -n "${!var:-}" ] || return 1
  mj_has gzip && mj_has base64 || { mj_err "$var is set, and reading it needs base64 and gzip"; return 2; }
  f="$(umask 077; mktemp "${TMPDIR:-/tmp}/mj.kitev.XXXXXX")" || return 2
  # a subshell without xtrace: the value is a secret, and a traced shell would print it
  ( set +x; set -o pipefail
    printf '%s' "${!var}" | LC_ALL=C tr -d ' \t\r\n' | base64 -d 2>/dev/null | gzip -dc 2>/dev/null > "$f" ) || rc=2
  if [ "$rc" = 0 ]; then
    # lengths, not a {16} interval: an older mawk reads the braces as literal characters
    LC_ALL=C awk -F'\t' '
      function hex(h) { return (length(h) == 16 || length(h) == 64) && h !~ /[^0-9a-f]/ }
      /^#/ || /^[ \t\r]*$/ { next }
      !((NF == 2 || NF == 3) && hex($1) && hex($2)) { bad = NR; exit }
      { n++ }
      END { exit (bad || !n) ? 1 : 0 }' "$f" || rc=2
  fi
  if [ "$rc" != 0 ]; then
    rm -f "$f"
    mj_err "$var does not hold base64(gzip($MJ_FLEET_EVIDENCE_FORMAT)): one '<exact>\\t<norm>[\\t<source>]' line per licensed file, sha256 hex"
    return 2
  fi
  chmod 600 "$f"; printf '%s' "$f"
}

# the declared ~-relative evidence files that exist, one path per line; never absolute
mj_fleet_evidence_declared() {
  local e
  mj_fleet_policy_ready || return 0
  while IFS= read -r e; do
    case "$e" in \~/?*) [ -f "$HOME/${e#\~/}" ] && printf '%s\n' "$HOME/${e#\~/}" ;; esac
  done < <(mj_ylist "$MJ_POL_FLAT" ui.evidence)
  return 0
}

# mj_fleet_licensed FILE EVIDENCE... -> "exact|normalised<TAB>source" and 0 when FILE's bytes
# are a licensed file of the evidence; 1 otherwise. The source is printed as ~/... or by name.
mj_fleet_licensed() {
  local f="$1" exact norm; shift
  [ $# -gt 0 ] || return 1
  exact="$(mj_sha256_hex "$f" | cut -c1-16)" || return 1
  norm="$(LC_ALL=C tr '\t\r\n' '   ' < "$f" | LC_ALL=C tr -s ' ' | mj_sha256_hex | cut -c1-16)" || return 1
  LC_ALL=C awk -F'\t' -v e="$exact" -v n="$norm" -v home="${HOME:-/nonexistent}" '
    /^#/ { next }
    { k = (substr($1, 1, 16) == e) ? "exact" : (substr($2, 1, 16) == n) ? "normalised" : "" }
    k != "" { s = $3; if (index(s, home "/") == 1) s = "~/" substr(s, length(home) + 2)
              else if (s ~ /^\//) { sub(/.*\//, "", s) }
              print k "\t" (s == "" ? "-" : s); found = 1; exit }
    END { exit found ? 0 : 1 }' "$@"
}

# ---------------------------------------------------------------- the installed copy
# the stamp's version line, or empty
mj_fleet_installed_version() {
  local s; s="$(mj_fleet_stamp)"; [ -f "$s" ] || return 0
  sed -n 's/^# version: *//p' "$s" | head -n 1
}

# mj_fleet_row_ok PATH KIND -> 0 when a stamp row names a place the fleet installs into: a
# skill as <skills>/fleet-<id>/SKILL.md, a rule or a kit under rules/vendor/fleet/ in the
# shape the manifest allows. The stamp is a file in the tree, so a row is data, never a path
# to trust: swap moves what a row names and the effective set loads it, and a row naming
# src/SKILL.md would hand either of them the adopter's own files.
mj_fleet_row_ok() {
  local path="$1" kind="$2" rest skills_rel fleet_rel
  case "$path" in *..*|/*|'') return 1 ;; esac
  skills_rel="$(mj_rel "$MJ_SKILLS_DIR")"; fleet_rel="$(mj_rel "$(mj_fleet_dir)")"
  case "$kind" in
    skill)
      [ "${path#"$skills_rel/"}" != "$path" ] || return 1
      rest="${path#"$skills_rel/"}"
      [[ "$rest" =~ ^fleet-[a-z0-9][a-z0-9-]*/SKILL\.md$ ]] ;;
    rule|kit)
      [ "${path#"$fleet_rel/"}" != "$path" ] || return 1
      rest="${path#"$fleet_rel/"}"
      if [ "$kind" = rule ]; then [[ "$rest" =~ ^rules/[A-Za-z0-9._-]+\.v[0-9]+\.md$ ]]
      else [[ "$rest" =~ ^kits/[a-z0-9][a-z0-9_-]*\.yaml$ ]]; fi ;;
    *) return 1 ;;
  esac
}

# the stamp's rows that name a place the fleet installs into, `<sha>\t<path>\t<kind>\t<src>`
mj_fleet_rows() {
  local stamp="${1:-$(mj_fleet_stamp)}" sha path kind src
  [ -f "$stamp" ] || return 0
  while IFS=$'\t' read -r sha path kind src; do
    case "$sha" in '#'*|'') continue ;; esac
    mj_fleet_row_ok "$path" "$kind" && printf '%s\t%s\t%s\t%s\n' "$sha" "$path" "$kind" "$src"
  done < "$stamp"
  return 0
}

# every way the installed copy differs from its stamp, one per line; 1 when any
mj_fleet_integrity() {
  local stamp dir sha path kind src bad=0 have=" " f
  stamp="$(mj_fleet_stamp)"; dir="$(mj_fleet_dir)"
  [ -f "$stamp" ] || return 0
  [ "$(head -n 1 "$stamp")" = "# $MJ_FLEET_STAMP_FORMAT" ] \
    || { printf 'MANIFEST is not an %s stamp\n' "$MJ_FLEET_STAMP_FORMAT"; return 1; }
  while IFS=$'\t' read -r sha path kind src; do
    case "$sha" in '#'*|'') continue ;; esac
    if ! mj_fleet_row_ok "$path" "$kind"; then
      printf 'MANIFEST names a path outside the fleet (%s, %s)\n' "$path" "${kind:-no kind}"; bad=1; continue
    fi
    have="$have$path "
    if [ ! -f "$MJ_ROOT/$path" ]; then printf '%s is stamped but absent\n' "$path"; bad=1; continue; fi
    [ "$(mj_sha256_hex "$MJ_ROOT/$path")" = "$sha" ] \
      || { printf '%s differs from its stamp (hand-edited?)\n' "$path"; bad=1; }
  done < "$stamp"
  while IFS= read -r f; do
    [ "$f" = "$stamp" ] && continue
    case "$have" in *" $(mj_rel "$f") "*) ;; *) printf '%s is present but not in the stamp\n' "$(mj_rel "$f")"; bad=1 ;; esac
  done < <(find "$dir" -type f 2>/dev/null | LC_ALL=C sort)
  return $bad
}

# ---------------------------------------------------------------- install
# mj_fleet_install CONTEXT [--force]
#   CONTEXT is the command asking (init, migrate, rules, update); only `rules` says that
#   there is no pack. Exit 0 installed, current or nothing to do; 10 the pack is malformed;
#   15 refused (an entry a public tree may not carry, or a hand-edited installed copy).
mj_fleet_install() {
  local ctx="$1" force=0 src tmp vis probs
  [ "${2:-}" = --force ] && force=1
  [ -n "$MJ_RULES_DIR" ] || return 0
  src="$(mj_fleet_source)" || return 0
  if ! mj_fleet_has_pack "$src"; then
    [ "$ctx" = rules ] && printf 'fleet: no pack at %s; nothing to install\n' "$(mj_fleet_tilde "$src")"
    return 0
  fi
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.fleet.XXXXXX")"
  if ! mj_fleet_load "$src" "$tmp"; then
    printf 'fleet: the pack at %s is malformed: %s; nothing installed\n' "$(mj_fleet_tilde "$src")" "$MJ_FLEET_ERROR" >&2
    rm -rf "$tmp"; return "$MJ_EX_CONTRACT"
  fi
  if ! mj_fleet_applies "$tmp/flat"; then
    printf 'fleet: pack v%s does not apply here (%s); nothing installed\n' "$MJ_FLEET_VERSION" "$MJ_FLEET_WHY"
    rm -rf "$tmp"; return 0
  fi
  if [ "$force" != 1 ]; then
    probs="$(mj_fleet_integrity || true)"
    if [ -n "$probs" ]; then
      printf 'fleet: the installed copy was hand-edited (%s); review it, then: majordomus rules vendor update --force\n' "$(printf '%s' "$probs" | head -n 1)" >&2
      rm -rf "$tmp"; return "$MJ_EX_REFUSED"
    fi
  fi
  vis="$(mj_fleet_visibility)"
  # shellcheck source=skills.sh
  . "$MJ_LIB_DIR/skills.sh"

  # the evidence a public tree's files are compared with: the declared files, and the CI
  # secret of the policy's kit and of every kit the pack delivers to this tree. A kit in play
  # with no evidence at all is not judged, and an unjudged pack is not installed: the gate's
  # not_judged is a failure, and the install is not the place it becomes a pass.
  local ev=() e k kits rc vars=""
  if [ "$vis" = public ]; then
    while IFS= read -r e; do [ -n "$e" ] && ev+=("$e"); done < <(mj_fleet_evidence_declared)
    kits="$(mj_fleet_kit) $(awk -F'\t' '$2 == "kit" && $3 != "private" { p = $1; sub(/^kits\//, "", p); sub(/\.yaml$/, "", p); print p }' "$tmp/entries")"
    for k in $kits; do
      case " $vars " in *" $(mj_fleet_evidence_var "$k") "*) continue ;; esac
      vars="${vars:+$vars }$(mj_fleet_evidence_var "$k")"
      # decoded into the staging directory, so that every way out of here removes it
      rc=0; e="$(TMPDIR="$tmp" mj_fleet_evidence_materialise "$k")" || rc=$?
      case "$rc" in 0) ev+=("$e") ;; 1) ;; *) rm -rf "$tmp"; return "$MJ_EX_CONTRACT" ;; esac
    done
    if [ -n "$vars" ] && [ ${#ev[@]} -eq 0 ]; then
      printf 'fleet: pack v%s refused for this public tree; nothing installed:\n' "$MJ_FLEET_VERSION" >&2
      printf '  refused: not judged: a kit is in play (%s) and no licence evidence is here; set it, or list a ~/ evidence file under ui.evidence\n' "$vars" >&2
      rm -rf "$tmp"; return "$MJ_EX_REFUSED"
    fi
  fi

  # stage only the listed files, judging each before anything is written
  local path kind evis rel staged id skills_rel fleet_rel refused="" skipped=0 n=0 hit line scan
  skills_rel="$(mj_rel "$MJ_SKILLS_DIR")"; fleet_rel="$(mj_rel "$(mj_fleet_dir)")"
  mkdir -p "$tmp/scan"
  while IFS=$'\t' read -r path kind evis; do
    if [ "$evis" = private ] && [ "$vis" = public ]; then skipped=$((skipped + 1)); continue; fi
    if [ ! -f "$src/$path" ] || [ -L "$src/$path" ]; then refused="$refused$path: listed but not a regular file of the pack"$'\n'; continue; fi
    case "$kind" in
      skill) id="${path#skills/}"; id="${id%/SKILL.md}"; rel="$skills_rel/$MJ_SKILLS_FLEET_PREFIX$id/SKILL.md" ;;
      *) rel="$fleet_rel/$path" ;;
    esac
    staged="$tmp/new/$rel"; mkdir -p "$(dirname "$staged")"; cp "$src/$path" "$staged"
    if [ "$vis" = public ]; then
      line="$(LC_ALL=C grep -nE "$MJ_FLEET_HOME_PATH_ERE" "$staged" | head -n 1 | cut -d: -f1)"
      [ -z "$line" ] || { refused="$refused$path: carries a home or absolute path (line $line); a public tree never names the owner's machine"$'\n'; continue; }
      if [ ${#ev[@]} -gt 0 ] && hit="$(mj_fleet_licensed "$staged" "${ev[@]}")"; then
        refused="$refused$path: is a licensed file (${hit%%$'\t'*} copy of ${hit#*$'\t'}); a public tree never carries it"$'\n'; continue
      fi
    fi
    case "$kind" in
      skill)
        scan="$(mj_skills_check_file "$staged" "$MJ_SKILLS_FLEET_PREFIX$id" 2>&1)" \
          || { refused="$refused$path: fails skills check ($(printf '%s' "$scan" | head -n 1))"$'\n'; continue; }
        if [ -e "$MJ_ROOT/$rel" ] && ! grep -qF "$(printf '\t%s\t' "$rel")" "$(mj_fleet_stamp)" 2>/dev/null; then
          refused="$refused$path: $rel exists and is not the fleet's; it is never overwritten"$'\n'; continue
        fi ;;
      rule)
        if ! mj_rule_scan "$staged" "vendor:$MJ_FLEET_NS" "$n" "$tmp/scan"; then
          refused="$refused$path: $MJ_RULE_REASON"$'\n'; continue; fi
        case "$MJ_RULE_KEY" in "$MJ_RULES_VENDOR_NS".*|"$MJ_RULES_PROJECT_NS".*)
          refused="$refused$path: claims the ${MJ_RULE_KEY%%.*} namespace (${MJ_RULE_KEY%@*}), which is not the fleet's"$'\n'; continue ;; esac ;;
    esac
    printf '%s\t%s\t%s\t%s\n' "$(mj_sha256_hex "$staged")" "$rel" "$kind" "$path" >> "$tmp/stamp.rows"
    n=$((n + 1))
  done < "$tmp/entries"
  if [ -n "$refused" ]; then
    printf 'fleet: pack v%s refused for this %s tree; nothing installed:\n' "$MJ_FLEET_VERSION" "$vis" >&2
    printf '%s' "$refused" | sed 's/^/  refused /' >&2
    rm -rf "$tmp"; return "$MJ_EX_REFUSED"
  fi
  mkdir -p "$tmp/new/$fleet_rel"
  { printf '# %s\n# version: %s\n' "$MJ_FLEET_STAMP_FORMAT" "$MJ_FLEET_VERSION"
    printf '# written by majordomus from the fleet pack; never edit: majordomus rules vendor update\n'
    [ -f "$tmp/stamp.rows" ] && cat "$tmp/stamp.rows"
  } > "$tmp/new/$fleet_rel/MANIFEST"

  # the same pack, the same bytes: nothing is written, so a refresh is byte-identical
  if [ -f "$(mj_fleet_stamp)" ] && cmp -s "$tmp/new/$fleet_rel/MANIFEST" "$(mj_fleet_stamp)" && [ "$force" != 1 ]; then
    [ "$ctx" = rules ] && printf 'fleet: pack v%s is current in %s\n' "$MJ_FLEET_VERSION" "$fleet_rel"
    rm -rf "$tmp"; return 0
  fi
  mj_fleet_swap "$tmp" "$fleet_rel" || { rm -rf "$tmp"; mj_die "$MJ_EX_INTERNAL" "could not install the fleet pack into $fleet_rel"; }
  printf 'fleet: installed pack v%s into %s (%s file(s)%s)\n' "$MJ_FLEET_VERSION" "$fleet_rel" "$n" \
    "$([ "$skipped" = 0 ] || printf '; %s private entr%s skipped for this public tree' "$skipped" "$([ "$skipped" = 1 ] && printf y || printf ies)")"
  rm -rf "$tmp"
  return 0
}
# Swap the staged copy in: the old fleet directory and the old stamped skills move aside
# under the staging directory first, so a failure puts them back and a success removes only
# what the old stamp listed. An adopter's own skill is never in that list.
mj_fleet_swap() {
  local tmp="$1" fleet_rel="$2" sha path kind src d
  mkdir -p "$tmp/old"
  # only rows that name a fleet skill's directory: a stamp row is never trusted as a path
  while IFS=$'\t' read -r sha path kind src; do
    [ "$kind" = skill ] || continue
    d="$(dirname "$path")"
    [ -d "$MJ_ROOT/$d" ] && { mkdir -p "$tmp/old/$(dirname "$d")"; mv "$MJ_ROOT/$d" "$tmp/old/$d"; }
  done < <(mj_fleet_rows "$MJ_ROOT/$fleet_rel/MANIFEST")
  if [ -d "$MJ_ROOT/$fleet_rel" ]; then mkdir -p "$tmp/old/$(dirname "$fleet_rel")"; mv "$MJ_ROOT/$fleet_rel" "$tmp/old/$fleet_rel"; fi
  local ok=1
  mkdir -p "$(dirname "$MJ_ROOT/$fleet_rel")" && mv "$tmp/new/$fleet_rel" "$MJ_ROOT/$fleet_rel" || ok=0
  if [ "$ok" = 1 ]; then
    while IFS=$'\t' read -r sha path kind src; do
      [ "$kind" = skill ] || continue
      d="$(dirname "$path")"
      mkdir -p "$(dirname "$MJ_ROOT/$d")" && mv "$tmp/new/$d" "$MJ_ROOT/$d" || { ok=0; break; }
    done < <(mj_fleet_rows "$MJ_ROOT/$fleet_rel/MANIFEST")
  fi
  [ "$ok" = 1 ] && return 0
  # put back what was there; the partial new copy goes with the staging directory
  [ -d "$MJ_ROOT/$fleet_rel" ] && mv "$MJ_ROOT/$fleet_rel" "$tmp/partial"
  [ -d "$tmp/old/$fleet_rel" ] && mv "$tmp/old/$fleet_rel" "$MJ_ROOT/$fleet_rel"
  while IFS=$'\t' read -r sha path kind src; do
    d="$(dirname "$path")"
    [ "$kind" = skill ] && [ -d "$tmp/old/$d" ] || continue
    [ -d "$MJ_ROOT/$d" ] && mv "$MJ_ROOT/$d" "$tmp/partial-$kind-${d##*/}"
    mv "$tmp/old/$d" "$MJ_ROOT/$d"
  done < <(mj_fleet_rows "$MJ_ROOT/$fleet_rel/MANIFEST")
  return 1
}

# ---------------------------------------------------------------- doctor and watch
# The installed copy against its stamp (a hand edit is the doctrine's failure), and against
# the pack on this machine when there is one (a lagging copy is a warning in doctor and
# drift in watch). Reported under the rule-package doctrine, which owns rules/vendor/.
mj_fleet_report() {
  local dir stamp probs inst src tmp p
  dir="$(mj_fleet_dir)"; stamp="$(mj_fleet_stamp)"
  if [ -f "$stamp" ]; then
    probs="$(mj_fleet_integrity || true)"
    if [ -n "$probs" ]; then
      # a here-string, not a pipe: a finding raised in a pipeline's subshell is never counted
      while IFS= read -r p; do
        mj_doctrine_fail rules "$(mj_rel "$dir")" "$p" "majordomus rules vendor update --force"
      done <<< "$probs"
    else mj_doctrine_ok rules "$(mj_rel "$dir")" "every file matches its stamp (fleet pack v$(mj_fleet_installed_version))"; fi
  fi
  src="$(mj_fleet_source)" || return 0
  mj_fleet_has_pack "$src" || return 0
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.fleetr.XXXXXX")"
  if ! mj_fleet_load "$src" "$tmp"; then
    mj_warn rules "fleet pack" "the pack at $(mj_fleet_tilde "$src") is malformed: $MJ_FLEET_ERROR" "majordomus rules vendor update"
  elif mj_fleet_applies "$tmp/flat"; then
    inst="$(mj_fleet_installed_version)"
    if [ -z "$inst" ] || [ "$inst" -lt "$MJ_FLEET_VERSION" ]; then
      mj_fleet_lag "$(mj_rel "$dir")" "the fleet pack is v$MJ_FLEET_VERSION and this repository holds ${inst:+v}${inst:-none of it}" "majordomus rules vendor update"
    fi
  fi
  rm -rf "$tmp"
  return 0
}
mj_fleet_lag() {
  if [ "${MJ_DOCTRINE_CMD:-}" = watch ]; then mj_drift rules "$@"; else mj_warn rules "$@"; fi
}

# ---------------------------------------------------------------- the licence gate
# Whether the tree carries a licensed file of its declared kit, judged by the executable's
# design.licence over every tracked file and the publish roots (`majordomus-cli design
# licence`). The CI secret is decoded for it when it is set. Fail closed: a public tree that
# declares a kit and has a UI, but that nothing judged — no evidence, no executable, an
# executable without the capability — is a failure, never a note.
mj_fleet_report_licence() {
  # shellcheck source=rust_bin.sh
  . "$MJ_LIB_DIR/rust_bin.sh"
  local kit vis var evf="" rc bin out verdict why share n=0 path source kind
  kit="$(mj_fleet_kit)"
  [ -n "$kit" ] || return 0
  vis="$(mj_fleet_visibility)"
  var="$(mj_fleet_evidence_var "$kit")"
  rc=0; evf="$(mj_fleet_evidence_materialise "$kit" 2>/dev/null)" || rc=$?
  if [ "$rc" = 2 ]; then
    mj_fail licence "$var" "does not hold base64(gzip($MJ_FLEET_EVIDENCE_FORMAT)); the kit $kit is not judged" "majordomus doctor"
    return 0
  fi
  bin="$(mj_rust_bin "$MJ_HOME")"
  out=""
  if [ -x "$bin" ]; then
    # the tool's own share, not the adopter's: an adopter carries no kinds.yaml of its own.
    # exit 10 (refused) and 12 (not judged) are verdicts; the document is read either way
    share="$(mj_rust_share "$MJ_HOME")"
    out="$( ( [ -z "$share" ] || export MAJORDOMUS_SHARE="$share"
              "$bin" design licence --repo "$MJ_ROOT" ${evf:+--evidence "$evf"} --json ) 2>/dev/null )" || true
  fi
  [ -z "$evf" ] || rm -f "$evf"
  verdict="$(printf '%s' "$out" | jq -r '.verdict // .state // empty' 2>/dev/null || true)"
  why="$(printf '%s' "$out" | jq -r '.why // .reason // empty' 2>/dev/null || true)"
  case "$verdict" in
    active)   mj_ok licence "$kit" "active: the licensed set is held in this private tree${why:+ ($why)}" ;;
    inactive) mj_info licence "$kit" "inactive${why:+: $why}" ;;
    refused)
      # in this shell, not a pipeline's subshell, so that each FAIL is counted in MJ_FAILS
      while IFS=$'\t' read -r path source kind; do
        [ -n "$path" ] || continue
        n=$((n + 1))
        mj_fail licence "$path" "is a licensed file of $kit ($kind copy of $source); a $vis tree never carries it" "majordomus-cli design licence"
      done < <(printf '%s' "$out" | jq -r '.matches[]? | [.path, (.source // "-"), .kind] | @tsv' 2>/dev/null)
      [ "$n" -gt 0 ] || mj_fail licence "$kit" "refused, and the verdict names no licensed file${why:+ ($why)}" "majordomus-cli design licence" ;;
    not_judged)
      if [ "$vis" = public ]; then mj_fail licence "$kit" "not judged${why:+: $why}; a public tree that declares a kit fails closed (set $var in CI)" "majordomus-cli design licence"
      else mj_info licence "$kit" "not judged${why:+: $why}"; fi ;;
    *)
      if [ -z "$(mj_git ls-files -- '*.html' '*.htm' '*.heex' '*.jsx' '*.tsx' '*.vue' '*.svelte' 2>/dev/null | head -n 1)" ]; then
        mj_info licence "$kit" "no UI file is tracked; the licence gate is skipped"
      elif [ "$vis" = public ]; then
        mj_fail licence "$kit" "not judged: the executable answers no design licence; a public tree that declares a kit fails closed" "majordomus-cli design licence"
      else mj_info licence "$kit" "not judged: the executable answers no design licence"; fi ;;
  esac
  return 0
}
