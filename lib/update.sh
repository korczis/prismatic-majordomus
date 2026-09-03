#!/usr/bin/env bash
# update — regenerate provider projections from policy. Deterministic. Refuses to clobber hand edits.
mj_cmd_update() {
  local dry=0 force=0 diff_target="" a
  while [ $# -gt 0 ]; do case "$1" in
    --dry-run) dry=1; shift ;; --force) force=1; shift ;;
    --diff) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--diff needs a target"; diff_target="$2"; shift 2 ;;
    --help|-h) cat <<H
usage: majordomus update [--dry-run] [--diff <target>] [--force]
  regenerates every projections[].target from .majordomus/policy.yaml and providers/*.tmpl
  --dry-run   print what would change, write nothing
  --diff T    show the diff between the current file T and its regenerated form
  --force     overwrite a target whose content matches neither its fingerprint nor the new output
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "update: unknown option $1" ;;
  esac; done
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"

  local tmp; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.up.XXXXXX")"
  # policy hash: policy + profiles, sorted, concatenated
  cat "$MJ_DIR/policy.yaml" "$MJ_DIR"/profiles/*.yaml > "$tmp/policy.cat"
  local psha; psha="$(mj_sha256 "$tmp/policy.cat")"
  mj_build_fragments "$tmp"

  # render every target first; write only if all pass
  local i=0 tgt prov out fp="$MJ_DIR/generated/fingerprints.yaml" fpflat="" always="" budget
  budget="$(mj_pol context.always_loaded_budget_lines)"
  [ -f "$fp" ] && { fpflat="$tmp/fp.flat"; mj_yaml_flatten "$fp" > "$fpflat" 2>/dev/null || fpflat=""; }
  local plan="" refuse=0
  while [ -n "$(mj_pol "projections.$i.target")" ]; do
    tgt="$(mj_pol "projections.$i.target")"; prov="$(mj_pol "projections.$i.provider")"
    [ -f "$MJ_DIR/providers/$prov.tmpl" ] || { rm -rf "$tmp"; mj_die "$MJ_EX_MISSING" "no template for provider '$prov' (.majordomus/providers/$prov.tmpl)"; }
    out="$tmp/out.$i"
    mj_render "$MJ_DIR/providers/$prov.tmpl" "$tmp" "$psha" > "$out"
    if [ "$(mj_pol "projections.$i.always_loaded")" = true ]; then
      always="$tgt"
      if [ "$(mj_lines "$out")" -gt "$budget" ]; then
        mj_fail budget "$tgt" "would be $(mj_lines "$out") lines, budget $budget; nothing written" "majordomus update --dry-run"
        rm -rf "$tmp"; exit "$MJ_EX_CONTRACT"
      fi
    fi
    if [ -n "$diff_target" ] && [ "$diff_target" = "$tgt" ]; then
      diff -u "$MJ_ROOT/$tgt" "$out" 2>/dev/null || true; rm -rf "$tmp"; return 0
    fi
    # hand-edit protection
    if [ -f "$MJ_ROOT/$tgt" ]; then
      local have want new; have="$(mj_sha256 "$MJ_ROOT/$tgt")"; new="$(mj_sha256 "$out")"; want=""
      [ -n "$fpflat" ] && want="$(mj_fp_sha_u "$fpflat" "$tgt")"
      if [ "$have" = "$new" ]; then plan="$plan unchanged:$tgt"
      elif [ "$have" = "$want" ] || [ "$force" = 1 ]; then plan="$plan update:$tgt"
      else
        mj_finding REFUSE projection "$tgt" "current content matches neither its fingerprint nor the new output (hand-edited?); use --diff $tgt, then --force" "majordomus update --diff $tgt"
        refuse=1
      fi
    else plan="$plan create:$tgt"; fi
    i=$((i+1))
  done
  [ -n "$diff_target" ] && { rm -rf "$tmp"; mj_die "$MJ_EX_USAGE" "--diff: '$diff_target' is not a projection target"; }
  [ "$refuse" = 1 ] && { rm -rf "$tmp"; exit "$MJ_EX_REFUSED"; }
  [ "$i" = 0 ] && { rm -rf "$tmp"; mj_die "$MJ_EX_CONTRACT" "policy declares no projections"; }

  local p
  for p in $plan; do printf '%s %s\n' "${p%%:*}" "${p#*:}"; done
  if [ "$dry" = 1 ]; then rm -rf "$tmp"; return 0; fi

  # write atomically, then fingerprints
  {
    printf 'policy_sha256: %s\ngenerated_at: %s\ntargets:\n' "$psha" "$(mj_now)"
    i=0
    while [ -n "$(mj_pol "projections.$i.target")" ]; do
      tgt="$(mj_pol "projections.$i.target")"; out="$tmp/out.$i"
      mkdir -p "$(dirname "$MJ_ROOT/$tgt")"
      cp "$out" "$MJ_ROOT/$tgt.mj-tmp" && mv "$MJ_ROOT/$tgt.mj-tmp" "$MJ_ROOT/$tgt"
      printf '  - target: %s\n    sha256: %s\n    lines: %s\n' "$tgt" "$(mj_sha256 "$MJ_ROOT/$tgt")" "$(mj_lines "$MJ_ROOT/$tgt")"
      i=$((i+1))
    done
  } > "$fp.mj-tmp" && mv "$fp.mj-tmp" "$fp"
  mkdir -p "$MJ_DIR/state"
  mj_ledger_append projections.updated "\"policy_sha256\":\"$psha\",\"targets\":$i"
  rm -rf "$tmp"
  printf 'fingerprints: .majordomus/generated/fingerprints.yaml (policy %s)\n' "${psha:0:12}"
}

mj_fp_sha_u() { local f="$1" t="$2" k=0
  while [ -n "$(mj_yget "$f" "targets.$k.target")" ]; do
    [ "$(mj_yget "$f" "targets.$k.target")" = "$t" ] && { mj_yget "$f" "targets.$k.sha256"; return; }; k=$((k+1)); done; }

# builds multi-line fragment files in $1: PROFILE_TABLE, FINISH_CONTRACT, REQUIRED_SECTIONS
mj_build_fragments() {
  local d="$1" pf n
  {
    printf '| profile | capability | effort | verbosity | checkpoint | verification |\n|---|---|---|---|---|---|\n'
    for pf in "$MJ_DIR"/profiles/*.yaml; do
      n="$(basename "$pf" .yaml)"; mj_load_profile "$n" || continue
      local v=""; [ "$(mj_pro verification.verify_command_required)" = true ] && v="verify command"
      [ "$(mj_pro verification.verify_command_required)" = if_files_changed ] && v="verify command if files changed"
      [ "$(mj_pro verification.regression_test_required)" = true ] && v="$v, regression test"
      [ "$(mj_pro verification.decision_record_required)" = true ] && v="$v, decision record"
      printf '| `%s` | %s | %s | %s | %s | %s |\n' "$n" "$(mj_pro capability)" "$(mj_pro effort)" "$(mj_pro verbosity)" "$(mj_pro checkpoint_interval)" "${v#, }"
    done
  } > "$d/PROFILE_TABLE"
  {
    local r
    for r in $(mj_ylist "$MJ_POL_FLAT" verification.finish_requires); do
      case "$r" in
        scope_respected)  printf -- '- **scope respected** — touched files are within the claimed paths\n' ;;
        verification_ran) printf -- '- **verification ran** — `--verify-command` exited 0 and was recorded\n' ;;
        state_updated)    printf -- '- **state updated** — the task record is at or behind HEAD, not diverged\n' ;;
        no_open_blockers) printf -- '- **no open blockers** — no unresolved entry for this task in open-questions.md\n' ;;
        note_present)     printf -- '- **note present** — a handover or completion note with the required sections exists\n' ;;
        *)                printf -- '- **%s**\n' "$r" ;;
      esac
    done
  } > "$d/FINISH_CONTRACT"
  mj_ylist "$MJ_POL_FLAT" handover.required_sections | sed 's/^/`# /; s/$/`/' | paste -sd, - | sed 's/,/, /g' > "$d/REQUIRED_SECTIONS"
  sed -e "s|{{CHECKPOINT_DEFAULT}}|$(mj_pol profiles.checkpoint_interval_default)|g" \
      -e "s|{{DEFAULT_PROFILE}}|$(mj_pol profiles.default)|g" "$MJ_DIR/providers/body.md" \
    | mj_expand_blocks "$d" > "$d/BODY"
}
# replace lines that are exactly {{TOKEN}} with the file $d/TOKEN; inline {{REQUIRED_SECTIONS}} too
mj_expand_blocks() {
  local d="$1"
  awk -v d="$d" '
    { line=$0
      if (match(line,/^\{\{[A-Z_]+\}\}$/)) { tok=substr(line,3,length(line)-4); f=d "/" tok
        if ((getline first < f) >= 0) { print first; while ((getline l < f) > 0) print l; close(f); next } }
      while (match(line,/\{\{REQUIRED_SECTIONS\}\}/)) { f=d "/REQUIRED_SECTIONS"; getline r < f; close(f)
        line=substr(line,1,RSTART-1) r substr(line,RSTART+RLENGTH) }
      print line }'
}
mj_render() { # template, fragment dir, policy sha
  sed -e "s|{{POLICY_SHA}}|${3:0:12}|g" "$1" | mj_expand_blocks "$2"
}
