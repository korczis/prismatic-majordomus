#!/usr/bin/env bash
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
  MJ_DIR="$MJ_ROOT/.majordomus"
  export MJ_ROOT MJ_DIR
  # hooks inherited from a parent process must never redirect our git calls
  unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX
}
mj_require_installed() {
  mj_require_repo
  [ -f "$MJ_DIR/policy.yaml" ] || mj_die "$MJ_EX_MISSING" "no .majordomus/policy.yaml in $MJ_ROOT (run: majordomus init)"
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
mj_lines() { wc -l < "$1" | tr -d ' '; }
mj_has()   { command -v "$1" >/dev/null 2>&1; }

# normalise a repo-relative path: strip ./ and trailing /, collapse //, refuse escapes
mj_norm_path() {
  local p="$1"
  p="$(printf '%s' "$p" | sed -e 's#^\./##' -e 's#//*#/#g' -e 's#/$##')"
  case "$p" in ""|/*|../*|*/../*|*/..|..) return 1 ;; esac
  printf '%s' "$p"
}
# does path a contain path b (or equal)?
mj_path_contains() { case "$2" in "$1"|"$1"/*) return 0 ;; esac; return 1; }

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
mj_yget() { sed -n "s/^$(printf '%s' "$2" | sed 's/[.[\*^$]/\\&/g')=//p" "$1" | head -n 1; }
# list values under a key prefix: key.0, key.1 ...
mj_ylist() { sed -n "s/^$(printf '%s' "$2" | sed 's/[.[\*^$]/\\&/g')\.[0-9][0-9]*=//p" "$1"; }
# keys not matching any regex in an allowlist file -> printed; returns 1 if any
mj_yaml_unknown_keys() {
  local flat="$1" allow="$2" bad=0 key
  while IFS='=' read -r key _; do
    grep -qE -f "$allow" <<<"$key" || { printf '%s\n' "$key"; bad=1; }
  done < "$flat"
  return $bad
}

# ---------------------------------------------------------------- ledger
# mj_ledger_append event 'extra json fields without braces'
mj_ledger_append() {
  local ev="$1" extra="${2:-}" line
  line="{\"ts\":\"$(mj_now)\",\"event\":\"$ev\",\"head\":\"$(mj_git_head)\",\"branch\":\"$(mj_json_esc "$(mj_git_branch)")\",\"by\":\"majordomus/$MJ_VERSION\""
  [ -n "$extra" ] && line="$line,$extra"
  printf '%s}\n' "$line" >> "$MJ_DIR/state/ledger.jsonl"
}

# ---------------------------------------------------------------- current task
mj_load_current() {
  MJ_CUR="$MJ_DIR/state/current.yaml"
  [ -f "$MJ_CUR" ] || return 1
  MJ_CUR_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.cur.XXXXXX")"
  mj_yaml_flatten "$MJ_CUR" > "$MJ_CUR_FLAT" || return 2
  return 0
}
mj_cur() { mj_yget "$MJ_CUR_FLAT" "$1"; }

mj_load_policy() {
  MJ_POL_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.pol.XXXXXX")"
  mj_yaml_flatten "$MJ_DIR/policy.yaml" > "$MJ_POL_FLAT" || return 1
}
mj_pol() { mj_yget "$MJ_POL_FLAT" "$1"; }
mj_load_profile() {
  local name="$1"
  [ -f "$MJ_DIR/profiles/$name.yaml" ] || return 1
  MJ_PRO_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.pro.XXXXXX")"
  mj_yaml_flatten "$MJ_DIR/profiles/$name.yaml" > "$MJ_PRO_FLAT" || return 2
}
mj_pro() { mj_yget "$MJ_PRO_FLAT" "$1"; }

mj_cleanup() { rm -f "${MJ_CUR_FLAT:-}" "${MJ_POL_FLAT:-}" "${MJ_PRO_FLAT:-}" 2>/dev/null; }
trap mj_cleanup EXIT
