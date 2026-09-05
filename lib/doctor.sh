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
# the command-surface doctrines: their validators read share/, not this repository's state
# shellcheck source=commands.sh
. "$MJ_LIB_DIR/commands.sh"
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
  local t_start; t_start="$(mj_ms)"

  # The policy has to parse before anything can be judged against it. This is the one
  # ordering doctor imposes; everything after it is the registry's order.
  if ! mj_load_policy 2>/dev/null; then
    mj_fail policy "$(mj_rel "$MJ_POLICY_FILE")" "does not parse: $(mj_yaml_flatten "$MJ_POLICY_FILE" 2>&1 >/dev/null | sed 's/^ERROR://')" "majordomus doctor"
    mj_finish_doctor; return
  fi

  mj_doctrine_dispatch doctor
  mj_report_environment
  mj_report_budget doctor "$t_start"
  mj_finish_doctor
}

# The command's own wall time against the policy's budget for it: WARN over budget, INFO
# under, never the exit code. The budget is measured, not guessed, and changing it is a
# policy edit with a run behind it (majordomus bench doctor).
mj_report_budget() { # command, start ms
  local elapsed budget; elapsed=$(( $(mj_ms) - $2 )); budget="$(mj_pol_req "benchmark.budget.${1}_ms")"
  if [ "$elapsed" -gt "$budget" ]; then mj_warn budget "$1" "$elapsed ms, over the budget of $budget ms (policy benchmark.budget.${1}_ms)" "MJ_TIMING=1 majordomus $1"
  else mj_info budget "$1" "$elapsed ms of $budget ms" "MJ_TIMING=1 majordomus $1"; fi
}

# ---------------------------------------------------------------- validators
mj_validate_policy() {
  local unk
  [ "$MJ_DOCTRINE_CMD" = watch ] && { mj_watch_policy; return 0; }
  if [ "$(mj_pol version)" = 1 ]; then
    unk="$(mj_yaml_unknown_keys "$MJ_POL_FLAT" "$MJ_ALLOW_DIR/policy.txt" || true)"
      if [ -z "$unk" ]; then mj_doctrine_ok policy "$(mj_rel "$MJ_POLICY_FILE")" "parsed, version 1"
      else mj_doctrine_fail policy "$(mj_rel "$MJ_POLICY_FILE")" "unknown keys: $(printf '%s' "$unk" | tr '\n' ' ')" "grep -nE '$(printf '%s' "$unk" | sed 's/\..*//' | sort -u | tr '\n' '|' | sed 's/|$//')' $(mj_rel "$MJ_POLICY_FILE")"; fi
  else mj_doctrine_fail policy "$(mj_rel "$MJ_POLICY_FILE")" "unsupported version '$(mj_pol version)' (want 1)"; fi

  local pf n count=0
  for pf in "$MJ_PROFILES_DIR"/*.yaml; do
    [ -f "$pf" ] || continue; count=$((count+1)); n="$(basename "$pf" .yaml)"
    if mj_load_profile "$n" 2>/dev/null; then
      unk="$(mj_yaml_unknown_keys "$MJ_PRO_FLAT" "$MJ_ALLOW_DIR/profile.txt" || true)"
      [ -n "$unk" ] && mj_doctrine_fail profiles "$n" "unknown keys: $(printf '%s' "$unk" | tr '\n' ' ')" "cat $(mj_rel "$MJ_PROFILES_DIR")/$n.yaml"
      [ "$(mj_pro name)" = "$n" ] || mj_doctrine_fail profiles "$n" "name field '$(mj_pro name)' does not match filename"
    else mj_doctrine_fail profiles "$n" "does not parse" "cat $(mj_rel "$MJ_PROFILES_DIR")/$n.yaml"; fi
  done
  local def; def="$(mj_pol profiles.default)"
  if [ -f "$MJ_PROFILES_DIR/$def.yaml" ]; then mj_doctrine_ok profiles "$count files" "parsed; default '$def' exists"
  else mj_doctrine_fail profiles "default" "profiles.default='$def' has no file $(mj_rel "$MJ_PROFILES_DIR")/$def.yaml"; fi

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
# The state of one projection target against its own stamp. Prints one word:
#   absent · no_template · region_absent · malformed · unstamped · hand_edited · ok
# followed, when the target carries a stamp, by the policy hash the stamp names.
mj_validate_projection() {
  [ "$MJ_DOCTRINE_CMD" = watch ] && { mj_watch_projection; return 0; }
  local j=0 tgt mode prov st always="" always_mode="file"
  while [ -n "$(mj_pol "projections.$j.target")" ]; do
    tgt="$(mj_pol "projections.$j.target")"; mode="$(mj_projection_mode "$j")"
    prov="$(mj_pol "projections.$j.provider")"
    if [ "$(mj_pol "projections.$j.always_loaded")" = true ]; then always="$tgt"; always_mode="$mode"; fi
    case "$mode" in
      file|region) ;;
      *) mj_doctrine_fail projection "$tgt" "unknown mode '$mode' (file | region)" "grep -n 'mode:' $(mj_rel "$MJ_POLICY_FILE")"; j=$((j+1)); continue ;;
    esac
    st="$(mj_projection_status "$tgt" "$mode" "$prov")"; st="${st%% *}"
    case "$st" in
      # update would die on this; doctor is the command that is supposed to say so first
      no_template) mj_doctrine_fail projection "$tgt" "provider '$prov' has no template $(mj_rel "$MJ_PROVIDERS_DIR")/$prov.tmpl, and the distribution ships none" "ls $(mj_rel "$MJ_PROVIDERS_DIR")/ $MJ_PROVIDERS_DEFAULT_DIR/"; MJ_DOCTOR_MISSING=1 ;;
      absent) mj_doctrine_fail projection "$tgt" "missing (run: majordomus update)" "majordomus update"; MJ_DOCTOR_MISSING=1 ;;
      region_absent) mj_doctrine_fail projection "$tgt" "region markers are absent (run: majordomus update)" "majordomus update"; MJ_DOCTOR_MISSING=1 ;;
      malformed) mj_doctrine_fail projection "$tgt" "region markers are malformed (unclosed, out of order, or repeated)" "grep -n 'majordomus:begin\\|majordomus:end' $tgt" ;;
      unstamped) mj_doctrine_fail projection "$tgt" "carries no generation stamp; not written by update (run: majordomus update)" "majordomus update --diff $tgt"; MJ_DOCTOR_MISSING=1 ;;
      hand_edited) mj_doctrine_fail projection "$tgt" "content differs from its own stamp (hand-edited?)" "majordomus update --diff $tgt" ;;
      ok) mj_doctrine_ok projection "$tgt" "content matches its stamp$([ "$mode" = region ] && printf ' (region)')" ;;
    esac
    j=$((j+1))
  done

  MJ_DOCTOR_ALWAYS="$always"; MJ_DOCTOR_ALWAYS_MODE="$always_mode"
  return 0
}

# The chain from a person to the layer: README.md names AGENTS.md, every generated target
# names .ai/README.md, and no generated target carries what the layer is for. A rule that
# exists in one provider's file and nowhere else is the two-rulebooks failure this tool was
# distilled from, so a generated target that holds a rule corpus fails here.
mj_validate_bootstrap() {
  local j=0 tgt bad=0
  if [ -f "$MJ_ROOT/README.md" ]; then
    if grep -q 'AGENTS\.md' "$MJ_ROOT/README.md"; then mj_doctrine_ok bootstrap "README.md" "names AGENTS.md"
    else mj_doctrine_fail bootstrap "README.md" "does not name AGENTS.md; a reader cannot find the agent bootstrap" "grep -n AGENTS.md README.md"; bad=1; fi
  fi
  while [ -n "$(mj_pol "projections.$j.target")" ]; do
    tgt="$(mj_pol "projections.$j.target")"; j=$((j+1))
    [ -f "$MJ_ROOT/$tgt" ] || continue   # projection_integrity reports absence
    if ! grep -q '\.ai/README\.md' "$MJ_ROOT/$tgt"; then
      mj_doctrine_fail bootstrap "$tgt" "does not point at .ai/README.md; a worker reading it never reaches the layer" "grep -n '.ai/README.md' $tgt"; bad=1
    elif grep -qE '^\| *`?(profile|routine|implementation)`? *\||^- \*\*[A-Za-z].*\*\*|^### (Rules|Ten rules|Lifecycle|Finish contract)' "$MJ_ROOT/$tgt"; then
      mj_doctrine_fail bootstrap "$tgt" "carries a rule corpus of its own (a profile table, rule bullets or a rules section); rules live under .ai/repo/rules/" "grep -nE '^- \*\*|^### ' $tgt"; bad=1
    fi
  done
  [ "$bad" = 0 ] && [ "$j" -gt 0 ] && mj_doctrine_ok bootstrap "$j projection(s)" "each points at .ai/README.md and carries no rule of its own"
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
  if ! cap="$(mj_pol_req ledger.retention_max_lines)"; then mj_doctrine_fail retention "ledger" "policy declares no ledger.retention_max_lines" "add it under ledger: in $(mj_rel "$MJ_POLICY_FILE"); see share/skeleton/policy.yaml"; else
  ll=0; [ -f "$MJ_STATE_DIR/ledger.jsonl" ] && ll="$(mj_lines "$MJ_STATE_DIR/ledger.jsonl")"
  if [ "$ll" -le "$cap" ]; then mj_doctrine_ok retention "ledger" "$ll lines, cap $cap"; else mj_doctrine_fail retention "ledger" "$ll lines over cap $cap" "wc -l $(mj_rel "$MJ_STATE_DIR")/ledger.jsonl"; fi; fi
  if ! cap="$(mj_pol_req handover.retention_max_files)"; then mj_doctrine_fail retention "handovers" "policy declares no handover.retention_max_files" "add it under handover: in $(mj_rel "$MJ_POLICY_FILE"); see share/skeleton/policy.yaml"; else
  hc="$(find "$MJ_STATE_DIR/handovers" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$hc" -le "$cap" ]; then mj_doctrine_ok retention "handovers" "$hc files, cap $cap"; else mj_doctrine_fail retention "handovers" "$hc files over cap $cap" "majordomus handover --list | wc -l"; fi; fi
  if ! cap="$(mj_pol_req checkpoint.retention_max_files)"; then mj_doctrine_fail retention "checkpoints" "policy declares no checkpoint.retention_max_files" "add a checkpoint: block to $(mj_rel "$MJ_POLICY_FILE"); see share/skeleton/policy.yaml"; else
  hc="$(find "$MJ_STATE_DIR/checkpoints" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$hc" -le "$cap" ]; then mj_doctrine_ok retention "checkpoints" "$hc files, cap $cap"; else mj_doctrine_fail retention "checkpoints" "$hc files over cap $cap" "majordomus checkpoint --list | wc -l"; fi; fi

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
  local skel="$MJ_SKELETON_DIR/policy.yaml" k last bad=0 n=0 flat
  [ -f "$skel" ] || { mj_doctrine_skip policy "skeleton" "no skeleton policy to compare against"; MJ_DOCTRINE_SKIPPED=1; return 0; }
  flat="$(mktemp "${TMPDIR:-/tmp}/mj.sk.XXXXXX")"
  mj_yaml_flatten "$skel" > "$flat" 2>/dev/null || { rm -f "$flat"; mj_doctrine_fail policy "skeleton" "share/skeleton/policy.yaml does not parse" "majordomus doctor"; return 0; }
  # a key is letters, digits and underscores per segment (benchmark.regression.p95 is one)
  for k in $(grep -rhE 'mj_pol(_req)? +[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)*' "$MJ_LIB_DIR" | grep -v '^[[:space:]]*#' \
             | grep -oE 'mj_pol(_req)? +[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)*' | sed -E 's/mj_pol(_req)? +//' | sort -u); do
    n=$((n + 1))
    if [ -z "$(mj_yget "$flat" "$k")" ] && ! grep -qE "^${k}\." "$flat"; then
      mj_doctrine_fail policy "$k" "read by lib/ but absent from share/skeleton/policy.yaml" "grep -rn 'mj_pol_req $k' lib/"; bad=1
    fi
  done
  rm -f "$flat"
  # a reader that supplies its own default is the drift this check exists to prevent
  for k in $(grep -rlnE 'mj_pol +[a-z_.]+\)"; \[ -n|:-[0-9]+\}' "$MJ_LIB_DIR" 2>/dev/null | grep -v common.sh || true); do
    last="$(grep -nE 'mj_pol +[a-z_.]+\)"; \[ -n' "$k" | head -1 | cut -d: -f1)"
    [ -n "$last" ] && { mj_doctrine_fail policy "$(basename "$k"):$last" "a policy value is read with a default written beside it; use mj_pol_req" "grep -n 'mj_pol ' $k"; bad=1; }
  done
  [ "$bad" = 0 ] && mj_doctrine_ok policy "$n key(s)" "every policy value the code reads is declared, with no reader-side default"
  return 0
}

# The AI layer as a whole: the manifest is one this executable reads and names sections
# that exist; the local half is ignored and nothing under it is tracked; no project data
# is left under the pre-.ai path. This is the check a fresh clone is judged by before any
# command reads a record: a layer that is not real makes every other finding moot.
mj_validate_ai_layout() {
  local bad=0 k d rel
  if [ "$MJ_LAYOUT" != ai ]; then
    mj_doctrine_fail layout ".ai/manifest.yaml" "absent; this repository has no AI layer this executable reads" "majordomus init"
    return 0
  fi
  mj_doctrine_ok layout "$(mj_rel "$MJ_AI_MANIFEST")" "schema $(mj_man schema)"
  for k in policy:MJ_POLICY_FILE profiles:MJ_PROFILES_DIR rules:MJ_RULES_DIR prompts:MJ_PROMPTS_DIR knowledge:MJ_KNOWLEDGE_DIR workflows:MJ_WORKFLOWS_DIR skills:MJ_SKILLS_DIR adrs:MJ_ADRS_DIR; do
    d="${k#*:}"; d="${!d}"
    [ -e "$d" ] || { mj_doctrine_fail layout "$(mj_rel "$d")" "named by the manifest as section '${k%%:*}' but absent" "majordomus init --extend"; bad=1; }
  done
  # the plan is the one section a repository may not have: an absent directory is a
  # repository without a plan, which `plan` says, not a broken layer
  [ -d "$MJ_PROJECT_DIR" ] || mj_info layout "$(mj_rel "$MJ_PROJECT_DIR")" "absent; this repository has no plan"
  [ -f "$MJ_AI_DIR/README.md" ] || { mj_doctrine_fail layout ".ai/README.md" "the protocol entrypoint is absent; the layer is not readable without the tool" "majordomus init --extend"; bad=1; }
  rel="$(mj_rel "$MJ_AI_LOCAL_DIR")"
  if ! mj_git check-ignore -q "$rel/state/current.yaml" 2>/dev/null; then
    mj_doctrine_fail layout "$rel/" "is not ignored by git; local state would travel with the branch" "printf '%s/\\n' $rel >> .gitignore"; bad=1
  elif [ -n "$(mj_git ls-files -- "$rel" 2>/dev/null | head -n 1)" ]; then
    mj_doctrine_fail layout "$rel/" "carries tracked files; the local half is this checkout's own" "git ls-files $rel"; bad=1
  else mj_doctrine_ok layout "$rel/" "ignored, and nothing under it is tracked"; fi
  if [ -f "$MJ_ROOT/.majordomus/policy.yaml" ]; then
    mj_doctrine_fail layout ".majordomus/policy.yaml" "pre-.ai project data beside the .ai layer; two layouts cannot both be authoritative" "majordomus migrate"; bad=1
  fi
  [ "$bad" = 0 ] && mj_doctrine_ok layout "$(mj_rel "$MJ_AI_DIR")/" "every section the manifest names exists; the layer is readable without the tool"
  return 0
}

mj_validate_layout() {
  local d
  for d in "$MJ_STATE_DIR/handovers" "$MJ_STATE_DIR/checkpoints" "$MJ_PROMPTS_DIR"; do
    if [ -d "$d" ]; then mj_doctrine_ok layout "$(mj_rel "$d")" "present"
    else mj_doctrine_fail layout "$(mj_rel "$d")" "missing; the command that writes it will create it, but update installs it" "majordomus update"; fi
  done
  return 0
}

# The questions store is read by the blocker gate. It is validated here as well as in
# check because doctor answers a different question — is the installation sound — and a
# repository with no active task never reaches the check path at all.
mj_validate_questions_store() {
  local bad; bad="$(mj_question_malformed "$MJ_STATE_DIR/open-questions.md")"
  if [ -n "$bad" ]; then mj_doctrine_fail records "open-questions.md" "line(s) $(printf '%s' "$bad" | sed 's/ $//') do not parse; an unreadable entry cannot block acceptance" "majordomus question list --all"
  else mj_doctrine_ok records "open-questions.md" "every entry parses"; fi
  return 0
}

mj_validate_prompts() {
  local f n=0 reason bad=0
  [ -d "$MJ_PROMPTS_DIR" ] || return 0
  [ "$MJ_DOCTRINE_CMD" = watch ] && mj_watch_prompts_empty
  for f in "$MJ_PROMPTS_DIR"/*.md; do
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
  if mj_resolve_latest "$MJ_STATE_DIR/handovers" ""; then
    mj_doctrine_ok resolver "handovers" "$MJ_RES_MATCH, $(mj_git_label "$MJ_RES_HEAD" "$MJ_RES_BRANCH")" "majordomus handover --resolve"
  else mj_doctrine_ok resolver "handovers" "no record for this worktree and branch (absence, not a stale match)" "majordomus handover --resolve"; fi
  [ "$MJ_RES_SKIPPED" -gt 0 ] && mj_doctrine_fail resolver "handovers" "$MJ_RES_SKIPPED record(s) skipped as malformed" "majordomus handover --list"
  return 0
}

# Part of context_budget: the same rule about the same resource, measured through the
# real command path rather than by re-implementing the builder here.
mj_context_builder_check() {
  local budget out lines
  if ! budget="$(mj_pol_req context.builder_budget_lines)"; then
    mj_doctrine_fail context "builder" "policy declares no context.builder_budget_lines; this installation predates the key" "add it under context: in $(mj_rel "$MJ_POLICY_FILE"); see share/skeleton/policy.yaml"
    return 0
  fi
  out="$(mktemp "${TMPDIR:-/tmp}/mj.dc.XXXXXX")"
  if ( export MJ_JSON=0; mj_cmd_context ) > "$out" 2>/dev/null; then
    lines="$(mj_lines "$out")"
    if [ "$lines" -le "$budget" ]; then mj_doctrine_ok context "builder" "$lines lines, budget $budget" "majordomus context"
    else mj_doctrine_fail context "builder" "$lines lines over budget $budget" "majordomus context"; fi
  else mj_doctrine_fail context "builder" "majordomus context failed" "majordomus context"; fi
  rm -f "$out"
  return 0
}
# ---------------------------------------------------------------- rule package
# The effective rule set is real: the vendored baseline matches its manifest file for file,
# every rule resolves with its dependencies, and a project rule claims no vendored identity.
# The dispatcher already refused to run at all if the set did not resolve, so what this
# adds is the vendor evidence and the report; a newer package in the distribution is
# information, never an action.
mj_validate_rule_package() {
  local vend probs n
  vend="$(mj_rules_vendor_dir)"
  if [ ! -f "$vend/manifest.yaml" ]; then
    mj_doctrine_fail rules "$(mj_rel "$vend")" "no vendored baseline; the repository enforces nothing of the standard package" "majordomus rules vendor update"
    return 0
  fi
  probs="$(mj_rules_manifest_check "$vend" || true)"
  if [ -n "$probs" ]; then
    mj_doctrine_fail rules "$(mj_rel "$vend")" "$(printf '%s' "$probs" | head -n 1)" "majordomus rules vendor status"
  else mj_doctrine_ok rules "$(mj_rel "$vend")" "every file matches the manifest ($(mj_rules_manifest_rev "$vend"))"; fi
  if [ -f "$MJ_STD_RULES_DIR/manifest.yaml" ] && ! diff -rq "$vend" "$MJ_STD_RULES_DIR" >/dev/null 2>&1; then
    mj_info rules "distribution" "ships a different package ($(mj_rules_manifest_rev "$MJ_STD_RULES_DIR")); the vendored one stays authoritative until updated" "majordomus rules vendor diff"
  fi
  mj_rules_load || { mj_doctrine_fail rules "effective set" "$MJ_RULES_ERROR" "majordomus rules list"; return 0; }
  n="$(mj_rule_count)"
  mj_doctrine_ok rules "$n rule(s)" "resolve in one deterministic order; vendored baseline plus project rules, no override"
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
  local lib="$MJ_LIB_DIR" root="$MJ_HOME"
  local i=0 id val cls fn cmd t c bad=0 n=0
  # the source, read once: which validator functions exist, which modules dispatch, which
  # modules turn a failing finding into a non-zero exit, and which claim ids are declared
  local fns dispatching propagating claims
  fns=" $(grep -rhoE '^mj_validate_[A-Za-z0-9_-]+\(\)' "$lib" | sed 's/()$//' | paste -sd' ' -) "
  dispatching=" $(grep -l 'mj_doctrine_dispatch' "$lib"/*.sh 2>/dev/null | paste -sd' ' -) "
  propagating=" $(grep -lE 'MJ_FAILS.*exit|exit .*MJ_EX_CONTRACT|MJ_DOCTOR_MISSING' "$lib"/*.sh 2>/dev/null | paste -sd' ' -) "
  claims=" $(sed -n 's/^  - id: //p' "$root/docs/CLAIMS.yaml" 2>/dev/null | paste -sd' ' -) "

  # 1. every declared doctrine resolves, end to end
  while mj_doc_row "$i"; do
    n=$((n+1)); id="$MJ_DR_ID"; val="$MJ_DR_VAL"; cls="$MJ_DR_CLASS"; fn="mj_validate_$val"
    case "$cls" in blocking|advisory) ;; *) mj_doctrine_fail doctrine "$id" "class '$cls' is neither blocking nor advisory" "grep -n '^class:' $MJ_DR_FILE"; bad=1 ;; esac
    case "$fns" in *" $fn "*) ;; *)
      mj_doctrine_fail doctrine "$id" "validator function $fn is defined nowhere in lib/" "grep -rn '$fn' lib/"; bad=1 ;;
    esac
    # the commands it claims to run under must actually dispatch
    for cmd in ${MJ_DR_EB//,/ }; do
      if [ ! -f "$lib/$cmd.sh" ]; then
        mj_doctrine_fail doctrine "$id" "enforced_by names '$cmd', which is not a command (lib/$cmd.sh)" "ls lib/"; bad=1
      else case "$dispatching" in *" $lib/$cmd.sh "*) ;; *)
        mj_doctrine_fail doctrine "$id" "declared for $cmd but lib/$cmd.sh never calls mj_doctrine_dispatch" "grep -n mj_doctrine_dispatch lib/$cmd.sh"; bad=1 ;;
      esac; fi
    done
    # a blocking doctrine must be able to stop its command
    if [ "$cls" = blocking ]; then
      for cmd in ${MJ_DR_EB//,/ }; do
        [ -f "$lib/$cmd.sh" ] || continue
        case "$propagating" in *" $lib/$cmd.sh "*) ;; *)
          mj_doctrine_fail doctrine "$id" "blocking, but lib/$cmd.sh never turns a failing finding into a non-zero exit" "grep -n 'MJ_FAILS' lib/$cmd.sh"; bad=1 ;;
        esac
      done
    fi
    # the tests that prove it must exist
    if [ -z "$MJ_DR_TEST" ]; then mj_doctrine_fail doctrine "$id" "declares no test" "grep -n 'tests:' $MJ_DR_FILE"; bad=1; fi
    for t in ${MJ_DR_TESTS//,/ }; do
      [ -f "$root/$t" ] || { mj_doctrine_fail doctrine "$id" "test $t does not exist" "ls $t"; bad=1; }
    done
    # every claim it carries must be a real claim
    for c in ${MJ_DR_CLAIMS//,/ }; do
      case "$claims" in *" $c "*) ;; *)
        mj_doctrine_fail doctrine "$id" "names claim '$c', which is not in docs/CLAIMS.yaml" "grep -n 'id: $c' docs/CLAIMS.yaml"; bad=1 ;;
      esac
    done
    i=$((i+1))
  done

  # 2. the other direction — a validator no doctrine declares
  local f declared=" "
  i=0; while mj_doc_row "$i"; do declared="$declared$MJ_DR_VAL "; i=$((i+1)); done
  for f in $(grep -rhoE '^mj_validate_[a-z_]+\(\)' "$lib" | sed -e 's/^mj_validate_//' -e 's/()//' | sort -u); do
    case "$declared" in *" $f "*) ;; *) mj_doctrine_fail doctrine "mj_validate_$f" "validator exists but no rule declares it; it runs under no rule" "grep -rn 'validator: $f' $(mj_rel "$MJ_RULES_DIR")"; bad=1 ;; esac
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
# watch's view of the same doctrine: every target against the stamp it carries.
mj_watch_projection() {
  local j=0 tgt mode prov st
  while [ -n "$(mj_pol "projections.$j.target")" ]; do
    tgt="$(mj_pol "projections.$j.target")"; mode="$(mj_projection_mode "$j")"; prov="$(mj_pol "projections.$j.provider")"
    case "$mode" in file|region) ;; *) j=$((j+1)); continue ;; esac   # the policy validator reports it
    st="$(mj_projection_status "$tgt" "$mode" "$prov")"; st="${st%% *}"
    case "$st" in
      no_template) mj_doctrine_fail projection "$tgt" "provider '$prov' has no template" "majordomus doctor" ;;
      absent) mj_doctrine_fail projection "$tgt" "missing" "majordomus update" ;;
      region_absent) mj_doctrine_fail projection "$tgt" "region markers are absent" "majordomus update" ;;
      malformed) mj_doctrine_fail projection "$tgt" "region markers are malformed" "grep -n 'majordomus:begin' $tgt" ;;
      unstamped) mj_doctrine_fail projection "$tgt" "carries no generation stamp" "majordomus update --diff $tgt" ;;
      hand_edited) mj_doctrine_fail projection "$tgt" "$([ "$mode" = region ] && printf 'region' || printf 'content') differs from its stamp (hand-edited?)" "majordomus update --diff $tgt" ;;
      ok) mj_doctrine_ok projection "$tgt" "$([ "$mode" = region ] && printf 'region' || printf 'content') matches its stamp" ;;
    esac
    j=$((j+1))
  done
}

# watch asks whether the policy has moved since the projections were generated; doctor
# asks whether it is valid at all. Same doctrine, two questions. The evidence is the
# policy hash each target's stamp names.
mj_watch_policy() {
  local tmp psha j=0 tgt mode prov st stamped=0 stale=""
  tmp="$(mktemp "${TMPDIR:-/tmp}/mj.w.XXXXXX")"; mj_policy_cat > "$tmp"; psha="$(mj_sha256 "$tmp")"; rm -f "$tmp"
  while [ -n "$(mj_pol "projections.$j.target")" ]; do
    tgt="$(mj_pol "projections.$j.target")"; mode="$(mj_projection_mode "$j")"; prov="$(mj_pol "projections.$j.provider")"
    case "$mode" in file|region) ;; *) j=$((j+1)); continue ;; esac
    st="$(mj_projection_status "$tgt" "$mode" "$prov")"
    case "${st%% *}" in
      ok|hand_edited) stamped=$((stamped+1)); [ "${st#* }" = "${psha:0:12}" ] || stale="$stale $tgt" ;;
    esac
    j=$((j+1))
  done
  if [ "$stamped" = 0 ]; then mj_doctrine_fail policy "projections" "no projections generated yet" "majordomus update"
  elif [ -z "$stale" ]; then mj_doctrine_ok policy "policy+profiles" "match the last update (${psha:0:12})"
  else mj_doctrine_fail policy "$(mj_rel "$MJ_POLICY_FILE")" "policy or profiles changed after the last update of${stale}" "majordomus update --dry-run"; fi
  return 0
}

# ---------------------------------------------------------------- project model validators
# A repository is not obliged to have a canonical project model; most installations will
# not. Both validators therefore skip cleanly when .ai/repo/project/ is absent, so
# adopting Majordomus does not turn an installation red for a feature nobody opted into.
mj_project_doctrine_load() {
  mj_project_present || { MJ_DOCTRINE_SKIPPED=1; mj_doctrine_skip project "$(mj_rel "$MJ_PROJECT_DIR")" "no canonical project model here; nothing to validate"; return 1; }
  local rc=0; mj_project_load || rc=$?
  [ "$rc" = 0 ] && return 0
  mj_doctrine_fail project "$(mj_rel "$MJ_PROJECT_DIR")" "the canonical model does not load; a file does not parse or two records claim one id" "majordomus plan validate"
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
  mj_doctrine_ok project "$(mj_rel "$MJ_PROJECT_DIR")" "$(mj_pj_milestone_ids | wc -l | tr -d ' ') milestone(s), $(mj_pj_issue_ids | wc -l | tr -d ' ') issue(s), every key read by something"
  return 0
}

# The graph rules, reported through the dispatcher so that watch sees the same violations
# as drift. A warning from the model is a warning here: work in progress is reported, not
# blocked, and only a graph that cannot be executed is a failure.
# The roadmap is a projection of milestone state, so no document may be a second authority
# for it. While a hand-written roadmap table survives anywhere, every version it lists has to
# be a canonical milestone's version and every canonical version has to appear in it — a table
# that can neither invent a release nor hide one. When the table is gone the check holds
# trivially, which is the point: this refuses the regression, it does not require the table.
mj_validate_roadmap() {
  local readme canon doc missing extra
  mj_project_doctrine_load || return 0
  readme="$MJ_ROOT/README.md"
  [ -f "$readme" ] || { mj_doctrine_ok roadmap "README.md" "absent; nothing can duplicate the roadmap"; return 0; }
  if ! grep -q '^## Roadmap' "$readme"; then
    mj_doctrine_ok roadmap "README.md" "no authored roadmap section; the roadmap is only a projection"
    return 0
  fi
  canon="$(awk -F'\t' '$1=="M" && $9!="" { print $9 }' "$MJ_PJ/model.tsv" | sort -u)"
  doc="$(awk '/^## Roadmap/{f=1;next} /^## /{f=0} f' "$readme" \
         | sed -n 's/^| *\**\([0-9][0-9.]*\)\** *|.*/\1/p' | sort -u)"
  # A section that lists no versions is prose pointing at the projection, not a second
  # authority. The rule is about an authored version list, not about the heading: a partial
  # list is the dangerous case, and no list at all is the intended end state.
  if [ -z "$doc" ]; then
    mj_doctrine_ok roadmap "README.md" "the roadmap section lists no versions; it points at the projection rather than restating it"
    return 0
  fi
  missing="$(comm -23 <(printf '%s\n' "$canon") <(printf '%s\n' "$doc") | tr '\n' ' ')"
  extra="$(comm -13 <(printf '%s\n' "$canon") <(printf '%s\n' "$doc") | tr '\n' ' ')"
  missing="${missing% }"; extra="${extra% }"
  if [ -n "$extra" ]; then
    mj_doctrine_fail roadmap "README.md" "lists version(s) no milestone declares: $extra" "majordomus plan roadmap"
    return 0
  fi
  if [ -n "$missing" ]; then
    mj_doctrine_fail roadmap "README.md" "omits milestone version(s) the model declares: $missing" "majordomus plan roadmap"
    return 0
  fi
  mj_doctrine_ok roadmap "README.md" "every version it lists is a milestone, and no milestone is hidden from it"
  return 0
}

mj_validate_dag() {
  local lvl subj msg n
  mj_project_doctrine_load || return 0
  mj_pj_findings | while IFS="$MJ_TAB" read -r _ lvl _ subj msg; do
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

# ---------------------------------------------------------------- catalogue
# The use-case and application catalogues describe the tool in terms of the tool: every
# step names a command, every rule a doctrine, every promise a claim, every context a
# responsibility. None of that is checked by reading it, so it is checked here — and the
# two files reference each other, so both directions are checked rather than one.
#
# This is the doctrine layer applied to prose: a catalogue entry that names a command
# nobody wrote is the same defect as a rule nothing invokes, and it fails the same way.
mj_validate_catalogue() {
  local root="$MJ_HOME" uc="$MJ_SHARE_DIR/use-cases.yaml" ap="$MJ_SHARE_DIR/applications.yaml"
  local ucf apf bad=0 i j k id ref n_uc=0 n_ap=0
  [ -f "$uc" ] && [ -f "$ap" ] || { mj_doctrine_skip catalogue "-" "no catalogue shipped with this installation"; MJ_DOCTRINE_SKIPPED=1; return 0; }
  ucf="$(mktemp "${TMPDIR:-/tmp}/mj.uc.XXXXXX")"; apf="$(mktemp "${TMPDIR:-/tmp}/mj.ap.XXXXXX")"
  mj_yaml_flatten "$uc" > "$ucf" 2>/dev/null || { mj_doctrine_fail catalogue "share/use-cases.yaml" "does not parse" "majordomus doctor"; rm -f "$ucf" "$apf"; return 0; }
  mj_yaml_flatten "$ap" > "$apf" 2>/dev/null || { mj_doctrine_fail catalogue "share/applications.yaml" "does not parse" "majordomus doctor"; rm -f "$ucf" "$apf"; return 0; }
  # both catalogues loaded into variables once; every field read below is an expansion
  mj_yload "$ucf" uc; mj_yload "$apf" ap
  local dispatch_line
  dispatch_line="$(grep -E '^ *init\|doctor\|' "$MJ_BIN_DIR/majordomus" | head -n1)"
  dispatch_line="${dispatch_line%%)*}"; dispatch_line="${dispatch_line#"${dispatch_line%%[! ]*}"}"

  # every reference resolves to something that exists
  i=0
  while [ -n "$(mj_yv uc "use_cases.$i.id")" ]; do
    id="$(mj_yv uc "use_cases.$i.id")"; n_uc=$((n_uc+1))
    # resolved against the dispatch table, which is what actually decides whether a
    # command exists — CLI.md documents it, bin/majordomus is it
    for ref in $(mj_yvlist uc "use_cases.$i.commands"); do
      mj_cat_is_command "$ref" "$dispatch_line" || { mj_doctrine_fail catalogue "$id" "names command '$ref', which bin/majordomus does not dispatch" "majordomus --help"; bad=1; }
    done
    # the steps are what a reader actually runs, so they are checked in their own right
    # rather than trusted to appear in the commands list
    k=0
    while [ -n "$(mj_yv uc "use_cases.$i.steps.$k.command")" ]; do
      ref="$(mj_yv uc "use_cases.$i.steps.$k.command")"
      mj_cat_is_command "$ref" "$dispatch_line" || { mj_doctrine_fail catalogue "$id" "step $((k+1)) names command '$ref', which bin/majordomus does not dispatch" "majordomus --help"; bad=1; }
      [ -n "$(mj_yv uc "use_cases.$i.steps.$k.note")" ] || { mj_doctrine_fail catalogue "$id" "step $((k+1)) has no note saying what it does here" "grep -n -A12 'id: $id' share/use-cases.yaml"; bad=1; }
      k=$((k+1))
    done
    [ "$k" -gt 0 ] || { mj_doctrine_fail catalogue "$id" "has no steps; a use case that runs nothing is a description, not a use case" "grep -n -A12 'id: $id' share/use-cases.yaml"; bad=1; }
    for ref in $(mj_yvlist uc "use_cases.$i.doctrines"); do
      mj_doc_index "$ref" >/dev/null 2>&1 || { mj_doctrine_fail catalogue "$id" "names doctrine '$ref', which is not in the registry" "majordomus doctrine list"; bad=1; }
    done
    for ref in $(mj_yvlist uc "use_cases.$i.claims"); do
      grep -q "^  - id: $ref$" "$root/docs/CLAIMS.yaml" 2>/dev/null || { mj_doctrine_fail catalogue "$id" "names claim '$ref', which is not in docs/CLAIMS.yaml" "grep -n 'id: $ref' docs/CLAIMS.yaml"; bad=1; }
    done
    # the reference to an application must be mutual: a catalogue that disagrees with
    # itself about what applies to what is worse than one that says less
    for ref in $(mj_yvlist uc "use_cases.$i.applications"); do
      if ! mj_cat_has ap applications "$ref"; then
        mj_doctrine_fail catalogue "$id" "names application '$ref', which does not exist" "grep -n 'id:' share/applications.yaml"; bad=1
      elif ! mj_cat_back ap applications "$ref" use_cases "$id"; then
        mj_doctrine_fail catalogue "$id" "names application '$ref', which does not name it back" "grep -n -A20 'id: $ref' share/applications.yaml"; bad=1
      fi
    done
    i=$((i+1))
  done

  j=0
  while [ -n "$(mj_yv ap "applications.$j.id")" ]; do
    id="$(mj_yv ap "applications.$j.id")"; n_ap=$((n_ap+1))
    # an application that lists only fits is a brochure; both sides are required
    [ -n "$(mj_yv ap "applications.$j.fits_when.0")" ] || { mj_doctrine_fail catalogue "$id" "declares no fits_when" "grep -n -A8 'id: $id' share/applications.yaml"; bad=1; }
    [ -n "$(mj_yv ap "applications.$j.does_not_fit_when.0")" ] || { mj_doctrine_fail catalogue "$id" "declares no does_not_fit_when; an application that only lists fits is not a description" "grep -n -A8 'id: $id' share/applications.yaml"; bad=1; }
    for ref in $(mj_yvlist ap "applications.$j.doctrines"); do
      mj_doc_index "$ref" >/dev/null 2>&1 || { mj_doctrine_fail catalogue "$id" "names doctrine '$ref', which is not in the registry" "majordomus doctrine list"; bad=1; }
    done
    for ref in $(mj_yvlist ap "applications.$j.use_cases"); do
      if ! mj_cat_has uc use_cases "$ref"; then
        mj_doctrine_fail catalogue "$id" "names use case '$ref', which does not exist" "grep -n 'id:' share/use-cases.yaml"; bad=1
      elif ! mj_cat_back uc use_cases "$ref" applications "$id"; then
        mj_doctrine_fail catalogue "$id" "names use case '$ref', which does not name it back" "grep -n -A20 'id: $ref' share/use-cases.yaml"; bad=1
      fi
    done
    j=$((j+1))
  done

  [ "$bad" = 0 ] && mj_doctrine_ok catalogue "$n_uc use case(s), $n_ap application(s)" "every command, doctrine, claim and cross-reference resolves, both directions"
  rm -f "$ucf" "$apf"
  return 0
}
# does <flat> contain an entry of <kind> with id <id>?
mj_cat_has() {
  local p="$1" kind="$2" want="$3" k=0 v
  while v="$(mj_yv "$p" "$kind.$k.id")"; [ -n "$v" ]; do
    [ "$v" = "$want" ] && return 0
    k=$((k+1))
  done
  return 1
}
mj_cat_back() {
  local p="$1" kind="$2" want="$3" field="$4" back="$5" k=0 v
  while v="$(mj_yv "$p" "$kind.$k.id")"; [ -n "$v" ]; do
    if [ "$v" = "$want" ]; then
      for v in $(mj_yvlist "$p" "$kind.$k.$field"); do [ "$v" = "$back" ] && return 0; done
      return 1
    fi
    k=$((k+1))
  done
  return 1
}
# mj_cat_is_command <name> [<dispatch line>]: the dispatch table decides; the caller that
# checks many names reads the line once and passes it
mj_cat_is_command() {
  local line="${2:-}"
  if [ -z "$line" ]; then
    line="$(grep -E '^ *init\|doctor\|' "$MJ_BIN_DIR/majordomus" | head -n1)"
    [ -n "$line" ] || return 1
    line="${line%%)*}"; line="${line#"${line%%[! ]*}"}"   # drop the trailing ) and the indent
  fi
  case "|$line|" in *"|$1|"*) return 0 ;; esac
  return 1
}
