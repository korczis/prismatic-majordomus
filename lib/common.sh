#!/usr/bin/env bash
# shellcheck disable=SC2034  # the MJ_RES_* resolution outputs are read by the commands that
# source this file: handover, checkpoint, context, prompt, start, doctor and watch.
# common.sh — shared helpers for majordomus. Sourced, never executed.
# Targets bash 3.2 and BSD userland: no associative arrays, no mapfile, no GNU-only flags.

MJ_EX_OK=0; MJ_EX_USAGE=2; MJ_EX_CONTRACT=10; MJ_EX_DRIFT=11
MJ_EX_MISSING=12; MJ_EX_INTERNAL=13; MJ_EX_REFUSED=15
export MJ_EX_OK MJ_EX_USAGE MJ_EX_CONTRACT MJ_EX_DRIFT MJ_EX_MISSING MJ_EX_INTERNAL MJ_EX_REFUSED

MJ_REPO=""; MJ_JSON=0; MJ_ARGS=()
MJ_FINDINGS=0; MJ_FAILS=0

# ---------------------------------------------------------------- output
mj_err()  { printf 'majordomus: %s\n' "$*" >&2; }
mj_die()  { local code="$1"; shift; mj_err "$*"; exit "$code"; }

# mj_finding LEVEL category subject message [reproduce]
mj_finding() {
  local level="$1" cat="$2" subj="$3" msg="$4" rep="${5:-}"
  MJ_FINDINGS=$((MJ_FINDINGS + 1))
  case "$level" in FAIL|DRIFT) MJ_FAILS=$((MJ_FAILS + 1)) ;; esac
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"level":"%s","category":"%s","subject":"%s","message":"%s","reproduce":"%s"}\n' \
      "$level" "$(mj_json_esc "$cat")" "$(mj_json_esc "$subj")" "$(mj_json_esc "$msg")" "$(mj_json_esc "$rep")"
  else
    if [ -n "$rep" ]; then
      printf '%-4s %-11s %s — %s  [reproduce: %s]\n' "$level" "$cat" "$subj" "$msg" "$rep"
    else
      printf '%-4s %-11s %s — %s\n' "$level" "$cat" "$subj" "$msg"
    fi
  fi
}
mj_ok()    { mj_finding OK    "$@"; }
mj_fail()  { mj_finding FAIL  "$@"; }
mj_warn()  { mj_finding WARN  "$@"; }
mj_info()  { mj_finding INFO  "$@"; }
mj_drift() { mj_finding DRIFT "$@"; }

mj_json_esc() { printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' | tr -d '\n'; }

# ---------------------------------------------------------------- options
mj_parse_global_opts() {
  MJ_ARGS=()
  while [ $# -gt 0 ]; do
    case "$1" in
      --repo) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--repo needs a path"; MJ_REPO="$2"; shift 2 ;;
      --repo=*) MJ_REPO="${1#--repo=}"; shift ;;
      --json) MJ_JSON=1; shift ;;
      *) MJ_ARGS+=("$1"); shift ;;
    esac
  done
  export MJ_JSON
}

# ---------------------------------------------------------------- repo & git
mj_repo_root() {
  local root
  if [ -n "$MJ_REPO" ]; then
    root="$(cd "$MJ_REPO" 2>/dev/null && git rev-parse --show-toplevel 2>/dev/null)" || return 1
  else
    root="$(git rev-parse --show-toplevel 2>/dev/null)" || return 1
  fi
  printf '%s' "$root"
}
mj_require_repo() {
  MJ_ROOT="$(mj_repo_root)" || mj_die "$MJ_EX_USAGE" "not inside a git repository (use --repo <path>)"
  export MJ_ROOT
  # hooks inherited from a parent process must never redirect our git calls
  unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX
  mj_resolve_layout
}
mj_require_installed() {
  mj_require_repo
  case "$MJ_LAYOUT" in
    ai) [ -f "$MJ_POLICY_FILE" ] || mj_die "$MJ_EX_MISSING" "no $(mj_rel "$MJ_POLICY_FILE") in $MJ_ROOT; the manifest names it (run: majordomus init)" ;;
    legacy) mj_die "$MJ_EX_MISSING" "project data lives under .majordomus/ (the pre-.ai layout); run: majordomus migrate" ;;
    *) mj_die "$MJ_EX_MISSING" "no .ai/manifest.yaml in $MJ_ROOT (run: majordomus init)" ;;
  esac
}

# ---------------------------------------------------------------- paths
# Two roots, never one variable. MJ_HOME is the tool distribution — bin, lib, share — and
# is read-only: it may be a PATH install, a checkout under ~/tools, a package, or an
# optional .majordomus/ submodule inside the managed repository, and every form behaves
# the same because nothing below writes into it. MJ_ROOT is the managed repository, whose
# AI layer lives under .ai/: repo/ is tracked canonical context, local/ is checkout-local
# and ignored. Every path the commands read or write is resolved here and nowhere else.
# a script that sources this file for its helpers alone (the test suite, the site generator)
# need not have set MJ_BIN_DIR; the distribution is the directory this file lives under.
# Never taken from the environment: a majordomus started by another majordomus (a verify
# command, a hook) must read its own distribution, not the one that started it.
if [ -n "${MJ_BIN_DIR:-}" ]; then MJ_HOME="$(cd "$MJ_BIN_DIR/.." && pwd)"
else MJ_HOME="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"; fi
MJ_SHARE_DIR="$MJ_HOME/share"
MJ_SKELETON_DIR="$MJ_SHARE_DIR/skeleton"
MJ_ALLOW_DIR="$MJ_SHARE_DIR/allow"
MJ_STD_RULES_DIR="$MJ_SHARE_DIR/standard/majordomus"
MJ_PROVIDERS_DEFAULT_DIR="$MJ_SHARE_DIR/providers"
export MJ_SHARE_DIR MJ_SKELETON_DIR MJ_ALLOW_DIR MJ_STD_RULES_DIR MJ_PROVIDERS_DEFAULT_DIR

MJ_LAYOUT=""; MJ_AI_DIR=""; MJ_AI_MANIFEST=""; MJ_AI_REPO_DIR=""; MJ_AI_LOCAL_DIR=""
MJ_STATE_DIR=""; MJ_POLICY_FILE=""; MJ_PROFILES_DIR=""; MJ_PROMPTS_DIR=""; MJ_PROJECT_DIR=""
MJ_RULES_DIR=""; MJ_KNOWLEDGE_DIR=""; MJ_ADRS_DIR=""; MJ_SKILLS_DIR=""; MJ_WORKFLOWS_DIR=""
MJ_PROVIDERS_DIR=""; MJ_TEMPLATES_DIR=""; MJ_CACHE_DIR=""

# a repository path, relative to the repository root, for messages and records
mj_rel() { printf '%s' "${1#"$MJ_ROOT/"}"; }
# is repository-relative path $1 inside the AI layer (the tool's own files)?
mj_is_ai_path() { case "$1" in "$(mj_rel "$MJ_AI_DIR")"|"$(mj_rel "$MJ_AI_DIR")"/*) return 0 ;; esac; return 1; }

# Resolve the repository layout. Never fails: a repository with no AI layer resolves to
# the paths init would create, so init and doctor can name them.
#   ai      .ai/manifest.yaml exists; every section path is read from it
#   legacy  the pre-.ai layout, .majordomus/policy.yaml; read-only compatibility during
#           the migration, removed once `majordomus migrate` exists
mj_resolve_layout() {
  MJ_AI_DIR="$MJ_ROOT/.ai"; MJ_AI_MANIFEST="$MJ_AI_DIR/manifest.yaml"
  if [ -f "$MJ_AI_MANIFEST" ]; then
    MJ_LAYOUT=ai
    mj_load_manifest || mj_die "$MJ_EX_CONTRACT" "$(mj_rel "$MJ_AI_MANIFEST") is not a manifest this version reads: $MJ_MANIFEST_ERROR"
    MJ_AI_REPO_DIR="$MJ_AI_DIR/$(mj_man repo.path)"
    MJ_AI_LOCAL_DIR="$MJ_AI_DIR/$(mj_man local.path)"
    MJ_POLICY_FILE="$MJ_AI_DIR/$(mj_man sections.policy)"
    MJ_PROFILES_DIR="$MJ_AI_DIR/$(mj_man sections.profiles)"
    MJ_RULES_DIR="$MJ_AI_DIR/$(mj_man sections.rules)"
    MJ_PROMPTS_DIR="$MJ_AI_DIR/$(mj_man sections.prompts)"
    MJ_SKILLS_DIR="$MJ_AI_DIR/$(mj_man sections.skills)"
    MJ_WORKFLOWS_DIR="$MJ_AI_DIR/$(mj_man sections.workflows)"
    MJ_KNOWLEDGE_DIR="$MJ_AI_DIR/$(mj_man sections.knowledge)"
    MJ_ADRS_DIR="$MJ_AI_DIR/$(mj_man sections.adrs)"
    MJ_PROJECT_DIR="$MJ_AI_DIR/$(mj_man sections.project)"
    MJ_PROVIDERS_DIR="$MJ_AI_REPO_DIR/providers"
    MJ_TEMPLATES_DIR="$MJ_AI_REPO_DIR/templates"
    MJ_STATE_DIR="$MJ_AI_LOCAL_DIR/state"
    MJ_CACHE_DIR="$MJ_AI_LOCAL_DIR/cache"
  else
    # Project data under .majordomus/ is the pre-.ai layout, which nothing reads any more:
    # the name is detected so that mj_require_installed can refuse with the migration
    # named and `migrate` can find its source, and the paths are the .ai ones either way.
    # A .majordomus/ holding a tool distribution (bin/majordomus) is an installation,
    # never project data.
    MJ_LAYOUT=""
    [ -d "$MJ_ROOT/.majordomus" ] && [ ! -f "$MJ_ROOT/.majordomus/bin/majordomus" ] && MJ_LAYOUT=legacy
    MJ_AI_REPO_DIR="$MJ_AI_DIR/repo"; MJ_AI_LOCAL_DIR="$MJ_AI_DIR/local"
    MJ_POLICY_FILE="$MJ_AI_REPO_DIR/policy.yaml"; MJ_PROFILES_DIR="$MJ_AI_REPO_DIR/profiles"
    MJ_RULES_DIR="$MJ_AI_REPO_DIR/rules"; MJ_PROMPTS_DIR="$MJ_AI_REPO_DIR/prompts"
    MJ_SKILLS_DIR="$MJ_AI_REPO_DIR/skills"; MJ_WORKFLOWS_DIR="$MJ_AI_REPO_DIR/workflows"
    MJ_KNOWLEDGE_DIR="$MJ_AI_REPO_DIR/knowledge"; MJ_ADRS_DIR="$MJ_AI_REPO_DIR/adrs"
    MJ_PROJECT_DIR="$MJ_AI_REPO_DIR/project"; MJ_PROVIDERS_DIR="$MJ_AI_REPO_DIR/providers"
    MJ_TEMPLATES_DIR="$MJ_AI_REPO_DIR/templates"; MJ_STATE_DIR="$MJ_AI_LOCAL_DIR/state"
    MJ_CACHE_DIR="$MJ_AI_LOCAL_DIR/cache"
  fi
  export MJ_LAYOUT MJ_AI_DIR MJ_AI_MANIFEST MJ_AI_REPO_DIR MJ_AI_LOCAL_DIR MJ_STATE_DIR
  export MJ_POLICY_FILE MJ_PROFILES_DIR MJ_PROMPTS_DIR MJ_PROJECT_DIR MJ_RULES_DIR MJ_KNOWLEDGE_DIR
  export MJ_ADRS_DIR MJ_SKILLS_DIR MJ_WORKFLOWS_DIR MJ_PROVIDERS_DIR MJ_TEMPLATES_DIR MJ_CACHE_DIR
  return 0
}

# The manifest is the section registry of the AI layer: which format it is, where the
# tracked and local halves are, and where each section lives. Nothing walks .ai/ to find
# out. A manifest newer than this executable, or one with a key nothing reads, is refused
# with the reason rather than read partially.
MJ_MAN_FLAT=""; MJ_MANIFEST_ERROR=""
MJ_MANIFEST_SCHEMA="ai-repository/v1"
mj_load_manifest() {
  local k
  MJ_MANIFEST_ERROR=""
  [ -n "$MJ_MAN_FLAT" ] && [ -f "$MJ_MAN_FLAT" ] && return 0
  MJ_MAN_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.man.XXXXXX")"
  if ! mj_yaml_flatten "$MJ_AI_MANIFEST" > "$MJ_MAN_FLAT" 2>/dev/null; then
    MJ_MANIFEST_ERROR="does not parse"; return 1
  fi
  if [ "$(mj_yget "$MJ_MAN_FLAT" schema)" != "$MJ_MANIFEST_SCHEMA" ]; then
    MJ_MANIFEST_ERROR="schema '$(mj_yget "$MJ_MAN_FLAT" schema)' is not $MJ_MANIFEST_SCHEMA (this executable reads $MJ_MANIFEST_SCHEMA)"; return 1
  fi
  k="$(mj_yaml_unknown_keys "$MJ_MAN_FLAT" "$MJ_ALLOW_DIR/manifest.txt" || true)"
  [ -z "$k" ] || { MJ_MANIFEST_ERROR="unknown key(s): $(printf '%s' "$k" | tr '\n' ' ')"; return 1; }
  for k in repo.path local.path sections.policy sections.profiles sections.rules sections.prompts \
           sections.skills sections.workflows sections.knowledge sections.adrs sections.project; do
    [ -n "$(mj_yget "$MJ_MAN_FLAT" "$k")" ] || { MJ_MANIFEST_ERROR="missing key $k"; return 1; }
  done
  return 0
}
mj_man() { [ -n "${MJ_MAN_FLAT:-}" ] || return 0; mj_yget "$MJ_MAN_FLAT" "$1"; }

# The provider template for a projection: the repository's own override under its AI
# layer when it has one, otherwise the adapter the distribution ships.
mj_provider_template() {
  if [ -f "$MJ_PROVIDERS_DIR/$1.tmpl" ]; then printf '%s' "$MJ_PROVIDERS_DIR/$1.tmpl"
  elif [ -f "$MJ_PROVIDERS_DEFAULT_DIR/$1.tmpl" ]; then printf '%s' "$MJ_PROVIDERS_DEFAULT_DIR/$1.tmpl"
  else return 1; fi
}

# The layout as data — one line per path variable, name then repository-relative path — for
# whatever needs to name the files the commands touch without repeating this file.
mj_layout_table() {
  local v
  for v in MJ_AI_DIR MJ_AI_REPO_DIR MJ_AI_LOCAL_DIR MJ_STATE_DIR MJ_POLICY_FILE MJ_PROFILES_DIR \
           MJ_PROMPTS_DIR MJ_PROJECT_DIR MJ_RULES_DIR MJ_KNOWLEDGE_DIR MJ_ADRS_DIR MJ_SKILLS_DIR \
           MJ_WORKFLOWS_DIR MJ_PROVIDERS_DIR MJ_TEMPLATES_DIR MJ_CACHE_DIR; do
    p="${!v}"
    [ -n "$p" ] || continue
    printf '%s\t%s\n' "$v" "$(mj_rel "$p")"
  done
}

mj_git() { git -C "$MJ_ROOT" "$@"; }
mj_git_repo_id() { mj_git rev-parse --git-common-dir 2>/dev/null | { read -r d; case "$d" in /*) printf '%s' "$d" ;; *) printf '%s/%s' "$MJ_ROOT" "$d" ;; esac; }; }
mj_git_branch()  { mj_git symbolic-ref --short HEAD 2>/dev/null || printf 'DETACHED'; }
mj_git_head()    { mj_git rev-parse HEAD 2>/dev/null || printf 'NONE'; }
mj_git_dirty()   { [ -z "$(mj_git status --porcelain=v1 2>/dev/null)" ] && printf 'clean' || printf 'dirty'; }
mj_branch_key()  { mj_git_branch | sed 's/[^A-Za-z0-9._-]/-/g'; }

# divergence label of a recorded head vs current HEAD:
#   exact | advanced | diverged | different_context | unknown
mj_git_label() {
  local rec_head="$1" rec_branch="$2" cur_head cur_branch
  cur_head="$(mj_git_head)"; cur_branch="$(mj_git_branch)"
  if [ "$rec_branch" != "$cur_branch" ]; then printf 'different_context'; return; fi
  if [ "$rec_head" = "$cur_head" ]; then printf 'exact'; return; fi
  if mj_git merge-base --is-ancestor "$rec_head" "$cur_head" 2>/dev/null; then printf 'advanced'; return; fi
  printf 'diverged'
}

# files touched since a base commit: uncommitted + committed
mj_git_touched() {
  local base="$1"
  { mj_git status --porcelain=v1 2>/dev/null | cut -c4- | sed 's/^.* -> //'
    [ -n "$base" ] && [ "$base" != "NONE" ] && mj_git diff --name-only "$base" HEAD 2>/dev/null
  } | sort -u | sed '/^$/d'
}

# ---------------------------------------------------------------- misc
mj_now()    { date -u +%Y-%m-%dT%H:%M:%SZ; }
mj_now_compact() { date -u +%Y%m%dT%H%M%SZ; }
mj_rand16() { od -An -N8 -tx1 /dev/urandom | tr -d ' \n'; }
mj_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
  else mj_die "$MJ_EX_MISSING" "need sha256sum or shasum"; fi
}
# NR counts a final line that carries no newline; wc -l does not, and a projection
# written without a trailing newline would be measured one line under its budget.
mj_lines() { awk 'END{ print NR + 0 }' "$1"; }
mj_has()   { command -v "$1" >/dev/null 2>&1; }

# policy + profiles, in that order, for the policy hash. An empty profiles/ is a policy
# error that doctor reports, not a cat(1) failure that aborts whoever asked for the hash.
mj_policy_cat() {
  local pf
  cat "$MJ_POLICY_FILE"
  for pf in "$MJ_PROFILES_DIR"/*.yaml; do
    if [ -f "$pf" ]; then cat "$pf"; fi
  done
  return 0
}

# normalise a repo-relative path: strip ./ and trailing /, collapse //, refuse escapes
mj_norm_path() {
  local p="$1"
  p="$(printf '%s' "$p" | sed -e 's#^\./##' -e 's#//*#/#g' -e 's#/$##')"
  case "$p" in ""|/*|../*|*/../*|*/..|..) return 1 ;; esac
  printf '%s' "$p"
}
# does path a contain path b (or equal)? Both sides are normalised first: state files can be
# hand-edited, so a trailing slash or ./ in a scope entry must not silently exclude everything.
mj_path_contains() {
  local a b; a="$(mj_norm_path "$1" 2>/dev/null || printf '%s' "$1")"; b="$(mj_norm_path "$2" 2>/dev/null || printf '%s' "$2")"
  case "$b" in "$a"|"$a"/*) return 0 ;; esac; return 1
}

# "15m" / "2h" / "90s" -> seconds
mj_duration_secs() {
  local v="$1" n u
  n="${v%[smh]}"; u="${v#"$n"}"
  case "$n" in ''|*[!0-9]*) return 1 ;; esac
  case "$u" in s|'') printf '%s' "$n" ;; m) printf '%s' $((n*60)) ;; h) printf '%s' $((n*3600)) ;; *) return 1 ;; esac
}
# ISO timestamp -> epoch seconds (BSD and GNU date)
mj_epoch() {
  local ts="$1"
  if date -u -j -f '%Y-%m-%dT%H:%M:%SZ' "$ts" +%s 2>/dev/null; then return; fi
  date -u -d "$ts" +%s 2>/dev/null
}

# ---------------------------------------------------------------- YAML subset
# Flattens a restricted YAML subset into "dotted.path=value" lines.
# Supported: `key: value`, nested maps by 2-space indent, block lists (`- item`),
# lists of maps (`- key: value` + indented keys), inline lists `[a, b]`, quotes,
# comments. Tabs, anchors, multi-line scalars and flow maps are rejected.
mj_yaml_flatten() {
  awk '
  function trim(s){ sub(/^[ \t]+/,"",s); sub(/[ \t]+$/,"",s); return s }
  function unq(v){
    if (v ~ /^".*"$/ || v ~ /^\047.*\047$/) return substr(v,2,length(v)-2)
    sub(/[ \t]+#.*$/,"",v); return trim(v)
  }
  function join(a,b){ return (a=="" ? b : a "." b) }
  function emit_val(p,v,  n,i,parts){
    if (v ~ /^\[.*\]$/) {
      v=substr(v,2,length(v)-2); v=trim(v)
      if (v=="") { print p "=[]"; return }
      n=split(v,parts,/[ \t]*,[ \t]*/)
      for(i=1;i<=n;i++) print p "." (i-1) "=" unq(parts[i])
      return
    }
    print p "=" unq(v)
  }
  function clear_from(i,  k){ for(k in ctx) if(k+0>i) delete ctx[k]; for(k in pend) if(k+0>=i) delete pend[k] }
  BEGIN{ ctx[0]="" }
  {
    line=$0
    if (line ~ /\t/) { print "ERROR:tab character on line " NR > "/dev/stderr"; exit 3 }
    if (line ~ /^[ \t]*(#|$)/) next
    if (line ~ /^---[ \t]*$/) next
    match(line,/^ */); ind=RLENGTH; s=substr(line,ind+1)
    if (ind % 2 != 0) { print "ERROR:odd indentation on line " NR > "/dev/stderr"; exit 3 }
    if (s ~ /^- /) {
      if (!(ind in pend)) { print "ERROR:list item without a parent key on line " NR > "/dev/stderr"; exit 3 }
      parent=pend[ind]; idx=cnt[parent]+0; cnt[parent]=idx+1
      item=trim(substr(s,3)); ip=parent "." idx
      if (item ~ /^[A-Za-z_][A-Za-z0-9_-]*:([ \t]|$)/) {
        k=item; sub(/:.*$/,"",k); v=item; sub(/^[^:]*:[ \t]*/,"",v)
        for(kk in ctx) if(kk+0>ind+2) delete ctx[kk]
        ctx[ind+2]=ip
        if (v=="") { pend[ind+2]=join(ip,k); pend[ind+4]=join(ip,k); ctx[ind+4]=join(ip,k) }
        else emit_val(join(ip,k),v)
      } else emit_val(ip,item)
      next
    }
    if (s !~ /^[A-Za-z_][A-Za-z0-9_-]*:([ \t]|$)/) { print "ERROR:cannot parse line " NR ": " s > "/dev/stderr"; exit 3 }
    if (!(ind in ctx)) { print "ERROR:unexpected indentation on line " NR > "/dev/stderr"; exit 3 }
    k=s; sub(/:.*$/,"",k); v=s; sub(/^[^:]*:[ \t]*/,"",v)
    clear_from(ind)
    p=join(ctx[ind],k)
    if (v=="") { pend[ind]=p; pend[ind+2]=p; ctx[ind+2]=p }
    else emit_val(p,v)
  }' "$1"
}
# value of a flattened key (first match); empty if absent
# One process each. These run thousands of times per command — the rule loader alone asks
# for well over a thousand keys — and the sed|sed|head form they replaced cost four forks
# per lookup, which was most of a command's wall time.
mj_yget()  { awk -v k="$2" 'index($0, k "=") == 1 { print substr($0, length(k) + 2); exit }' "$1"; }
# list values under a key prefix: key.0, key.1 ...
mj_ylist() { awk -v k="$2" 'index($0, k ".") == 1 { r = substr($0, length(k) + 2); if (r ~ /^[0-9]+=/) { sub(/^[0-9]+=/, "", r); print r } }' "$1"; }
# keys not matching any regex in an allowlist file -> printed; returns 1 if any
mj_yaml_unknown_keys() {
  local flat="$1" allow="$2" bad=0 key
  while IFS='=' read -r key _; do
    grep -qE -f "$allow" <<<"$key" || { printf '%s\n' "$key"; bad=1; }
  done < "$flat"
  return $bad
}

# ---------------------------------------------------------------- regions and stamps
# A generated target describes itself. In file mode its first line is a stamp naming the
# policy hash it was rendered from and the hash of the content below it; in region mode the
# begin marker carries the same two hashes for the region body. Nothing else records what
# was generated: a fresh clone carries the evidence inside the target, so a hand edit is
# detected wherever the file is, and there is no provenance file to keep in step with it.
#
# A region projection owns only the text between two markers in a file it does not
# otherwise control, so Majordomus can be adopted by a repository that already has a
# hand-written CLAUDE.md. The marker is not part of the owned content, so re-hashing the
# policy alone is never a hand edit.
MJ_REGION_BEGIN_RE='^<!-- majordomus:begin( [0-9a-f]+)?( [0-9a-f]+)? -->$'
MJ_REGION_END_RE='^<!-- majordomus:end -->$'
MJ_REGION_END='<!-- majordomus:end -->'
MJ_STAMP_RE='^<!-- generated by `majordomus update` from .* \(policy [0-9a-f]+, content [0-9a-f]+\) .* -->$'

# the first line of a file-mode target: mj_stamp_line POLICY_SHA CONTENT_SHA
mj_stamp_line() {
  printf -- '<!-- generated by `majordomus update` from %s (policy %s, content %s) — do not edit; edit the policy and regenerate -->\n' \
    "$(mj_rel "$MJ_POLICY_FILE")" "${1:0:12}" "${2:0:16}"
}
# the stamp a target carries, as "policy content" on stdout; exit 1 when it carries none
# mj_stamp_read FILE MODE
mj_stamp_read() {
  local line
  if [ "$2" = region ]; then
    line="$(grep -E "$MJ_REGION_BEGIN_RE" "$1" 2>/dev/null | head -n 1)"
    set -- $line; [ $# -ge 5 ] || return 1
    printf '%s %s' "$3" "$4"
  else
    line="$(head -n 1 "$1" 2>/dev/null)"
    printf '%s' "$line" | grep -qE "$MJ_STAMP_RE" || return 1
    printf '%s' "$line" | sed -E 's/^.*\(policy ([0-9a-f]+), content ([0-9a-f]+)\).*$/\1 \2/'
  fi
}
# the content a stamp covers: everything below the stamp line, or the region body
# mj_owned_content FILE MODE -> stdout; exit codes as mj_region_extract for region mode
mj_owned_content() {
  if [ "$2" = region ]; then mj_region_extract "$1"
  elif mj_stamp_read "$1" file >/dev/null; then tail -n +2 "$1"
  else cat "$1"; fi
}

# prints the region body of file $1
# exit 0 found · 1 no begin marker · 2 malformed (unclosed, out of order, or repeated)
mj_region_extract() {
  awk -v b="$MJ_REGION_BEGIN_RE" -v e="$MJ_REGION_END_RE" '
    $0 ~ b { if (seen) { bad = 2; exit } seen = 1; inside = 1; next }
    $0 ~ e { if (!inside) { bad = 2; exit } inside = 0; closed = 1; next }
    inside { print }
    END { if (bad) exit bad; if (!seen) exit 1; if (!closed) exit 2 }' "$1"
}

# does file $1 carry a begin marker at all?
mj_region_present() { grep -qE "$MJ_REGION_BEGIN_RE" "$1" 2>/dev/null; }

# host file $1 (may be absent), region body file $2, marker payload $3 ("policy content"
# hashes) -> whole new file on stdout. An existing region is replaced in place; otherwise
# the region is appended.
mj_region_splice() {
  local host="$1" body="$2" sha="$3"
  if [ -f "$host" ] && mj_region_present "$host"; then
    awk -v b="$MJ_REGION_BEGIN_RE" -v e="$MJ_REGION_END_RE" -v body="$body" -v sha="$sha" '
      $0 ~ b { print "<!-- majordomus:begin " sha " -->"
               while ((getline l < body) > 0) print l
               close(body)
               print "<!-- majordomus:end -->"
               skip = 1; next }
      $0 ~ e { skip = 0; next }
      !skip  { print }' "$host"
  else
    if [ -f "$host" ]; then cat "$host"; printf '\n'; fi
    printf -- '<!-- majordomus:begin %s -->\n' "$sha"
    cat "$body"
    printf '%s\n' "$MJ_REGION_END"
  fi
}

# projection mode of index $1: "file" (whole file) or "region" (between markers)
mj_projection_mode() {
  local m; m="$(mj_pol "projections.$1.mode")"
  case "$m" in ""|file) printf 'file' ;; region) printf 'region' ;; *) printf '%s' "$m" ;; esac
}

# ---------------------------------------------------------------- ledger
# mj_ledger_append event 'extra json fields without braces'
mj_ledger_append() {
  local ev="$1" extra="${2:-}" line sid
  line="{\"ts\":\"$(mj_now)\",\"event\":\"$ev\",\"head\":\"$(mj_git_head)\",\"branch\":\"$(mj_json_esc "$(mj_git_branch)")\",\"by\":\"majordomus/$MJ_VERSION\""
  sid="$(mj_open_session_id)"
  [ -n "$sid" ] && line="$line,\"session\":\"$sid\""
  [ -n "$extra" ] && line="$line,$extra"
  # the local half is never tracked, so a second worktree of the same repository starts
  # without it; the first write creates it
  mkdir -p "$MJ_STATE_DIR"
  printf '%s}\n' "$line" >> "$MJ_STATE_DIR/ledger.jsonl"
}

# The open session in THIS worktree, or nothing. Read with two seds rather than the YAML
# parser: this runs on every ledger append, and the two fields it needs are top-level
# scalars written by one command.
#
# Which episode wrote an event is a fact the machine knows, so it is stamped like `head`
# and `branch` are, rather than reconstructed later from a time range. A time range cannot
# tell two workers apart — the ledger is one file per repository, and a session that
# selected by time alone claimed another session's checkpoints the first time this was run
# for real. A line with no session belongs to no episode, which is the honest answer for
# work done outside one: sessions are optional, and nothing is attributed by proximity.
mj_open_session_id() {
  local f w
  f="$MJ_STATE_DIR/session-current.yaml"
  [ -f "$f" ] || return 0
  w="$(sed -n 's/^worktree: //p' "$f" | head -n 1)"
  [ -n "$w" ] && [ "$w" != "$MJ_ROOT" ] && return 0
  sed -n 's/^session_id: //p' "$f" | head -n 1
}

# ---------------------------------------------------------------- current task
mj_load_current() {
  MJ_CUR="$MJ_STATE_DIR/current.yaml"
  [ -f "$MJ_CUR" ] || return 1
  MJ_CUR_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.cur.XXXXXX")"
  mj_yaml_flatten "$MJ_CUR" > "$MJ_CUR_FLAT" || return 2
  return 0
}
mj_cur() { [ -n "${MJ_CUR_FLAT:-}" ] || return 0; mj_yget "$MJ_CUR_FLAT" "$1"; }

mj_load_policy() {
  MJ_POL_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.pol.XXXXXX")"
  mj_yaml_flatten "$MJ_POLICY_FILE" > "$MJ_POL_FLAT" || return 1
}
mj_pol() { [ -n "${MJ_POL_FLAT:-}" ] || return 0; mj_yget "$MJ_POL_FLAT" "$1"; }
# A policy value the code depends on. There is no default here on purpose: a fallback
# written beside the reader is a second source of truth for the same number, and a reader
# that silently substitutes its own value enforces something the configuration does not
# say. A missing key is a policy error, and doctor names the key.
# A required policy value. Almost every caller uses this inside a command substitution,
# where mj_die can only exit the subshell — the parent then carries on with an empty
# string, which is how an installation with an older policy produced
# "[: : integer expected" and a finding that read "17 lines over budget " with no number.
# A helper that is meant to fail closed and fails open in its usual position is worse
# than no helper, so this returns non-zero and the caller decides: a command dies, a
# validator reports. Callers must check, and mj_validate_policy_keys enforces that they do.
mj_pol_req() {
  local v; v="$(mj_pol "$1")"
  if [ -z "$v" ]; then
    printf 'majordomus: policy is missing required key %s (%s)\n' "$1" "$(mj_rel "$MJ_POLICY_FILE")" >&2
    return "$MJ_EX_CONTRACT"
  fi
  printf '%s' "$v"
}
mj_load_profile() {
  local name="$1"
  # cleared first: a failed load must not leave the previous profile readable as this one
  rm -f "${MJ_PRO_FLAT:-}" 2>/dev/null || true; MJ_PRO_FLAT=""
  [ -f "$MJ_PROFILES_DIR/$name.yaml" ] || return 1
  MJ_PRO_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.pro.XXXXXX")"
  mj_yaml_flatten "$MJ_PROFILES_DIR/$name.yaml" > "$MJ_PRO_FLAT" || return 2
}
mj_pro() { [ -n "${MJ_PRO_FLAT:-}" ] || return 0; mj_yget "$MJ_PRO_FLAT" "$1"; }

mj_cleanup() { rm -f "${MJ_CUR_FLAT:-}" "${MJ_POL_FLAT:-}" "${MJ_PRO_FLAT:-}" 2>/dev/null; }
trap mj_cleanup EXIT

# ---------------------------------------------------------------- records
# A record is a Markdown file with computed YAML front matter and an authored body.
# Handovers and checkpoints share this shape; only the directory and the caller's
# extra front-matter lines differ.

# front matter of a record (between the first --- and the next ---), empty if malformed
mj_record_front() { awk 'NR==1&&$0!="---"{exit 2} NR>1&&$0=="---"{exit} NR>1' "$1"; }
# body of a record: everything after the second ---
mj_record_body()  { awk 'c>=2{print} /^---$/{c++}' "$1"; }

# refuse a body that carries fields Majordomus computes: prose must not forge identity
mj_reject_identity() {
  grep -qE '^(schema_version|created_at|task_id|repository_id|worktree|branch|head|working_tree|changed_files):' "$1"
}

# publish content as a new file in a directory, atomically, mode 0600, never overwriting.
# mj_publish_record DIR NAME_PREFIX CONTENT_FILE -> prints the final path
mj_publish_record() {
  local dir="$1" prefix="$2" src="$3" tmp final n=0 head
  mkdir -p "$dir"
  tmp="$(mktemp "$dir/.tmp.XXXXXX")"; chmod 600 "$tmp"; cat "$src" > "$tmp"
  head="$(mj_git_head)"
  while :; do
    final="$dir/$(mj_now_compact)--${prefix:+$prefix--}$(mj_branch_key)--${head:0:7}--$(mj_rand16).md"
    ln "$tmp" "$final" 2>/dev/null && break
    n=$((n + 1)); [ "$n" -lt 10 ] || { rm -f "$tmp"; return 1; }
  done
  rm -f "$tmp"
  printf '%s\n' "$final"
}

# write the computed front matter of a record to stdout. $1 = task id or "none", $2 = profile,
# $3 = owner, remaining args are extra "key: value" lines appended verbatim.
mj_record_front_matter() {
  local task_id="${1:-none}" profile="${2:-none}" owner="${3:-}"; shift 3 2>/dev/null || true
  printf -- '---\nschema_version: 1\ncreated_at: %s\ntask_id: %s\nprofile: %s\nowner: "%s"\n' \
    "$(mj_now)" "$task_id" "$profile" "$(printf '%s' "$owner" | sed 's/"/\\"/g')"
  printf 'repository_id: %s\nworktree: %s\nbranch: %s\nhead: %s\nworking_tree: %s\nchanged_files:\n' \
    "$(mj_git_repo_id)" "$MJ_ROOT" "$(mj_git_branch)" "$(mj_git_head)" "$(mj_git_dirty)"
  mj_git status --porcelain=v1 2>/dev/null | cut -c4- | sed 's/^.* -> //' | sed 's/^/  - /'
  local extra; for extra in "$@"; do printf '%s\n' "$extra"; done
  printf -- '---\n\n'
}

# ---------------------------------------------------------------- resolution
# Resolve the most relevant record in a directory for this worktree and branch.
# Tier 0: same repository, same worktree, same branch. Tier 1: same repository, same
# non-detached branch. Never repository-wide: an unrelated worktree's record must not
# become current context. Newest wins within a tier.
# mj_resolve_latest DIR [TASK_ID]  ->  0 and sets MJ_RES_*, or 1 when nothing matches.
# Malformed records are skipped with a warning on stderr, never silently.
MJ_RES_PATH=""; MJ_RES_TIER=""; MJ_RES_MATCH=""; MJ_RES_HEAD=""; MJ_RES_BRANCH=""
MJ_RES_DIRTY=""; MJ_RES_CREATED=""; MJ_RES_TASK=""; MJ_RES_SKIPPED=0
mj_resolve_latest() {
  local dir="$1" want_task="${2:-}" f fm flat tier key best="" best_key=""
  local my_id my_branch; my_id="$(mj_git_repo_id)"; my_branch="$(mj_git_branch)"
  MJ_RES_PATH=""; MJ_RES_SKIPPED=0
  [ -d "$dir" ] || return 1
  for f in "$dir"/*.md; do
    [ -f "$f" ] || continue
    fm="$(mktemp "${TMPDIR:-/tmp}/mj.fm.XXXXXX")"
    mj_record_front "$f" > "$fm" || { rm -f "$fm"; mj_err "warning: skipped $f: no front matter"; MJ_RES_SKIPPED=$((MJ_RES_SKIPPED+1)); continue; }
    flat="$(mktemp "${TMPDIR:-/tmp}/mj.fl.XXXXXX")"
    if ! mj_yaml_flatten "$fm" > "$flat" 2>/dev/null; then
      rm -f "$fm" "$flat"; mj_err "warning: skipped $f: malformed front matter"; MJ_RES_SKIPPED=$((MJ_RES_SKIPPED+1)); continue; fi
    if [ "$(mj_yget "$flat" schema_version)" != 1 ] || [ -z "$(mj_yget "$flat" head)" ] || [ -z "$(mj_yget "$flat" created_at)" ]; then
      rm -f "$fm" "$flat"; mj_err "warning: skipped $f: missing required fields"; MJ_RES_SKIPPED=$((MJ_RES_SKIPPED+1)); continue; fi
    if [ -n "$want_task" ] && [ "$(mj_yget "$flat" task_id)" != "$want_task" ]; then rm -f "$fm" "$flat"; continue; fi
    tier=""
    if [ "$(mj_yget "$flat" repository_id)" = "$my_id" ]; then
      if [ "$(mj_yget "$flat" worktree)" = "$MJ_ROOT" ] && [ "$(mj_yget "$flat" branch)" = "$my_branch" ]; then tier=0
      elif [ "$my_branch" != DETACHED ] && [ "$(mj_yget "$flat" branch)" = "$my_branch" ]; then tier=1; fi
    fi
    if [ -n "$tier" ]; then
      key="$tier|$(mj_yget "$flat" created_at)|$(mj_record_rank "$f")"
      if [ -z "$best" ] || [ "${key%%|*}" -lt "${best_key%%|*}" ] || { [ "${key%%|*}" = "${best_key%%|*}" ] && [ "${key#*|}" \> "${best_key#*|}" ]; }; then
        best="$f"; best_key="$key"
        MJ_RES_HEAD="$(mj_yget "$flat" head)"; MJ_RES_BRANCH="$(mj_yget "$flat" branch)"
        MJ_RES_DIRTY="$(mj_yget "$flat" working_tree)"; MJ_RES_CREATED="$(mj_yget "$flat" created_at)"
        MJ_RES_TASK="$(mj_yget "$flat" task_id)"
      fi
    fi
    rm -f "$fm" "$flat"
  done
  [ -n "$best" ] || return 1
  MJ_RES_PATH="$best"; MJ_RES_TIER="${best_key%%|*}"
  [ "$MJ_RES_TIER" = 0 ] && MJ_RES_MATCH=same_worktree_same_branch || MJ_RES_MATCH=same_branch
  return 0
}

# Position of a record in the ledger, zero-padded, or 000000 when nothing recorded it.
# created_at has second resolution, so two records written inside one second would
# otherwise resolve in an order decided by a random filename suffix. The ledger is
# append-only and written in the order the commands ran, which makes it the one portable
# monotonic tiebreak available without sub-second timestamps.
mj_record_rank() {
  local base n
  base="$(basename "$1")"
  n="$(grep -n -F -- "$base" "$MJ_STATE_DIR/ledger.jsonl" 2>/dev/null | tail -n 1 | cut -d: -f1)"
  printf '%06d' "${n:-0}"
}

# age of an ISO timestamp in whole minutes; empty when unparseable
mj_age_minutes() {
  local e; e="$(mj_epoch "$1")"; [ -n "$e" ] || return 1
  printf '%s' $(( ( $(mj_epoch "$(mj_now)") - e ) / 60 ))
}
# "18" -> "18m ago"; "1500" -> "25h ago"
mj_age_human() {
  local m="$1"
  if [ -z "$m" ]; then printf 'unknown'
  elif [ "$m" -lt 90 ]; then printf '%sm ago' "$m"
  elif [ "$m" -lt 2880 ]; then printf '%sh ago' $((m/60))
  else printf '%sd ago' $((m/1440)); fi
}

# value of a flat JSON key on one ledger line: mj_json_field LINE KEY
mj_json_field() {
  printf '%s' "$1" | awk -v k="$2" '
    { s=$0
      if (match(s, "\"" k "\":\"")) { r=substr(s, RSTART+length(k)+4); sub(/".*/,"",r); print r; exit }
      if (match(s, "\"" k "\":")) { r=substr(s, RSTART+length(k)+3); sub(/[,}].*/,"",r); print r; exit } }'
}

# line numbers of ledger lines that are not well-formed events (need ts and event)
mj_ledger_bad_lines() {
  [ -f "$1" ] || return 0
  awk '{ if ($0 !~ /^\{/ || $0 !~ /"ts":"/ || $0 !~ /"event":"/ || $0 !~ /\}$/) printf "%s ", NR }' "$1"
}

# Does a string contain a newline? Written as a function because the obvious inline form,
# case "$v" in *"$(printf '\n')"*), can never match: command substitution strips the
# trailing newline, leaving an empty pattern that matches everything.
mj_is_multiline() { case "$1" in *"$(printf 'x\ny')"*) return 0 ;; esac
  [ "$(printf '%s' "$1" | wc -l | tr -d ' ')" != 0 ]; }
