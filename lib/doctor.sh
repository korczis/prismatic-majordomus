#!/usr/bin/env bash
# shellcheck disable=SC2034  # MJ_DOCTRINE_SKIPPED is read by the dispatcher in doctrine.sh
# sourced by watch as well as run directly; guard against re-sourcing
[ -n "${MJ_LIB_doctor:-}" ] && return 0 || MJ_LIB_doctor=1
# doctor — is Majordomus itself healthy and actually wired here? Read-only.
#
# doctor runs no check by name: it dispatches the doctrines whose enforced_by names
# doctor, and one of those — doctrine_wiring_integrity — verifies the whole chain,
# including this dispatch.
# shellcheck source=doctrine.sh
. "$MJ_LIB_DIR/doctrine.sh"
# doctor enforces doctrines whose validators live in check.sh — the store rules it shares
# with check. Sourcing it is the dependency being honest rather than a second copy.
# shellcheck source=check.sh
. "$MJ_LIB_DIR/check.sh"
# the continuity validators read these stores through their own commands' helpers
# shellcheck source=question.sh
. "$MJ_LIB_DIR/question.sh"
# shellcheck source=decision.sh
. "$MJ_LIB_DIR/decision.sh"
# shellcheck source=prompt.sh
. "$MJ_LIB_DIR/prompt.sh"
# shellcheck source=context.sh
. "$MJ_LIB_DIR/context.sh"
# the project-model validators read the one engine, not a copy of its rules
# shellcheck source=project.sh
. "$MJ_LIB_DIR/project.sh"

MJ_DOCTOR_MISSING=0
mj_cmd_doctor() {
  local a
  for a in "$@"; do case "$a" in
    --help|-h) echo "usage: majordomus doctor [--json]   (read-only; exit 0 healthy, 10 failures, 12 missing)"; return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "doctor: unknown option $a" ;;
  esac; done
  mj_require_installed
  MJ_DOCTOR_MISSING=0

  # The policy has to parse before anything can be judged against it. This is the one
  # ordering doctor imposes; everything after it is the registry's order.
  if ! mj_load_policy 2>/dev/null; then
    mj_fail policy ".majordomus/policy.yaml" "does not parse: $(mj_yaml_flatten "$MJ_DIR/policy.yaml" 2>&1 >/dev/null | sed 's/^ERROR://')" "majordomus doctor"
    mj_finish_doctor; return
  fi

  mj_doctrine_dispatch doctor
  mj_report_environment
  mj_finish_doctor
}

# ---------------------------------------------------------------- validators
mj_validate_policy() {
  local unk
  [ "$MJ_DOCTRINE_CMD" = watch ] && { mj_watch_policy; return 0; }
  if [ "$(mj_pol version)" = 1 ]; then
    unk="$(mj_yaml_unknown_keys "$MJ_POL_FLAT" "$MJ_BIN_DIR/../share/allow/policy.txt" || true)"
      if [ -z "$unk" ]; then mj_doctrine_ok policy ".majordomus/policy.yaml" "parsed, version 1"
      else mj_doctrine_fail policy ".majordomus/policy.yaml" "unknown keys: $(printf '%s' "$unk" | tr '\n' ' ')" "grep -nE '$(printf '%s' "$unk" | sed 's/\..*//' | sort -u | tr '\n' '|' | sed 's/|$//')' .majordomus/policy.yaml"; fi
  else mj_doctrine_fail policy ".majordomus/policy.yaml" "unsupported version '$(mj_pol version)' (want 1)"; fi

  local pf n count=0
  for pf in "$MJ_DIR"/profiles/*.yaml; do
    [ -f "$pf" ] || continue; count=$((count+1)); n="$(basename "$pf" .yaml)"
    if mj_load_profile "$n" 2>/dev/null; then
      unk="$(mj_yaml_unknown_keys "$MJ_PRO_FLAT" "$MJ_BIN_DIR/../share/allow/profile.txt" || true)"
      [ -n "$unk" ] && mj_doctrine_fail profiles "$n" "unknown keys: $(printf '%s' "$unk" | tr '\n' ' ')" "cat .majordomus/profiles/$n.yaml"
      [ "$(mj_pro name)" = "$n" ] || mj_doctrine_fail profiles "$n" "name field '$(mj_pro name)' does not match filename"
    else mj_doctrine_fail profiles "$n" "does not parse" "cat .majordomus/profiles/$n.yaml"; fi
  done
  local def; def="$(mj_pol profiles.default)"
  if [ -f "$MJ_DIR/profiles/$def.yaml" ]; then mj_doctrine_ok profiles "$count files" "parsed; default '$def' exists"
  else mj_doctrine_fail profiles "default" "profiles.default='$def' has no file .majordomus/profiles/$def.yaml"; fi

  return 0
}

mj_validate_wiring() {
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
      mj_doctrine_fail wiring "$name" "'$path' exists but is not executable" "chmod +x $path"
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
          if [ ! -f "$hookfile" ]; then mj_doctrine_fail wiring "$name" "hook $hookdir/$target does not exist" "ls -l $hookdir/$target"
          elif [ ! -x "$hookfile" ]; then mj_doctrine_fail wiring "$name" "hook $hookdir/$target is not executable" "chmod +x $hookdir/$target"
          elif [ -z "$wirefile" ]; then
            mj_doctrine_fail wiring "$name" "$(basename "$path") $arg0 is not invoked by $hookdir/$target or anything in $hookdir/$target.d/" "grep -rn 'majordomus $arg0' $hookdir/$target $hookdir/$target.d 2>/dev/null"
          elif [ ! -x "$wirefile" ]; then
            mj_doctrine_fail wiring "$name" "$rel invokes it but is not executable, so the dispatcher skips it" "chmod +x $rel"
          elif [ -z "$resolved" ] && ! mj_hook_binary_ok "$wirefile" "$arg0"; then
            mj_doctrine_fail wiring "$name" "'$path' is not on PATH and $rel does not name an executable majordomus" "grep -n 'majordomus $arg0' $rel"
          elif grep -E "majordomus[[:space:]]+$arg0" "$wirefile" | grep -qE '\|\|[[:space:]]*(true|exit[[:space:]]+0)'; then
            mj_doctrine_fail wiring "$name" "$rel invokes it but swallows the exit code (|| true)" "grep -n 'majordomus $arg0' $rel"
          else mj_doctrine_ok wiring "$name" "wired via $rel"; fi ;;
        ci)
          if [ ! -f "$MJ_ROOT/$target" ]; then mj_doctrine_fail wiring "$name" "ci file $target does not exist"
          elif ! grep -qE "majordomus[[:space:]]+$arg0" "$MJ_ROOT/$target"; then mj_doctrine_fail wiring "$name" "$target does not invoke majordomus $arg0" "grep -n majordomus $target"
          else mj_doctrine_ok wiring "$name" "invoked from $target"; fi ;;
        manual) mj_doctrine_skip wiring "$name" "wired_by: manual — documented, not verified" ;;
        *) mj_doctrine_fail wiring "$name" "unknown wired_by kind '$kind' (git-hook:<name> | ci:<path> | manual)" ;;
      esac
    fi
    i=$((i+1))
  done
  [ "$i" = 0 ] && mj_doctrine_fail wiring "policy" "no enforcement entries declared; nothing is wired to run majordomus"

  return 0
}

MJ_DOCTOR_ALWAYS=""; MJ_DOCTOR_ALWAYS_MODE="file"
mj_validate_projection() {
  [ "$MJ_DOCTRINE_CMD" = watch ] && { mj_watch_projection; return 0; }
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
      *) mj_doctrine_fail projection "$tgt" "unknown mode '$mode' (file | region)" "grep -n 'mode:' .majordomus/policy.yaml"; j=$((j+1)); continue ;;
    esac
    # update would die on this; doctor is the command that is supposed to say so first
    [ -f "$MJ_DIR/providers/$prov.tmpl" ] || { mj_doctrine_fail projection "$tgt" "provider '$prov' has no template .majordomus/providers/$prov.tmpl" "ls .majordomus/providers/"; MJ_DOCTOR_MISSING=1; j=$((j+1)); continue; }
    if [ ! -f "$MJ_ROOT/$tgt" ]; then mj_doctrine_fail projection "$tgt" "missing (run: majordomus update)" "majordomus update"; MJ_DOCTOR_MISSING=1
    elif [ -z "$fpflat" ]; then mj_doctrine_fail projection "$tgt" "no fingerprints recorded (run: majordomus update)" "majordomus update"; MJ_DOCTOR_MISSING=1
    else
      want="$(mj_fp_sha "$fpflat" "$tgt")"; have=""
      if [ "$mode" = region ]; then
        rc=0; mj_region_extract "$MJ_ROOT/$tgt" > "$owned" 2>/dev/null || rc=$?
        case "$rc" in
          0) have="$(mj_sha256 "$owned")" ;;
          1) mj_doctrine_fail projection "$tgt" "region markers are absent (run: majordomus update)" "majordomus update"; MJ_DOCTOR_MISSING=1 ;;
          *) mj_doctrine_fail projection "$tgt" "region markers are malformed (unclosed, out of order, or repeated)" "grep -n 'majordomus:begin\\|majordomus:end' $tgt" ;;
        esac
      else cp "$MJ_ROOT/$tgt" "$owned"; have="$(mj_sha256 "$owned")"; fi
      if [ -n "$have" ]; then
        if [ -z "$want" ]; then mj_doctrine_fail projection "$tgt" "not in fingerprints (run: majordomus update)" "majordomus update"; MJ_DOCTOR_MISSING=1
        elif [ "$want" != "$have" ]; then mj_doctrine_fail projection "$tgt" "hash differs from fingerprint (hand-edited?)" "majordomus update --diff $tgt"
        else mj_doctrine_ok projection "$tgt" "fingerprint matches$([ "$mode" = region ] && printf ' (region)')"; fi
      fi
    fi
    j=$((j+1))
  done
  [ -n "$fpflat" ] && rm -f "$fpflat"

  MJ_DOCTOR_ALWAYS="$always"; MJ_DOCTOR_ALWAYS_MODE="$always_mode"
  return 0
}

mj_validate_budget() {
  # Two budgets, one rule: the context a worker is given must fit what was budgeted for
  # it. doctor measures the always-loaded projection and the builder; watch measures only
  # the builder, because the projection's size is drift the projection doctrine reports.
  [ "$MJ_DOCTRINE_CMD" = watch ] && { mj_context_builder_check; return 0; }
  local owned rc always="$MJ_DOCTOR_ALWAYS" always_mode="$MJ_DOCTOR_ALWAYS_MODE" budget
  owned="$(mktemp "${TMPDIR:-/tmp}/mj.own.XXXXXX")"
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
    if [ "$l" -le "$budget" ]; then mj_doctrine_ok budget "$subject" "$l lines, budget $budget"
    else mj_doctrine_fail budget "$subject" "$l lines, budget $budget" "majordomus update --dry-run"; fi
    # 6. links resolve
    local bad=0 l2 dir; dir="$(dirname "$MJ_ROOT/$always")"
    for l2 in $(grep -oE '\]\(([^)#]+)\)' "$measured" | sed -E 's/\]\(([^)]+)\)/\1/' | grep -vE '^https?://' || true); do
      [ -e "$dir/$l2" ] || { mj_doctrine_fail links "$subject" "reference $l2 does not resolve" "ls $l2"; bad=1; }
    done
    [ "$bad" = 0 ] && mj_doctrine_ok links "$subject" "all references resolve"
    # 7. no hardcoded counts
    if grep -qE '\b[0-9]+ (agents|files|apps|commands|skills|rules)\b' "$measured"; then
      mj_doctrine_fail counts "$subject" "hardcoded count in always-loaded context" "grep -nE '[0-9]+ (agents|files|apps|commands|skills|rules)' $always"
    else mj_doctrine_ok counts "$subject" "no hardcoded counts"; fi
  elif [ -z "$always" ]; then mj_doctrine_fail budget "policy" "no projection is marked always_loaded: true"; fi
  rm -f "$owned"

  # 8. the continuity subsystem: reachable through the CLI, not merely present on disk.
  #    A directory full of records that no command reads is the failure mode this whole
  #    tool exists to catch, so doctor proves each store is readable by the command that
  #    owns it rather than checking that the files exist.
  mj_context_builder_check
  return 0
}

mj_validate_retention() {
  local cap ll hc
  cap="$(mj_pol_req ledger.retention_max_lines)"; ll=0; [ -f "$MJ_DIR/state/ledger.jsonl" ] && ll="$(mj_lines "$MJ_DIR/state/ledger.jsonl")"
  if [ "$ll" -le "$cap" ]; then mj_doctrine_ok retention "ledger" "$ll lines, cap $cap"; else mj_doctrine_fail retention "ledger" "$ll lines over cap $cap" "wc -l .majordomus/state/ledger.jsonl"; fi
  cap="$(mj_pol_req handover.retention_max_files)"; hc="$(find "$MJ_DIR/state/handovers" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$hc" -le "$cap" ]; then mj_doctrine_ok retention "handovers" "$hc files, cap $cap"; else mj_doctrine_fail retention "handovers" "$hc files over cap $cap" "majordomus handover --list | wc -l"; fi
  cap="$(mj_pol_req checkpoint.retention_max_files)"; hc="$(find "$MJ_DIR/state/checkpoints" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$hc" -le "$cap" ]; then mj_doctrine_ok retention "checkpoints" "$hc files, cap $cap"; else mj_doctrine_fail retention "checkpoints" "$hc files over cap $cap" "majordomus checkpoint --list | wc -l"; fi

  return 0
}

mj_report_environment() {
  local env="bash ${BASH_VERSION%%(*}"; env="$env, git $(git --version | awk '{print $3}')"
  mj_has jq && env="$env, jq $(jq --version 2>/dev/null | sed 's/jq-//')" || env="$env, jq absent"
  mj_has shellcheck && env="$env, shellcheck present"
  mj_info env "-" "$env"
  return 0
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
  [ "$MJ_JSON" = 1 ] || printf 'doctor: %s failure(s)\n' "$MJ_FAILS"
  if [ "$MJ_DOCTOR_MISSING" = 1 ]; then exit "$MJ_EX_MISSING"; fi
  [ "$MJ_FAILS" = 0 ] && exit "$MJ_EX_OK" || exit "$MJ_EX_CONTRACT"
}

# Prove the continuity subsystem is wired, not merely installed. Each check runs the real
# code path a worker would run; a store that cannot be read is a failure, because a record
# nothing reads is indistinguishable from no record at all.
# ---------------------------------------------------------------- continuity validators
# These were one inline mj_doctor_continuity block. Each is now a declared doctrine, so
# the class decides the level and watch gets the drift view for free rather than by
# a second copy of the same logic.

# Every literal policy key the code reads exists in the skeleton policy, and no reader
# carries its own default for one. A default written beside a reader is a second source of
# truth for the same number: nothing keeps the two in step, and a reader that substitutes
# its own value enforces something the configuration does not say.
#
# The key list is derived from the source, not written here, so a new mj_pol_req call is
# covered the moment it is written.
mj_validate_policy_defaults() {
  local skel="$MJ_BIN_DIR/../share/skeleton/policy.yaml" k last bad=0 n=0 flat
  [ -f "$skel" ] || { mj_doctrine_skip policy "skeleton" "no skeleton policy to compare against"; MJ_DOCTRINE_SKIPPED=1; return 0; }
  flat="$(mktemp "${TMPDIR:-/tmp}/mj.sk.XXXXXX")"
  mj_yaml_flatten "$skel" > "$flat" 2>/dev/null || { rm -f "$flat"; mj_doctrine_fail policy "skeleton" "share/skeleton/policy.yaml does not parse" "majordomus doctor"; return 0; }
  for k in $(grep -rhE 'mj_pol(_req)? +[a-z_]+(\.[a-z_]+)*' "$MJ_BIN_DIR/../lib" | grep -v '^[[:space:]]*#' \
             | grep -oE 'mj_pol(_req)? +[a-z_]+(\.[a-z_]+)*' | sed -E 's/mj_pol(_req)? +//' | sort -u); do
    n=$((n + 1))
    if [ -z "$(mj_yget "$flat" "$k")" ] && ! grep -qE "^${k}\." "$flat"; then
      mj_doctrine_fail policy "$k" "read by lib/ but absent from share/skeleton/policy.yaml" "grep -rn 'mj_pol_req $k' lib/"; bad=1
    fi
  done
  rm -f "$flat"
  # a reader that supplies its own default is the drift this check exists to prevent
  for k in $(grep -rlnE 'mj_pol +[a-z_.]+\)"; \[ -n|:-[0-9]+\}' "$MJ_BIN_DIR/../lib" 2>/dev/null | grep -v common.sh || true); do
    last="$(grep -nE 'mj_pol +[a-z_.]+\)"; \[ -n' "$k" | head -1 | cut -d: -f1)"
    [ -n "$last" ] && { mj_doctrine_fail policy "$(basename "$k"):$last" "a policy value is read with a default written beside it; use mj_pol_req" "grep -n 'mj_pol ' $k"; bad=1; }
  done
  [ "$bad" = 0 ] && mj_doctrine_ok policy "$n key(s)" "every policy value the code reads is declared, with no reader-side default"
  return 0
}

mj_validate_layout() {
  local d
  for d in state/handovers state/checkpoints prompts; do
    if [ -d "$MJ_DIR/$d" ]; then mj_doctrine_ok layout ".majordomus/$d" "present"
    else mj_doctrine_fail layout ".majordomus/$d" "missing; the command that writes it will create it, but update installs it" "majordomus update"; fi
  done
  return 0
}

# The questions store is read by the blocker gate. It is validated here as well as in
# check because doctor answers a different question — is the installation sound — and a
# repository with no active task never reaches the check path at all.
mj_validate_questions_store() {
  local bad; bad="$(mj_question_malformed "$MJ_DIR/state/open-questions.md")"
  if [ -n "$bad" ]; then mj_doctrine_fail records "open-questions.md" "line(s) $(printf '%s' "$bad" | sed 's/ $//') do not parse; an unreadable entry cannot block acceptance" "majordomus question list --all"
  else mj_doctrine_ok records "open-questions.md" "every entry parses"; fi
  return 0
}

mj_validate_prompts() {
  local f n=0 reason bad=0
  [ -d "$MJ_DIR/prompts" ] || return 0
  [ "$MJ_DOCTRINE_CMD" = watch ] && mj_watch_prompts_empty
  for f in "$MJ_DIR"/prompts/*.md; do
    [ -f "$f" ] || continue; n=$((n + 1))
    reason="$(mj_prompt_validate "$f")" || { mj_doctrine_fail prompt "$(basename "$f" .md)" "$(printf '%s' "$reason" | tr '\n' ';' | sed 's/;$//')" "majordomus prompt show $(basename "$f" .md)"; bad=1; }
  done
  [ "$n" -gt 0 ] && [ "$bad" = 0 ] && mj_doctrine_ok prompt "$n asset(s)" "front matter valid, every token known"
  return 0
}

# The resolver running and reporting a clean absence is as healthy as it finding a
# record; only a malformed record is a finding.
mj_validate_resolver() {
  [ "$MJ_DOCTRINE_CMD" = watch ] && { mj_watch_resolver; return 0; }
  if mj_resolve_latest "$MJ_DIR/state/handovers" ""; then
    mj_doctrine_ok resolver "handovers" "$MJ_RES_MATCH, $(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH")" "majordomus handover --resolve"
  else mj_doctrine_ok resolver "handovers" "no record for this worktree and branch (absence, not a stale match)" "majordomus handover --resolve"; fi
  [ "$MJ_RES_SKIPPED" -gt 0 ] && mj_doctrine_fail resolver "handovers" "$MJ_RES_SKIPPED record(s) skipped as malformed" "majordomus handover --list"
  return 0
}

# Part of context_budget: the same rule about the same resource, measured through the
# real command path rather than by re-implementing the builder here.
mj_context_builder_check() {
  local budget out lines
  budget="$(mj_pol_req context.builder_budget_lines)"
  out="$(mktemp "${TMPDIR:-/tmp}/mj.dc.XXXXXX")"
  if ( export MJ_JSON=0; mj_cmd_context ) > "$out" 2>/dev/null; then
    lines="$(mj_lines "$out")"
    if [ "$lines" -le "$budget" ]; then mj_doctrine_ok context "builder" "$lines lines, budget $budget" "majordomus context"
    else mj_doctrine_fail context "builder" "$lines lines over budget $budget" "majordomus context"; fi
  else mj_doctrine_fail context "builder" "majordomus context failed" "majordomus context"; fi
  rm -f "$out"
  return 0
}
# ---------------------------------------------------------------- doctrine wiring
# The imported idea this whole layer exists for: a rule is not enforced because it is
# written down. Every link below is read out of the source, never out of the registry's
# own prose, so a registry that lies about itself fails here.
#
#   declared -> validator exists -> the command dispatches -> failure propagates
#            -> a test proves it -> CI runs the test
#
# It also runs the other direction: a mj_validate_* function that no doctrine declares
# is an orphan validator, which is how a check quietly stops being governed.
mj_validate_doctrine_wiring() {
  local lib="$MJ_BIN_DIR/../lib" root="$MJ_BIN_DIR/.."
  local i=0 id val cls fn cmd t bad=0 n=0

  # 1. every declared doctrine resolves, end to end
  while [ -n "$(mj_doc "$i" id)" ]; do
    n=$((n+1)); id="$(mj_doc "$i" id)"; val="$(mj_doc "$i" validator)"; cls="$(mj_doc "$i" class)"; fn="mj_validate_$val"
    case "$cls" in blocking|advisory) ;; *) mj_doctrine_fail doctrine "$id" "class '$cls' is neither blocking nor advisory" "grep -n 'id: $id' share/doctrines.yaml"; bad=1 ;; esac
    if ! grep -rqE "^$fn\(\)" "$lib"; then
      mj_doctrine_fail doctrine "$id" "validator function $fn is defined nowhere in lib/" "grep -rn '$fn' lib/"; bad=1
    fi
    # the commands it claims to run under must actually dispatch
    for cmd in $(mj_doc_list "$i" enforced_by); do
      if [ ! -f "$lib/$cmd.sh" ]; then
        mj_doctrine_fail doctrine "$id" "enforced_by names '$cmd', which is not a command (lib/$cmd.sh)" "ls lib/"; bad=1
      elif ! grep -q 'mj_doctrine_dispatch' "$lib/$cmd.sh"; then
        mj_doctrine_fail doctrine "$id" "declared for $cmd but lib/$cmd.sh never calls mj_doctrine_dispatch" "grep -n mj_doctrine_dispatch lib/$cmd.sh"; bad=1
      fi
    done
    # a blocking doctrine must be able to stop its command
    if [ "$cls" = blocking ]; then
      for cmd in $(mj_doc_list "$i" enforced_by); do
        [ -f "$lib/$cmd.sh" ] || continue
        grep -qE 'MJ_FAILS.*exit|exit .*MJ_EX_CONTRACT|MJ_DOCTOR_MISSING' "$lib/$cmd.sh" \
          || { mj_doctrine_fail doctrine "$id" "blocking, but lib/$cmd.sh never turns a failing finding into a non-zero exit" "grep -n 'MJ_FAILS' lib/$cmd.sh"; bad=1; }
      done
    fi
    # the test that proves it must exist and must name it
    t="$(mj_doc "$i" test)"
    if [ -z "$t" ]; then mj_doctrine_fail doctrine "$id" "declares no test" "grep -n 'id: $id' share/doctrines.yaml"; bad=1
    elif [ ! -f "$root/$t" ]; then mj_doctrine_fail doctrine "$id" "test $t does not exist" "ls $t"; bad=1; fi
    # every claim it carries must be a real claim
    local c
    for c in $(mj_doc_list "$i" claims); do
      grep -q "^  - id: $c$" "$root/docs/CLAIMS.yaml" 2>/dev/null \
        || { mj_doctrine_fail doctrine "$id" "names claim '$c', which is not in docs/CLAIMS.yaml" "grep -n 'id: $c' docs/CLAIMS.yaml"; bad=1; }
    done
    i=$((i+1))
  done

  # 2. the other direction — a validator no doctrine declares
  local f declared=" "
  i=0; while [ -n "$(mj_doc "$i" id)" ]; do declared="$declared$(mj_doc "$i" validator) "; i=$((i+1)); done
  for f in $(grep -rhoE '^mj_validate_[a-z_]+\(\)' "$lib" | sed -e 's/^mj_validate_//' -e 's/()//' | sort -u); do
    case "$declared" in *" $f "*) ;; *) mj_doctrine_fail doctrine "mj_validate_$f" "validator exists but no doctrine declares it; it runs under no rule" "grep -rn 'mj_validate_$f' lib/ share/doctrines.yaml"; bad=1 ;; esac
  done

  # 3. CI must run the test runner, without swallowing it
  local ci="$root/.github/workflows/validate.yml"
  if [ ! -f "$ci" ]; then mj_doctrine_fail doctrine "ci" "no .github/workflows/validate.yml; nothing runs the doctrine tests on integration" "ls .github/workflows/"; bad=1
  elif ! grep -qE 'bash test/run\.sh' "$ci"; then mj_doctrine_fail doctrine "ci" "validate.yml does not run test/run.sh" "grep -n 'test/run.sh' .github/workflows/validate.yml"; bad=1
  elif grep -E 'bash test/run\.sh' "$ci" | grep -qE '\|\|[[:space:]]*(true|:)|continue-on-error'; then
    mj_doctrine_fail doctrine "ci" "validate.yml runs test/run.sh but does not let it fail the job" "grep -n -A2 'test/run.sh' .github/workflows/validate.yml"; bad=1
  fi
  # and the runner must run every case, not a list that a new case can miss
  if ! grep -qE 'cases/\*\.sh|cases/\*' "$root/test/run.sh"; then
    mj_doctrine_fail doctrine "runner" "test/run.sh does not glob test/cases/; a new case would not run" "grep -n cases test/run.sh"; bad=1
  fi

  [ "$bad" = 0 ] && mj_doctrine_ok doctrine "$n doctrines" "validator, dispatch, propagation, test and CI resolve for every one"
  return 0
}
# watch's view of the same doctrine: the fingerprints are the record of what Majordomus
# last generated, so drift is measured against them rather than against the policy.
mj_watch_projection() {
  local fp="$MJ_DIR/generated/fingerprints.yaml" fpflat="" owned k=0 tgt mode rc
  [ -f "$fp" ] || return 0   # policy validator already reported "no projections generated yet"
  fpflat="$(mktemp "${TMPDIR:-/tmp}/mj.wf.XXXXXX")"
  mj_yaml_flatten "$fp" > "$fpflat" 2>/dev/null || { rm -f "$fpflat"; return 0; }
  owned="$(mktemp "${TMPDIR:-/tmp}/mj.wo.XXXXXX")"
  while [ -n "$(mj_yget "$fpflat" "targets.$k.target")" ]; do
    tgt="$(mj_yget "$fpflat" "targets.$k.target")"
    mode="$(mj_yget "$fpflat" "targets.$k.mode")"; [ -n "$mode" ] || mode="file"
    if [ ! -f "$MJ_ROOT/$tgt" ]; then mj_doctrine_fail projection "$tgt" "missing" "majordomus update"
    elif [ "$mode" = region ]; then
      rc=0; mj_region_extract "$MJ_ROOT/$tgt" > "$owned" 2>/dev/null || rc=$?
      if [ "$rc" = 1 ]; then mj_doctrine_fail projection "$tgt" "region markers are absent" "majordomus update"
      elif [ "$rc" != 0 ]; then mj_doctrine_fail projection "$tgt" "region markers are malformed" "grep -n 'majordomus:begin' $tgt"
      elif [ "$(mj_sha256 "$owned")" != "$(mj_yget "$fpflat" "targets.$k.sha256")" ]; then mj_doctrine_fail projection "$tgt" "region differs from fingerprint (hand-edited?)" "majordomus update --diff $tgt"
      else mj_doctrine_ok projection "$tgt" "region matches fingerprint"; fi
    elif [ "$(mj_sha256 "$MJ_ROOT/$tgt")" != "$(mj_yget "$fpflat" "targets.$k.sha256")" ]; then mj_doctrine_fail projection "$tgt" "hash differs from fingerprint (hand-edited?)" "majordomus update --diff $tgt"
    else mj_doctrine_ok projection "$tgt" "matches fingerprint"; fi
    k=$((k+1))
  done
  rm -f "$fpflat" "$owned"
}

# watch asks whether the policy has moved since the projections were generated; doctor
# asks whether it is valid at all. Same doctrine, two questions.
mj_watch_policy() {
  local tmp psha fp="$MJ_DIR/generated/fingerprints.yaml" fpflat=""
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.w.XXXXXX")"; mj_policy_cat > "$tmp"; psha="$(mj_sha256 "$tmp")"; rm -f "$tmp"
  if [ -f "$fp" ]; then fpflat="$(mktemp "${TMPDIR:-/tmp}/mj.wf.XXXXXX")"; mj_yaml_flatten "$fp" > "$fpflat" 2>/dev/null || { rm -f "$fpflat"; fpflat=""; }; fi
  if [ -z "$fpflat" ]; then mj_doctrine_fail policy "fingerprints" "no projections generated yet" "majordomus update"
  elif [ "$(mj_yget "$fpflat" policy_sha256)" = "$psha" ]; then mj_doctrine_ok policy "policy+profiles" "match last update (${psha:0:12})"
  else mj_doctrine_fail policy ".majordomus/policy.yaml" "policy or profiles changed after the last update" "majordomus update --dry-run"; fi
  [ -n "$fpflat" ] && rm -f "$fpflat"
  return 0
}

# ---------------------------------------------------------------- project model validators
# A repository is not obliged to have a canonical project model; most installations will
# not. Both validators therefore skip cleanly when .majordomus/project/ is absent, so
# adopting Majordomus does not turn an installation red for a feature nobody opted into.
mj_project_doctrine_load() {
  mj_project_present || { MJ_DOCTRINE_SKIPPED=1; mj_doctrine_skip project ".majordomus/project" "no canonical project model here; nothing to validate"; return 1; }
  local rc=0; mj_project_load || rc=$?
  [ "$rc" = 0 ] && return 0
  mj_doctrine_fail project ".majordomus/project" "the canonical model does not load; a file does not parse or two records claim one id" "majordomus plan validate"
  return 1
}

mj_validate_project() {
  local unk rec keys
  mj_project_doctrine_load || return 0
  unk="$(mj_project_unknown_keys || true)"
  if [ -n "$unk" ]; then
    # read from a redirect rather than a pipe: a validator that reports its violations
    # inside a subshell raises nothing, and the command it runs in would exit 0
    while read -r rec keys; do
      mj_doctrine_fail project "$rec" "unknown keys: $keys" "majordomus plan validate"
    done < <(printf '%s\n' "$unk")
    return 0
  fi
  mj_doctrine_ok project ".majordomus/project" "$(mj_pj_milestone_ids | wc -l | tr -d ' ') milestone(s), $(mj_pj_issue_ids | wc -l | tr -d ' ') issue(s), every key read by something"
  return 0
}

# The graph rules, reported through the dispatcher so that watch sees the same violations
# as drift. A warning from the model is a warning here: work in progress is reported, not
# blocked, and only a graph that cannot be executed is a failure.
mj_validate_dag() {
  local lvl subj msg n
  mj_project_doctrine_load || return 0
  mj_pj_findings | while IFS="$(printf '\t')" read -r _ lvl _ subj msg; do
    case "$lvl" in
      FAIL) mj_doctrine_fail dag "$subj" "$msg" "majordomus plan validate" ;;
      *)    mj_warn dag "$subj" "$msg" "majordomus plan show $subj" ;;
    esac
  done
  n="$(mj_pj_fail_count)"
  [ "$n" = 0 ] && mj_doctrine_ok dag "$(mj_pj_rows W | wc -l | tr -d ' ') wave(s)" "acyclic, every edge resolves, nothing running ahead of a dependency"
  # The findings above were printed in a subshell, so the failure has to be raised here.
  [ "$n" = 0 ] || mj_doctrine_fail dag "graph" "$n dependency-graph failure(s)" "majordomus plan validate"
  return 0
}
