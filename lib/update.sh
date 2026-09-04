#!/usr/bin/env bash
# update — regenerate provider projections from policy. Deterministic. Refuses to clobber hand edits.
mj_cmd_update() {
  local dry=0 force=0 diff_target=""
  while [ $# -gt 0 ]; do case "$1" in
    --dry-run) dry=1; shift ;; --force) force=1; shift ;;
    --diff) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--diff needs a target"; diff_target="$2"; shift 2 ;;
    --help|-h) cat <<H
usage: majordomus update [--dry-run] [--diff <target>] [--force]
  regenerates every projections[].target from .majordomus/policy.yaml and providers/*.tmpl
  mode: file    the whole target is generated (the default)
  mode: region  only the text between the majordomus:begin and majordomus:end markers is
                generated; the rest of the target is left byte for byte alone, and an
                absent region is appended once
  --dry-run   print what would change, write nothing
  --diff T    show the diff between the current file T and its regenerated form
  --force     overwrite a target whose content matches neither its fingerprint nor the new output
H
      return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "update: unknown option $1" ;;
  esac; done
  mj_require_installed
  mj_load_policy || mj_die "$MJ_EX_CONTRACT" "policy does not parse (run: majordomus doctor)"
  [ "$dry" = 1 ] || mj_ensure_layout

  local tmp; tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.up.XXXXXX")"
  # policy hash: policy + profiles, in that order, concatenated
  mj_policy_cat > "$tmp/policy.cat"
  local psha; psha="$(mj_sha256 "$tmp/policy.cat")"
  mj_build_fragments "$tmp"

  # render every target first; write only if all pass
  local i=0 tgt prov out gen mode rc fp="$MJ_GENERATED_DIR/fingerprints.yaml" fpflat="" budget
  budget="$(mj_pol context.always_loaded_budget_lines)"
  [ -f "$fp" ] && { fpflat="$tmp/fp.flat"; mj_yaml_flatten "$fp" > "$fpflat" 2>/dev/null || fpflat=""; }
  local plan="" refuse=0
  while [ -n "$(mj_pol "projections.$i.target")" ]; do
    tgt="$(mj_pol "projections.$i.target")"; prov="$(mj_pol "projections.$i.provider")"
    mode="$(mj_projection_mode "$i")"
    case "$mode" in file|region) ;;
      *) rm -rf "$tmp"; mj_die "$MJ_EX_CONTRACT" "projection $tgt: unknown mode '$mode' (file | region)" ;;
    esac
    local tpl
    tpl="$(mj_provider_template "$prov")" || { rm -rf "$tmp"; mj_die "$MJ_EX_MISSING" "no template for provider '$prov' ($(mj_rel "$MJ_PROVIDERS_DIR")/$prov.tmpl, or one shipped in $MJ_PROVIDERS_DEFAULT_DIR/)"; }
    # gen is the generated content this projection owns; out is the whole file to write
    gen="$tmp/gen.$i"; out="$tmp/out.$i"
    mj_render "$tpl" "$tmp" "$psha" > "$gen"

    # the budget measures what Majordomus generates, which for a region is the region
    # and not the host document it was appended to
    if [ "$(mj_pol "projections.$i.always_loaded")" = true ]; then
      if [ "$(mj_lines "$gen")" -gt "$budget" ]; then
        mj_fail budget "$tgt" "would be $(mj_lines "$gen") lines, budget $budget; nothing written" "majordomus update --dry-run"
        rm -rf "$tmp"; exit "$MJ_EX_CONTRACT"
      fi
    fi

    # current content of what this projection owns, and the whole file that would replace it
    local have="" cur="$tmp/cur.$i"
    if [ "$mode" = region ]; then
      : > "$cur"
      if [ -f "$MJ_ROOT/$tgt" ]; then
        rc=0; mj_region_extract "$MJ_ROOT/$tgt" > "$cur" 2>/dev/null || rc=$?
        case "$rc" in
          0) have="$(mj_sha256 "$cur")" ;;
          1) have="" ;;   # no region yet; update appends one
          *) mj_finding REFUSE projection "$tgt" "region markers are malformed (unclosed, out of order, or repeated); nothing written" "grep -n 'majordomus:begin\|majordomus:end' $tgt"
             refuse=1; i=$((i+1)); continue ;;
        esac
      fi
      mj_region_splice "$MJ_ROOT/$tgt" "$gen" "${psha:0:12}" > "$out"
    else
      out="$gen"
      [ -f "$MJ_ROOT/$tgt" ] && have="$(mj_sha256 "$MJ_ROOT/$tgt")"
    fi

    if [ -n "$diff_target" ] && [ "$diff_target" = "$tgt" ]; then
      if [ "$mode" = region ]; then diff -u "$cur" "$gen" 2>/dev/null || true
      else diff -u "$MJ_ROOT/$tgt" "$out" 2>/dev/null || true; fi
      rm -rf "$tmp"; return 0
    fi

    # hand-edit protection, on the content this projection owns
    if [ -z "$have" ]; then plan="$plan create:$tgt"
    else
      local want new; new="$(mj_sha256 "$gen")"; want=""
      [ -n "$fpflat" ] && want="$(mj_fp_sha_u "$fpflat" "$tgt")"
      if [ "$have" = "$new" ] && { [ "$mode" = file ] || [ "$(mj_sha256 "$MJ_ROOT/$tgt")" = "$(mj_sha256 "$out")" ]; }; then plan="$plan unchanged:$tgt"
      elif [ "$have" = "$new" ] || [ "$have" = "$want" ] || [ "$force" = 1 ]; then plan="$plan update:$tgt"
      else
        mj_finding REFUSE projection "$tgt" "current content matches neither its fingerprint nor the new output (hand-edited?); use --diff $tgt, then --force" "majordomus update --diff $tgt"
        refuse=1
      fi
    fi
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
      tgt="$(mj_pol "projections.$i.target")"; mode="$(mj_projection_mode "$i")"
      gen="$tmp/gen.$i"; out="$tmp/out.$i"; [ "$mode" = region ] || out="$gen"
      mkdir -p "$(dirname "$MJ_ROOT/$tgt")"
      cp "$out" "$MJ_ROOT/$tgt.mj-tmp" && mv "$MJ_ROOT/$tgt.mj-tmp" "$MJ_ROOT/$tgt"
      # the fingerprint covers the generated content, so for a region an edit outside
      # the markers is the host document's business and never reported as drift
      printf '  - target: %s\n    mode: %s\n    sha256: %s\n    lines: %s\n' \
        "$tgt" "$mode" "$(mj_sha256 "$gen")" "$(mj_lines "$gen")"
      i=$((i+1))
    done
  } > "$fp.mj-tmp" && mv "$fp.mj-tmp" "$fp"
  mkdir -p "$MJ_STATE_DIR"
  mj_ledger_append projections.updated "\"policy_sha256\":\"$psha\",\"targets\":$i"
  rm -rf "$tmp"
  printf 'fingerprints: %s/fingerprints.yaml (policy %s)\n' "$(mj_rel "$MJ_GENERATED_DIR")" "${psha:0:12}"
}

mj_fp_sha_u() { local f="$1" t="$2" k=0
  while [ -n "$(mj_yget "$f" "targets.$k.target")" ]; do
    [ "$(mj_yget "$f" "targets.$k.target")" = "$t" ] && { mj_yget "$f" "targets.$k.sha256"; return; }; k=$((k+1)); done; }

# builds multi-line fragment files in $1: PROFILE_TABLE, FINISH_CONTRACT, REQUIRED_SECTIONS
mj_build_fragments() {
  local d="$1" pf n
  {
    printf '| profile | capability | effort | verbosity | checkpoint | verification |\n|---|---|---|---|---|---|\n'
    for pf in "$MJ_PROFILES_DIR"/*.yaml; do
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
      -e "s|{{DEFAULT_PROFILE}}|$(mj_pol profiles.default)|g" "$MJ_PROVIDERS_DIR/body.md" \
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

# Bring an installation created by an older version up to the current layout: create the
# directories this version reads, and seed the skeleton prompt assets when the directory
# is absent entirely. Never overwrites a file, never deletes, never touches state records.
mj_ensure_layout() {
  local skel="$MJ_SKELETON_DIR" d
  for d in "$MJ_STATE_DIR/handovers" "$MJ_STATE_DIR/checkpoints" "$MJ_PROMPTS_DIR"; do
    [ -d "$d" ] || { mkdir -p "$d"; printf 'create %s/\n' "$(mj_rel "$d")"; }
  done
  if [ -d "$skel/prompts" ] && [ -z "$(ls -1 "$MJ_PROMPTS_DIR" 2>/dev/null)" ]; then
    cp "$skel"/prompts/*.md "$MJ_PROMPTS_DIR/" 2>/dev/null || true
    printf 'create %s/ (%s asset(s))\n' "$(mj_rel "$MJ_PROMPTS_DIR")" "$(ls -1 "$MJ_PROMPTS_DIR"/*.md 2>/dev/null | wc -l | tr -d ' ')"
  fi
  for d in decisions open-questions; do
    [ -f "$MJ_STATE_DIR/$d.md" ] || { cp "$skel/templates/$d.md" "$MJ_STATE_DIR/$d.md" 2>/dev/null && printf 'create %s/%s.md\n' "$(mj_rel "$MJ_STATE_DIR")" "$d"; }
  done
}
