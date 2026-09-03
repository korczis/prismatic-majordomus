#!/usr/bin/env bash
# doctor — is Majordomus itself healthy and actually wired here? Read-only.
mj_cmd_doctor() {
  local a
  for a in "$@"; do case "$a" in
    --help|-h) echo "usage: majordomus doctor [--json]   (read-only; exit 0 healthy, 10 failures, 12 missing)"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "doctor: unknown option $a" ;;
  esac; done
  mj_require_installed
  local missing=0

  # 1. policy parses, version, unknown keys
  if mj_load_policy 2>/dev/null; then
    if [ "$(mj_pol version)" = 1 ]; then
      local unk; unk="$(mj_yaml_unknown_keys "$MJ_POL_FLAT" "$MJ_BIN_DIR/../share/allow/policy.txt" || true)"
      if [ -z "$unk" ]; then mj_ok policy ".majordomus/policy.yaml" "parsed, version 1"
      else mj_fail policy ".majordomus/policy.yaml" "unknown keys: $(printf '%s' "$unk" | tr '\n' ' ')" "grep -nE '$(printf '%s' "$unk" | sed 's/\..*//' | sort -u | tr '\n' '|' | sed 's/|$//')' .majordomus/policy.yaml"; fi
    else mj_fail policy ".majordomus/policy.yaml" "unsupported version '$(mj_pol version)' (want 1)"; fi
  else
    mj_fail policy ".majordomus/policy.yaml" "does not parse: $(mj_yaml_flatten "$MJ_DIR/policy.yaml" 2>&1 >/dev/null | sed 's/^ERROR://')" "majordomus doctor"
    mj_finish_doctor "$missing"; return
  fi

  # 2. profiles
  local pf n count=0 unk
  for pf in "$MJ_DIR"/profiles/*.yaml; do
    [ -f "$pf" ] || continue; count=$((count+1)); n="$(basename "$pf" .yaml)"
    if mj_load_profile "$n" 2>/dev/null; then
      unk="$(mj_yaml_unknown_keys "$MJ_PRO_FLAT" "$MJ_BIN_DIR/../share/allow/profile.txt" || true)"
      [ -n "$unk" ] && mj_fail profiles "$n" "unknown keys: $(printf '%s' "$unk" | tr '\n' ' ')" "cat .majordomus/profiles/$n.yaml"
      [ "$(mj_pro name)" = "$n" ] || mj_fail profiles "$n" "name field '$(mj_pro name)' does not match filename"
    else mj_fail profiles "$n" "does not parse" "cat .majordomus/profiles/$n.yaml"; fi
  done
  local def; def="$(mj_pol profiles.default)"
  if [ -f "$MJ_DIR/profiles/$def.yaml" ]; then mj_ok profiles "$count files" "parsed; default '$def' exists"
  else mj_fail profiles "default" "profiles.default='$def' has no file .majordomus/profiles/$def.yaml"; fi

  # 3. enforcement wiring
  local i=0 name path wired kind target hookdir hookfile arg0
  while [ -n "$(mj_pol "enforcement.$i.name")" ]; do
    name="$(mj_pol "enforcement.$i.name")"; path="$(mj_pol "enforcement.$i.path")"; wired="$(mj_pol "enforcement.$i.wired_by")"
    arg0="$(mj_pol "enforcement.$i.args.0")"; [ -z "$arg0" ] && arg0="$(mj_pol "enforcement.$i.args" | sed 's/\[\]//')"
    # path may be repo-relative, absolute, or a command name; the hook line may also carry its own path
    local resolved=""
    case "$path" in
      /*) [ -x "$path" ] && resolved="$path" ;;
      */*) [ -x "$MJ_ROOT/$path" ] && resolved="$MJ_ROOT/$path" ;;
      *) mj_has "$path" && resolved="$(command -v "$path")" ;;
    esac
    if [ -z "$resolved" ] && [ -e "$MJ_ROOT/$path" ] && [ ! -x "$MJ_ROOT/$path" ]; then
      mj_fail wiring "$name" "'$path' exists but is not executable" "chmod +x $path"
    else
      kind="${wired%%:*}"; target="${wired#*:}"
      case "$kind" in
        git-hook)
          hookdir="$(mj_git config core.hooksPath 2>/dev/null || true)"; [ -z "$hookdir" ] && hookdir=".git/hooks"
          case "$hookdir" in /*) hookfile="$hookdir/$target" ;; *) hookfile="$MJ_ROOT/$hookdir/$target" ;; esac
          if [ ! -f "$hookfile" ]; then mj_fail wiring "$name" "hook $hookdir/$target does not exist" "ls -l $hookdir/$target"
          elif [ ! -x "$hookfile" ]; then mj_fail wiring "$name" "hook $hookdir/$target is not executable" "chmod +x $hookdir/$target"
          elif ! grep -qE "majordomus[[:space:]]+$arg0([[:space:]]|$)" "$hookfile"; then
            mj_fail wiring "$name" "$(basename "$path") $arg0 is not invoked by $hookdir/$target" "grep -n 'majordomus $arg0' $hookdir/$target"
          elif [ -z "$resolved" ] && ! mj_hook_binary_ok "$hookfile" "$arg0"; then
            mj_fail wiring "$name" "'$path' is not on PATH and $hookdir/$target does not name an executable majordomus" "grep -n 'majordomus $arg0' $hookdir/$target"
          elif grep -E "majordomus[[:space:]]+$arg0" "$hookfile" | grep -qE '\|\|[[:space:]]*(true|exit[[:space:]]+0)'; then
            mj_fail wiring "$name" "$hookdir/$target invokes it but swallows the exit code (|| true)" "grep -n 'majordomus $arg0' $hookdir/$target"
          else mj_ok wiring "$name" "wired via $hookdir/$target"; fi ;;
        ci)
          if [ ! -f "$MJ_ROOT/$target" ]; then mj_fail wiring "$name" "ci file $target does not exist"
          elif ! grep -qE "majordomus[[:space:]]+$arg0" "$MJ_ROOT/$target"; then mj_fail wiring "$name" "$target does not invoke majordomus $arg0" "grep -n majordomus $target"
          else mj_ok wiring "$name" "invoked from $target"; fi ;;
        manual) mj_info wiring "$name" "wired_by: manual — documented, not verified" ;;
        *) mj_fail wiring "$name" "unknown wired_by kind '$kind' (git-hook:<name> | ci:<path> | manual)" ;;
      esac
    fi
    i=$((i+1))
  done
  [ "$i" = 0 ] && mj_warn wiring "policy" "no enforcement entries declared; nothing is wired to run majordomus"

  # 4. projections exist and match fingerprints
  local fp="$MJ_DIR/generated/fingerprints.yaml" fpflat="" j=0 tgt want have always="" budget
  if [ -f "$fp" ]; then fpflat="$(mktemp "${TMPDIR:-/tmp}/mj.fp.XXXXXX")"; mj_yaml_flatten "$fp" > "$fpflat" 2>/dev/null || { rm -f "$fpflat"; fpflat=""; }; fi
  while [ -n "$(mj_pol "projections.$j.target")" ]; do
    tgt="$(mj_pol "projections.$j.target")"
    [ "$(mj_pol "projections.$j.always_loaded")" = true ] && always="$tgt"
    if [ ! -f "$MJ_ROOT/$tgt" ]; then mj_fail projection "$tgt" "missing (run: majordomus update)" "majordomus update"; missing=1
    elif [ -z "$fpflat" ]; then mj_fail projection "$tgt" "no fingerprints recorded (run: majordomus update)" "majordomus update"; missing=1
    else
      want="$(mj_fp_sha "$fpflat" "$tgt")"; have="$(mj_sha256 "$MJ_ROOT/$tgt")"
      if [ -z "$want" ]; then mj_fail projection "$tgt" "not in fingerprints (run: majordomus update)" "majordomus update"; missing=1
      elif [ "$want" != "$have" ]; then mj_fail projection "$tgt" "hash differs from fingerprint (hand-edited?)" "majordomus update --diff $tgt"
      else mj_ok projection "$tgt" "fingerprint matches"; fi
    fi
    j=$((j+1))
  done
  [ -n "$fpflat" ] && rm -f "$fpflat"

  # 5. budget on always-loaded projection
  budget="$(mj_pol context.always_loaded_budget_lines)"
  if [ -n "$always" ] && [ -f "$MJ_ROOT/$always" ]; then
    local l; l="$(mj_lines "$MJ_ROOT/$always")"
    if [ "$l" -le "$budget" ]; then mj_ok budget "$always" "$l lines, budget $budget"
    else mj_fail budget "$always" "$l lines, budget $budget" "wc -l $always"; fi
    # 6. links resolve
    local bad=0 l2 dir; dir="$(dirname "$MJ_ROOT/$always")"
    for l2 in $(grep -oE '\]\(([^)#]+)\)' "$MJ_ROOT/$always" | sed -E 's/\]\(([^)]+)\)/\1/' | grep -vE '^https?://' || true); do
      [ -e "$dir/$l2" ] || { mj_fail links "$always" "reference $l2 does not resolve" "ls $l2"; bad=1; }
    done
    [ "$bad" = 0 ] && mj_ok links "$always" "all references resolve"
    # 7. no hardcoded counts
    if grep -qE '\b[0-9]+ (agents|files|apps|commands|skills|rules)\b' "$MJ_ROOT/$always"; then
      mj_fail counts "$always" "hardcoded count in always-loaded context" "grep -nE '[0-9]+ (agents|files|apps|commands|skills|rules)' $always"
    else mj_ok counts "$always" "no hardcoded counts"; fi
  elif [ -z "$always" ]; then mj_warn budget "policy" "no projection is marked always_loaded: true"; fi

  # 8. retention
  local cap ll hc
  cap="$(mj_pol ledger.retention_max_lines)"; ll=0; [ -f "$MJ_DIR/state/ledger.jsonl" ] && ll="$(mj_lines "$MJ_DIR/state/ledger.jsonl")"
  if [ "$ll" -le "${cap:-5000}" ]; then mj_ok retention "ledger" "$ll lines, cap ${cap:-5000}"; else mj_fail retention "ledger" "$ll lines over cap $cap" "wc -l .majordomus/state/ledger.jsonl"; fi
  cap="$(mj_pol handover.retention_max_files)"; hc="$(find "$MJ_DIR/state/handovers" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$hc" -le "${cap:-200}" ]; then mj_ok retention "handovers" "$hc files, cap ${cap:-200}"; else mj_fail retention "handovers" "$hc files over cap $cap" "ls .majordomus/state/handovers | wc -l"; fi

  # 9. environment
  local env="bash ${BASH_VERSION%%(*}"; env="$env, git $(git --version | awk '{print $3}')"
  mj_has jq && env="$env, jq $(jq --version 2>/dev/null | sed 's/jq-//')" || env="$env, jq absent"
  mj_has shellcheck && env="$env, shellcheck present"
  mj_info env "-" "$env"

  mj_finish_doctor "$missing"
}
# does the hook line that invokes "majordomus <arg0>" name an executable binary?
mj_hook_binary_ok() {
  local hookfile="$1" arg0="$2" tok
  for tok in $(grep -E "majordomus[[:space:]]+$arg0" "$hookfile" | grep -oE '[^[:space:]"'"'"']*majordomus'); do
    case "$tok" in
      /*) [ -x "$tok" ] && return 0 ;;
      */*) [ -x "$MJ_ROOT/$tok" ] && return 0 ;;
      majordomus) mj_has majordomus && return 0 ;;
    esac
  done
  return 1
}
mj_fp_sha() { # fingerprints flat file, target -> sha
  local f="$1" t="$2" k=0
  while [ -n "$(mj_yget "$f" "targets.$k.target")" ]; do
    [ "$(mj_yget "$f" "targets.$k.target")" = "$t" ] && { mj_yget "$f" "targets.$k.sha256"; return; }
    k=$((k+1))
  done
}
mj_finish_doctor() {
  local missing="$1"
  [ "$MJ_JSON" = 1 ] || printf 'doctor: %s failure(s)\n' "$MJ_FAILS"
  if [ "$missing" = 1 ]; then exit "$MJ_EX_MISSING"; fi
  [ "$MJ_FAILS" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_CONTRACT"
}
