#!/usr/bin/env bash
# doctor — is Majordomus itself healthy and actually wired here? Read-only.
# shellcheck source=question.sh
. "$MJ_LIB_DIR/question.sh"
# shellcheck source=decision.sh
. "$MJ_LIB_DIR/decision.sh"
# shellcheck source=prompt.sh
. "$MJ_LIB_DIR/prompt.sh"
# shellcheck source=context.sh
. "$MJ_LIB_DIR/context.sh"
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
          # A hook is commonly a dispatcher that runs every executable in <hook>.d/; the
          # invocation then lives in one of those files, not in the hook git calls.
          local wirefile="" cand rel
          if [ -f "$hookfile" ]; then
            for cand in $(mj_hook_candidates "$hookfile"); do
              if grep -qE "majordomus[[:space:]]+$arg0([[:space:]]|$)" "$cand"; then wirefile="$cand"; break; fi
            done
          fi
          rel="${wirefile#"$MJ_ROOT"/}"
          if [ ! -f "$hookfile" ]; then mj_fail wiring "$name" "hook $hookdir/$target does not exist" "ls -l $hookdir/$target"
          elif [ ! -x "$hookfile" ]; then mj_fail wiring "$name" "hook $hookdir/$target is not executable" "chmod +x $hookdir/$target"
          elif [ -z "$wirefile" ]; then
            mj_fail wiring "$name" "$(basename "$path") $arg0 is not invoked by $hookdir/$target or anything in $hookdir/$target.d/" "grep -rn 'majordomus $arg0' $hookdir/$target $hookdir/$target.d 2>/dev/null"
          elif [ ! -x "$wirefile" ]; then
            mj_fail wiring "$name" "$rel invokes it but is not executable, so the dispatcher skips it" "chmod +x $rel"
          elif [ -z "$resolved" ] && ! mj_hook_binary_ok "$wirefile" "$arg0"; then
            mj_fail wiring "$name" "'$path' is not on PATH and $rel does not name an executable majordomus" "grep -n 'majordomus $arg0' $rel"
          elif grep -E "majordomus[[:space:]]+$arg0" "$wirefile" | grep -qE '\|\|[[:space:]]*(true|exit[[:space:]]+0)'; then
            mj_fail wiring "$name" "$rel invokes it but swallows the exit code (|| true)" "grep -n 'majordomus $arg0' $rel"
          else mj_ok wiring "$name" "wired via $rel"; fi ;;
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
  local fp="$MJ_DIR/generated/fingerprints.yaml" fpflat="" j=0 tgt want have always="" always_mode="file" budget
  local mode prov owned rc
  owned="$(mktemp "${TMPDIR:-/tmp}/mj.own.XXXXXX")"
  if [ -f "$fp" ]; then fpflat="$(mktemp "${TMPDIR:-/tmp}/mj.fp.XXXXXX")"; mj_yaml_flatten "$fp" > "$fpflat" 2>/dev/null || { rm -f "$fpflat"; fpflat=""; }; fi
  while [ -n "$(mj_pol "projections.$j.target")" ]; do
    tgt="$(mj_pol "projections.$j.target")"; mode="$(mj_projection_mode "$j")"
    prov="$(mj_pol "projections.$j.provider")"
    if [ "$(mj_pol "projections.$j.always_loaded")" = true ]; then always="$tgt"; always_mode="$mode"; fi
    case "$mode" in
      file|region) ;;
      *) mj_fail projection "$tgt" "unknown mode '$mode' (file | region)" "grep -n 'mode:' .majordomus/policy.yaml"; j=$((j+1)); continue ;;
    esac
    # update would die on this; doctor is the command that is supposed to say so first
    [ -f "$MJ_DIR/providers/$prov.tmpl" ] || { mj_fail projection "$tgt" "provider '$prov' has no template .majordomus/providers/$prov.tmpl" "ls .majordomus/providers/"; missing=1; j=$((j+1)); continue; }
    if [ ! -f "$MJ_ROOT/$tgt" ]; then mj_fail projection "$tgt" "missing (run: majordomus update)" "majordomus update"; missing=1
    elif [ -z "$fpflat" ]; then mj_fail projection "$tgt" "no fingerprints recorded (run: majordomus update)" "majordomus update"; missing=1
    else
      want="$(mj_fp_sha "$fpflat" "$tgt")"; have=""
      if [ "$mode" = region ]; then
        rc=0; mj_region_extract "$MJ_ROOT/$tgt" > "$owned" 2>/dev/null || rc=$?
        case "$rc" in
          0) have="$(mj_sha256 "$owned")" ;;
          1) mj_fail projection "$tgt" "region markers are absent (run: majordomus update)" "majordomus update"; missing=1 ;;
          *) mj_fail projection "$tgt" "region markers are malformed (unclosed, out of order, or repeated)" "grep -n 'majordomus:begin\\|majordomus:end' $tgt" ;;
        esac
      else cp "$MJ_ROOT/$tgt" "$owned"; have="$(mj_sha256 "$owned")"; fi
      if [ -n "$have" ]; then
        if [ -z "$want" ]; then mj_fail projection "$tgt" "not in fingerprints (run: majordomus update)" "majordomus update"; missing=1
        elif [ "$want" != "$have" ]; then mj_fail projection "$tgt" "hash differs from fingerprint (hand-edited?)" "majordomus update --diff $tgt"
        else mj_ok projection "$tgt" "fingerprint matches$([ "$mode" = region ] && printf ' (region)')"; fi
      fi
    fi
    j=$((j+1))
  done
  [ -n "$fpflat" ] && rm -f "$fpflat"

  # 5. budget on always-loaded projection
  # These three judge the generated content only. For a region projection that is the
  # region, never the host document: every failure doctor reports must be fixable by
  # editing the policy, and the text around the markers is not Majordomus's to fix.
  budget="$(mj_pol context.always_loaded_budget_lines)"
  local subject="$always"
  if [ -n "$always" ] && [ -f "$MJ_ROOT/$always" ]; then
    local measured="$MJ_ROOT/$always"
    if [ "$always_mode" = region ]; then
      subject="$always (region)"
      rc=0; mj_region_extract "$MJ_ROOT/$always" > "$owned" 2>/dev/null || rc=$?
      [ "$rc" = 0 ] && measured="$owned"
      mj_info context "$always" "host document is $(mj_lines "$MJ_ROOT/$always") lines; the budget below covers the generated region only"
    fi
    local l; l="$(mj_lines "$measured")"
    if [ "$l" -le "$budget" ]; then mj_ok budget "$subject" "$l lines, budget $budget"
    else mj_fail budget "$subject" "$l lines, budget $budget" "majordomus update --dry-run"; fi
    # 6. links resolve
    local bad=0 l2 dir; dir="$(dirname "$MJ_ROOT/$always")"
    for l2 in $(grep -oE '\]\(([^)#]+)\)' "$measured" | sed -E 's/\]\(([^)]+)\)/\1/' | grep -vE '^https?://' || true); do
      [ -e "$dir/$l2" ] || { mj_fail links "$subject" "reference $l2 does not resolve" "ls $l2"; bad=1; }
    done
    [ "$bad" = 0 ] && mj_ok links "$subject" "all references resolve"
    # 7. no hardcoded counts
    if grep -qE '\b[0-9]+ (agents|files|apps|commands|skills|rules)\b' "$measured"; then
      mj_fail counts "$subject" "hardcoded count in always-loaded context" "grep -nE '[0-9]+ (agents|files|apps|commands|skills|rules)' $always"
    else mj_ok counts "$subject" "no hardcoded counts"; fi
  elif [ -z "$always" ]; then mj_warn budget "policy" "no projection is marked always_loaded: true"; fi
  rm -f "$owned"

  # 8. the continuity subsystem: reachable through the CLI, not merely present on disk.
  #    A directory full of records that no command reads is the failure mode this whole
  #    tool exists to catch, so doctor proves each store is readable by the command that
  #    owns it rather than checking that the files exist.
  mj_doctor_continuity

  # 9. retention
  local cap ll hc
  cap="$(mj_pol ledger.retention_max_lines)"; ll=0; [ -f "$MJ_DIR/state/ledger.jsonl" ] && ll="$(mj_lines "$MJ_DIR/state/ledger.jsonl")"
  if [ "$ll" -le "${cap:-5000}" ]; then mj_ok retention "ledger" "$ll lines, cap ${cap:-5000}"; else mj_fail retention "ledger" "$ll lines over cap $cap" "wc -l .majordomus/state/ledger.jsonl"; fi
  cap="$(mj_pol handover.retention_max_files)"; hc="$(find "$MJ_DIR/state/handovers" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$hc" -le "${cap:-200}" ]; then mj_ok retention "handovers" "$hc files, cap ${cap:-200}"; else mj_fail retention "handovers" "$hc files over cap $cap" "ls .majordomus/state/handovers | wc -l"; fi
  cap="$(mj_pol checkpoint.retention_max_files)"; hc="$(find "$MJ_DIR/state/checkpoints" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$hc" -le "${cap:-500}" ]; then mj_ok retention "checkpoints" "$hc files, cap ${cap:-500}"; else mj_fail retention "checkpoints" "$hc files over cap $cap" "majordomus checkpoint --list | wc -l"; fi

  # 10. environment
  local env="bash ${BASH_VERSION%%(*}"; env="$env, git $(git --version | awk '{print $3}')"
  mj_has jq && env="$env, jq $(jq --version 2>/dev/null | sed 's/jq-//')" || env="$env, jq absent"
  mj_has shellcheck && env="$env, shellcheck present"
  mj_info env "-" "$env"

  mj_finish_doctor "$missing"
}
# a hook file plus every file in its <hook>.d/ dispatch directory, in dispatch order
mj_hook_candidates() {
  printf '%s\n' "$1"
  if [ -d "$1.d" ]; then find "$1.d" -type f 2>/dev/null | sort; fi
  return 0
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

# Prove the continuity subsystem is wired, not merely installed. Each check runs the real
# code path a worker would run; a store that cannot be read is a failure, because a record
# nothing reads is indistinguishable from no record at all.
mj_doctor_continuity() {
  local d bad

  # directories the commands write into
  for d in state/handovers state/checkpoints prompts; do
    if [ -d "$MJ_DIR/$d" ]; then mj_ok layout ".majordomus/$d" "present"
    else mj_fail layout ".majordomus/$d" "missing; the command that writes it will create it, but update installs it" "majordomus update"; fi
  done

  # the two stores the finish contract reads must parse, or a gate can be bypassed by a typo
  bad="$(mj_question_malformed "$MJ_DIR/state/open-questions.md")"
  if [ -n "$bad" ]; then mj_fail records "open-questions.md" "line(s) $(printf '%s' "$bad" | sed 's/ $//') do not parse; an unreadable entry cannot block acceptance" "majordomus question list --all"
  else mj_ok records "open-questions.md" "every entry parses"; fi
  bad="$(mj_ledger_bad_lines "$MJ_DIR/state/ledger.jsonl")"
  if [ -n "$bad" ]; then mj_fail records "ledger.jsonl" "line(s) $(printf '%s' "$bad" | sed 's/ $//') are not well-formed events" "majordomus history --validate"
  else mj_ok records "ledger.jsonl" "every line carries ts and event"; fi
  bad="$(mj_decision_malformed "$MJ_DIR/state/decisions.md")"
  [ -n "$bad" ] && mj_warn records "decisions.md" "entry at line(s) $(printf '%s' "$bad" | sed 's/ $//') lacks Task, Head or Why" "majordomus decision list"

  # every prompt asset renders, and every token in it is one the renderer knows
  local f n=0 reason
  if [ -d "$MJ_DIR/prompts" ]; then
    for f in "$MJ_DIR"/prompts/*.md; do
      [ -f "$f" ] || continue; n=$((n + 1))
      reason="$(mj_prompt_validate "$f")" || mj_fail prompt "$(basename "$f" .md)" "$(printf '%s' "$reason" | tr '\n' ';' | sed 's/;$//')" "majordomus prompt show $(basename "$f" .md)"
    done
    [ "$n" -gt 0 ] && mj_ok prompt "$n asset(s)" "front matter valid, every token known"
  fi

  # the resolver runs and reports either a record or its absence; both are healthy
  if mj_resolve_latest "$MJ_DIR/state/handovers" ""; then
    mj_ok resolver "handovers" "$MJ_RES_MATCH, $(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH")" "majordomus handover --resolve"
  else mj_ok resolver "handovers" "no record for this worktree and branch (absence, not a stale match)" "majordomus handover --resolve"; fi
  [ "$MJ_RES_SKIPPED" -gt 0 ] && mj_fail resolver "handovers" "$MJ_RES_SKIPPED record(s) skipped as malformed" "majordomus handover --list"

  # the context builder produces output within its own budget through the real command path
  local budget out lines
  budget="$(mj_pol context.builder_budget_lines)"; [ -n "$budget" ] || budget=300
  out="$(mktemp "${TMPDIR:-/tmp}/mj.dc.XXXXXX")"
  if ( export MJ_JSON=0; mj_cmd_context ) > "$out" 2>/dev/null; then
    lines="$(mj_lines "$out")"
    if [ "$lines" -le "$budget" ]; then mj_ok context "builder" "$lines lines, budget $budget" "majordomus context"
    else mj_fail context "builder" "$lines lines over budget $budget" "majordomus context"; fi
  else mj_fail context "builder" "majordomus context failed" "majordomus context"; fi
  rm -f "$out"
}
