#!/usr/bin/env bash
# update — regenerate provider projections from policy. Deterministic. Refuses to clobber hand edits.
#
# Every target it writes is self-describing: a file-mode target starts with a stamp naming
# the policy hash and the hash of the content under it, and a region-mode target carries
# both in its begin marker. doctor and watch compare a target with its own stamp, so a hand
# edit is detected on a fresh clone with nothing else tracked, and there is no fingerprint
# file that could disagree with the targets it describes.
mj_cmd_update() {
  local dry=0 force=0 diff_target=""
  while [ $# -gt 0 ]; do case "$1" in
    --dry-run) dry=1; shift ;; --force) force=1; shift ;;
    --diff) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--diff needs a target"; diff_target="$2"; shift 2 ;;
    --help|-h) cat <<H
usage: majordomus update [--dry-run] [--diff <target>] [--force]
  regenerates every projections[].target from the policy and the provider templates
  mode: file    the whole target is generated (the default); its first line is a stamp
                naming the policy hash and the hash of the content below it
  mode: region  only the text between the majordomus:begin and majordomus:end markers is
                generated and the begin marker carries the two hashes; the rest of the
                target is left byte for byte alone, and an absent region is appended once
  --dry-run   print what would change, write nothing
  --diff T    show the diff between the current file T and its regenerated form
  --force     overwrite a target whose content matches neither its own stamp nor the new output
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
  local i=0 tgt prov tpl body gen out mode rc budget plan="" refuse=0
  budget="$(mj_pol context.always_loaded_budget_lines)"
  while [ -n "$(mj_pol "projections.$i.target")" ]; do
    tgt="$(mj_pol "projections.$i.target")"; prov="$(mj_pol "projections.$i.provider")"
    mode="$(mj_projection_mode "$i")"
    case "$mode" in file|region) ;;
      *) rm -rf "$tmp"; mj_die "$MJ_EX_CONTRACT" "projection $tgt: unknown mode '$mode' (file | region)" ;;
    esac
    tpl="$(mj_provider_template "$prov")" || { rm -rf "$tmp"; mj_die "$MJ_EX_MISSING" "no template for provider '$prov' ($(mj_rel "$MJ_PROVIDERS_DIR")/$prov.tmpl, or one shipped in $MJ_PROVIDERS_DEFAULT_DIR/)"; }
    # body is the rendered content the stamp covers; gen is the generated content this
    # projection owns (the stamp plus the body in file mode, the body alone in region
    # mode); out is the whole file that would be written
    body="$tmp/body.$i"; gen="$tmp/gen.$i"; out="$tmp/out.$i"
    mj_render "$tpl" "$tmp" "$psha" > "$body"
    local new_c; new_c="$(mj_sha256 "$body")"
    if [ "$mode" = region ]; then gen="$body"
    else { mj_stamp_line "$psha" "$new_c"; cat "$body"; } > "$gen"; out="$gen"; fi

    # the budget measures what Majordomus generates, which for a region is the region
    # and not the host document it was appended to
    if [ "$(mj_pol "projections.$i.always_loaded")" = true ]; then
      if [ "$(mj_lines "$gen")" -gt "$budget" ]; then
        mj_fail budget "$tgt" "would be $(mj_lines "$gen") lines, budget $budget; nothing written" "majordomus update --dry-run"
        rm -rf "$tmp"; exit "$MJ_EX_CONTRACT"
      fi
    fi

    # what is on disk: the content this projection owns, and the hash its own stamp claims
    local have_c="" want_c="" present=0 cur="$tmp/cur.$i"
    : > "$cur"
    if [ "$mode" = region ]; then
      if [ -f "$MJ_ROOT/$tgt" ]; then
        rc=0; mj_region_extract "$MJ_ROOT/$tgt" > "$cur" 2>/dev/null || rc=$?
        case "$rc" in
          0) present=1; have_c="$(mj_sha256 "$cur")"; want_c="$(mj_stamp_read "$MJ_ROOT/$tgt" region | cut -d' ' -f2)" ;;
          1) ;;   # no region yet; update appends one
          *) mj_finding REFUSE projection "$tgt" "region markers are malformed (unclosed, out of order, or repeated); nothing written" "grep -n 'majordomus:begin\|majordomus:end' $tgt"
             refuse=1; i=$((i+1)); continue ;;
        esac
      fi
      mj_region_splice "$MJ_ROOT/$tgt" "$body" "${psha:0:12} ${new_c:0:16}" > "$out"
    elif [ -f "$MJ_ROOT/$tgt" ]; then
      present=1; mj_owned_content "$MJ_ROOT/$tgt" file > "$cur"; have_c="$(mj_sha256 "$cur")"
      want_c="$(mj_stamp_read "$MJ_ROOT/$tgt" file | cut -d' ' -f2)"
    fi

    if [ -n "$diff_target" ] && [ "$diff_target" = "$tgt" ]; then
      if [ "$mode" = region ]; then diff -u "$cur" "$body" 2>/dev/null || true
      else diff -u "$MJ_ROOT/$tgt" "$out" 2>/dev/null || true; fi
      rm -rf "$tmp"; return 0
    fi

    # hand-edit protection, on the content this projection owns: content that matches its
    # own stamp was generated and may be replaced; content that does not was edited by
    # somebody, and is never replaced without --force
    if [ "$present" = 0 ]; then plan="$plan create:$tgt"
    elif [ -f "$MJ_ROOT/$tgt" ] && [ "$(mj_sha256 "$MJ_ROOT/$tgt")" = "$(mj_sha256 "$out")" ]; then plan="$plan unchanged:$tgt"
    elif [ -n "$want_c" ] && [ "${have_c:0:16}" = "${want_c:0:16}" ]; then plan="$plan update:$tgt"
    elif [ "$force" = 1 ]; then plan="$plan update:$tgt"
    elif [ -z "$want_c" ]; then
      mj_finding REFUSE projection "$tgt" "carries no generation stamp; it was not written by update (hand-written?); use --diff $tgt, then --force" "majordomus update --diff $tgt"
      refuse=1
    else
      mj_finding REFUSE projection "$tgt" "current content matches neither its own stamp nor the new output (hand-edited?); use --diff $tgt, then --force" "majordomus update --diff $tgt"
      refuse=1
    fi
    i=$((i+1))
  done
  [ -n "$diff_target" ] && { rm -rf "$tmp"; mj_die "$MJ_EX_USAGE" "--diff: '$diff_target' is not a projection target"; }
  [ "$refuse" = 1 ] && { rm -rf "$tmp"; exit "$MJ_EX_REFUSED"; }
  [ "$i" = 0 ] && { rm -rf "$tmp"; mj_die "$MJ_EX_CONTRACT" "policy declares no projections"; }

  local p
  for p in $plan; do printf '%s %s\n' "${p%%:*}" "${p#*:}"; done
  if [ "$dry" = 1 ]; then rm -rf "$tmp"; return 0; fi

  # write atomically, every target
  i=0
  while [ -n "$(mj_pol "projections.$i.target")" ]; do
    tgt="$(mj_pol "projections.$i.target")"; mode="$(mj_projection_mode "$i")"
    out="$tmp/out.$i"; [ "$mode" = region ] || out="$tmp/gen.$i"
    mkdir -p "$(dirname "$MJ_ROOT/$tgt")"
    cp "$out" "$MJ_ROOT/$tgt.mj-tmp" && mv "$MJ_ROOT/$tgt.mj-tmp" "$MJ_ROOT/$tgt"
    i=$((i+1))
  done
  mkdir -p "$MJ_STATE_DIR"
  mj_ledger_append projections.updated "\"policy_sha256\":\"$psha\",\"targets\":$i"
  rm -rf "$tmp"
  printf 'generated %s target(s) from policy %s; each carries its own stamp\n' "$i" "${psha:0:12}"
}

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
