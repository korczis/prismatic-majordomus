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
# the skill validator reads the catalogue every skill surface reads
# shellcheck source=skills.sh
. "$MJ_LIB_DIR/skills.sh"
# the decision and knowledge validators read the catalogues their own commands read
# shellcheck source=session.sh
. "$MJ_LIB_DIR/session.sh"
# shellcheck source=adr.sh
. "$MJ_LIB_DIR/adr.sh"
# the capture doctrine and the provider states the wiring verifier reports
# shellcheck source=capture.sh
. "$MJ_LIB_DIR/capture.sh"
# shellcheck source=context.sh
. "$MJ_LIB_DIR/context.sh"
# the project-model validators read the one engine, not a copy of its rules
# shellcheck source=project.sh
. "$MJ_LIB_DIR/project.sh"
# the use-case validators: the catalogue's references and the coverage tally
# shellcheck source=usecase.sh
. "$MJ_LIB_DIR/usecase.sh"
# the deployment contract: the section's own closure, read without a Rust toolchain
# shellcheck source=deployment.sh
. "$MJ_LIB_DIR/deployment.sh"
# the bootstrap validator asks what update would render now, through update's own renderer
# shellcheck source=update.sh
. "$MJ_LIB_DIR/update.sh"

MJ_DOCTOR_MISSING=0
mj_cmd_doctor() {
  # what doctor reaches comes before its usage, and apart from the option loop (I2707)
  case " $* " in *" --help "*|*" -h "*) mj_doctor_reach ;; esac
  local a
  for a in "$@"; do case "$a" in
    --help|-h) echo "usage: majordomus doctor [--json]   (read-only; exit 0 healthy, 10 failures, 12 missing)"
               echo "  MAJORDOMUS_DOCTOR_CONTEXT=commit   the pre-commit hook's run: leaves out the per-worktree uncommitted-work reading, and the"
               echo "                                     context builder when nothing staged can change it, and says so"
               return 0 ;;
    *) mj_die "$MJ_EX_USAGE" "doctor: unknown option $a" ;;
  esac; done
  # The one context doctor knows. Anything else is refused rather than read as unset, so a
  # misspelt switch in a hook cannot quietly run, or quietly skip, what it meant to.
  case "${MAJORDOMUS_DOCTOR_CONTEXT:-}" in
    ''|commit) ;;
    *) mj_die "$MJ_EX_USAGE" "doctor: MAJORDOMUS_DOCTOR_CONTEXT='$MAJORDOMUS_DOCTOR_CONTEXT' is not a context (commit, or unset)" ;;
  esac
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
  mj_report_edit_claims
  mj_report_reasoning
  mj_report_layout
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
      else mj_doctrine_fail policy "$(mj_rel "$MJ_POLICY_FILE")" "unknown keys: $(printf '%s' "$unk" | tr '\n' ' ')" "grep -nE '$(printf '%s' "$unk" | sed 's/\..*//' | LC_ALL=C sort -u | tr '\n' '|' | sed 's/|$//')' $(mj_rel "$MJ_POLICY_FILE")"; fi
  else mj_doctrine_fail policy "$(mj_rel "$MJ_POLICY_FILE")" "unsupported version '$(mj_pol version)' (want 1)" \
    "grep -n '^version:' $(mj_rel "$MJ_POLICY_FILE")"; fi

  local pf n count=0
  for pf in "$MJ_PROFILES_DIR"/*.yaml; do
    [ -f "$pf" ] || continue; count=$((count+1)); n="$(basename "$pf" .yaml)"
    if mj_load_profile "$n" 2>/dev/null; then
      unk="$(mj_yaml_unknown_keys "$MJ_PRO_FLAT" "$MJ_ALLOW_DIR/profile.txt" || true)"
      [ -n "$unk" ] && mj_doctrine_fail profiles "$n" "unknown keys: $(printf '%s' "$unk" | tr '\n' ' ')" "cat $(mj_rel "$MJ_PROFILES_DIR")/$n.yaml"
      [ "$(mj_pro name)" = "$n" ] || mj_doctrine_fail profiles "$n" "name field '$(mj_pro name)' does not match filename" \
        "grep -n '^name:' $(mj_rel "$MJ_PROFILES_DIR")/$n.yaml"
    else mj_doctrine_fail profiles "$n" "does not parse" "cat $(mj_rel "$MJ_PROFILES_DIR")/$n.yaml"; fi
  done
  local def; def="$(mj_pol profiles.default)"
  if [ -f "$MJ_PROFILES_DIR/$def.yaml" ]; then mj_doctrine_ok profiles "$count files" "parsed; default '$def' exists"
  else mj_doctrine_fail profiles "default" "profiles.default='$def' has no file $(mj_rel "$MJ_PROFILES_DIR")/$def.yaml" \
    "ls $(mj_rel "$MJ_PROFILES_DIR")"; fi

  mj_profile_limits_reconcile
  return 0
}

# share/profile-limits.yaml against the profiles and lib/ (I0201): every declared axis is set
# on every profile, every declared adapter is a function that names its limit in a
# `# majordomus-limit: <limit>` line above it, and no function names a limit the file does
# not declare. Each profile's limits are then printed as data, one name=value per limit.
mj_profile_limits_reconcile() {
  local reg="$MJ_BIN_DIR/../share/profile-limits.yaml" rel="share/profile-limits.yaml"
  local flat tagged i id axis derive adapter ids="" pf n v line vals bad
  flat="$(mktemp "${TMPDIR:-/tmp}/mj.pl.XXXXXX")"
  if ! mj_yaml_flatten "$reg" > "$flat" 2>/dev/null; then
    mj_doctrine_fail profile-limits "$rel" "does not parse" "cat $rel"; rm -f "$flat"; return 0
  fi
  if [ "$(mj_yget "$flat" version)" != 1 ]; then
    mj_doctrine_fail profile-limits "$rel" "unsupported version '$(mj_yget "$flat" version)' (want 1)" "grep -n '^version:' $rel"
    rm -f "$flat"; return 0
  fi
  # file, limit, function: each function under lib/ with the marker lines directly above it
  tagged="$(awk '
    FNR == 1 { n = 0 }
    /^# majordomus-limit: / { t[n++] = $3; next }
    /^#/ { next }
    /^[a-z_][a-z0-9_]*\(\) *\{/ { f = $0; sub(/\(.*/, "", f); for (k = 0; k < n; k++) print FILENAME, t[k], f; n = 0; next }
    { n = 0 }' "$MJ_LIB_DIR"/*.sh)"
  i=0
  while id="$(mj_yget "$flat" "limits.$i.id")" && [ -n "$id" ]; do
    axis="$(mj_yget "$flat" "limits.$i.axis")"; derive="$(mj_yget "$flat" "limits.$i.derive")"
    adapter="$(mj_yget "$flat" "limits.$i.adapter")"; ids="$ids $id"
    case "$derive" in
      count|duration_seconds) ;;
      *) mj_doctrine_fail profile-limits "$id" "derive '$derive' is not count or duration_seconds" "grep -n -A4 'id: $id' $rel" ;;
    esac
    printf '%s\n' "$tagged" | awk -v l="$id" -v f="$adapter" '$2 == l && $3 == f { ok = 1 } END { exit ok ? 0 : 1 }' \
      || mj_doctrine_fail profile-limits "$id" "declares adapter $adapter, and no function $adapter under lib/ carries '# majordomus-limit: $id'" \
        "grep -rn -A1 'majordomus-limit: $id' lib/"
    i=$((i+1))
  done
  while read -r line; do
    [ -n "$line" ] || continue
    set -- $line
    case " $ids " in *" $2 "*) ;;
      *) mj_doctrine_fail profile-limits "$2" "$(mj_rel "$1"): $3 applies limit '$2', which $rel does not declare" "grep -n 'majordomus-limit: $2' $(mj_rel "$1")" ;;
    esac
  done <<EOF
$tagged
EOF
  for pf in "$MJ_PROFILES_DIR"/*.yaml; do
    [ -f "$pf" ] || continue; n="$(basename "$pf" .yaml)"
    mj_load_profile "$n" 2>/dev/null || continue
    vals=""; bad=0; i=0
    while id="$(mj_yget "$flat" "limits.$i.id")" && [ -n "$id" ]; do
      axis="$(mj_yget "$flat" "limits.$i.axis")"; derive="$(mj_yget "$flat" "limits.$i.derive")"
      v="$(mj_pro "$axis")"
      if [ -z "$v" ]; then
        mj_doctrine_fail profile-limits "$n" "does not set $axis, from which $rel derives $id" "cat $(mj_rel "$pf")"; bad=1
      else
        case "$derive" in
          duration_seconds) v="$(mj_duration_secs "$v")" || v="" ;;
          count) case "$v" in *[!0-9]*) v="" ;; esac ;;
        esac
        if [ -z "$v" ]; then
          mj_doctrine_fail profile-limits "$n" "$axis: '$(mj_pro "$axis")' is not a $derive value" "grep -n '$(printf '%s' "$axis" | sed 's/.*\.//'):' $(mj_rel "$pf")"; bad=1
        else vals="$vals $id=$v"; fi
      fi
      i=$((i+1))
    done
    [ "$bad" = 1 ] || mj_doctrine_ok profile-limits "$n" "limits${vals}"
  done
  rm -f "$flat"
}

# A linked worktree runs the hooks of its own tree (I2718). The repository's policy is the
# primary checkout's core.hooksPath; when it names a directory of the primary checkout — an
# absolute <primary>/.githooks in the configuration every worktree shares — git sends every
# linked worktree to the primary's hooks, and a stale primary then judges every commit with
# stale gates. That is a finding, with git's own per-worktree remedy, and the wiring below
# judges the worktree's own directory (MJ_WIRING_HOOKDIR) rather than the one git would run.
# The primary checkout, a bare repository, a repository with no core.hooksPath, one whose
# hooks live outside every checkout, and a tree without the directory are left as they are.
mj_wiring_own_hooks() {
  MJ_WIRING_HOOKDIR=""
  local gd common primary line policy rel own runs fix=""
  gd="$(mj_git rev-parse --path-format=absolute --git-dir 2>/dev/null)" || return 0
  common="$(mj_git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" || return 0
  [ -n "$gd" ] && [ "$gd" != "$common" ] || return 0
  primary="$(mj_git worktree list --porcelain 2>/dev/null | sed -n '1s/^worktree //p')"
  [ -n "$primary" ] && [ -d "$primary" ] || return 0
  mj_count git; line="$(git -C "$primary" config --show-scope --get core.hooksPath 2>/dev/null)" || return 0
  case "$line" in local"$MJ_TAB"*|worktree"$MJ_TAB"*) policy="${line#*"$MJ_TAB"}" ;; *) return 0 ;; esac
  primary="$(cd "$primary" && pwd -P)"
  case "$policy" in
    "") return 0 ;;
    /*) [ -d "$policy" ] && policy="$(cd "$policy" && pwd -P)"
        case "$policy" in "$primary"/*) rel="${policy#"$primary"/}" ;; *) return 0 ;; esac ;;
    *) rel="$policy" ;;
  esac
  own="$MJ_ROOT/$rel"
  [ -d "$own" ] || return 0
  own="$(cd "$own" && pwd -P)"
  runs="$(mj_git rev-parse --path-format=absolute --git-path hooks 2>/dev/null)" || return 0
  [ -d "$runs" ] && runs="$(cd "$runs" && pwd -P)"
  [ "$runs" = "$own" ] && return 0
  MJ_WIRING_HOOKDIR="$own"
  if [ "$(mj_git config --local --type=bool --get extensions.worktreeConfig 2>/dev/null)" != true ]; then
    case "$(mj_git config --local --get core.repositoryformatversion 2>/dev/null)" in
      ""|0) fix="git -C $MJ_ROOT config core.repositoryformatversion 1 && " ;;
    esac
    fix="${fix}git -C $MJ_ROOT config extensions.worktreeConfig true && "
  fi
  fix="${fix}git -C $MJ_ROOT config --worktree core.hooksPath $own"
  mj_doctrine_fail wiring "hooks-path" "this linked worktree runs $runs, not its own $own, so every commit here is judged by another checkout's hooks; fix: $fix (or majordomus worktree repair, which binds every linked worktree)" \
    "git -C $MJ_ROOT rev-parse --path-format=absolute --git-path hooks"
}

mj_validate_wiring() {
  local i=0 name path wired kind target hookdir hookfile arg0 prog
  mj_wiring_own_hooks
  while [ -n "$(mj_pol "enforcement.$i.name")" ]; do
    name="$(mj_pol "enforcement.$i.name")"; path="$(mj_pol "enforcement.$i.path")"; wired="$(mj_pol "enforcement.$i.wired_by")"
    arg0="$(mj_pol "enforcement.$i.args.0")"; [ -z "$arg0" ] && arg0="$(mj_pol "enforcement.$i.args" | sed 's/\[\]//')"
    # The program the entry names, not an assumption that it is this tool. A repository has
    # gates of its own — a fingerprint check, a linter — and the point of declaring one here
    # is that doctor proves it is wired; a verifier that only recognises `majordomus` would
    # report every one of them as unwired and teach people to stop declaring them.
    prog="$(basename "$path")"
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
          # Git says where its hooks are: core.hooksPath when set, else the common
          # directory's hooks/. A linked worktree's .git is a file naming that directory,
          # so "$MJ_ROOT/.git/hooks" does not exist there and every hook read as missing.
          # A linked worktree sent to another checkout's hooks is judged by its own, which
          # is what it will run once the finding above is fixed.
          hookdir="$MJ_WIRING_HOOKDIR"
          [ -z "$hookdir" ] && hookdir="$(mj_git rev-parse --git-path hooks 2>/dev/null || true)"
          [ -z "$hookdir" ] && hookdir=".git/hooks"
          case "$hookdir" in /*) hookfile="$hookdir/$target" ;; *) hookfile="$MJ_ROOT/$hookdir/$target" ;; esac
          # A hook is commonly a dispatcher that runs every executable in <hook>.d/; the
          # invocation then lives in one of those files, not in the hook git calls.
          local wirefile="" cand rel
          if [ -f "$hookfile" ]; then
            for cand in $(mj_hook_candidates "$hookfile"); do
              if grep -qE "${prog}[[:space:]]+$arg0([[:space:]]|$)" "$cand"; then wirefile="$cand"; break; fi
            done
          fi
          rel="${wirefile#"$MJ_ROOT"/}"
          if [ ! -f "$hookfile" ]; then mj_doctrine_fail wiring "$name" "hook $hookdir/$target does not exist" "ls -l $hookdir/$target"
          elif [ ! -x "$hookfile" ]; then mj_doctrine_fail wiring "$name" "hook $hookdir/$target is not executable" "chmod +x $hookdir/$target"
          elif [ -z "$wirefile" ]; then
            mj_doctrine_fail wiring "$name" "$prog $arg0 is not invoked by $hookdir/$target or anything in $hookdir/$target.d/" "grep -rn '$prog $arg0' $hookdir/$target $hookdir/$target.d 2>/dev/null"
          elif [ ! -x "$wirefile" ]; then
            mj_doctrine_fail wiring "$name" "$rel invokes it but is not executable, so the dispatcher skips it" "chmod +x $rel"
          elif [ -z "$resolved" ] && ! mj_hook_binary_ok "$wirefile" "$arg0" "$prog"; then
            mj_doctrine_fail wiring "$name" "'$path' is not on PATH and $rel does not name an executable $prog" "grep -n '$prog $arg0' $rel"
          elif grep -E "${prog}[[:space:]]+$arg0" "$wirefile" | grep -qE '\|\|[[:space:]]*(true|exit[[:space:]]+0)'; then
            mj_doctrine_fail wiring "$name" "$rel invokes it but swallows the exit code (|| true)" "grep -n '$prog $arg0' $rel"
          else mj_doctrine_ok wiring "$name" "wired via $rel"; fi ;;
        git-config)
          # A gate that lives in the clone's own configuration rather than in a tracked file.
          # `.gitattributes` can say `merge=derived`, but the driver it names is declared per
          # clone and per worktree, and git falls back to the default text merge in silence
          # when it is not — which is the failure this exists to catch: the repository looks
          # configured, every file is committed, and the driver never runs. The value has to
          # name the tool the entry declares, or it is some other driver wearing the name.
          local cfg; cfg="$(mj_git config --get "$target" 2>/dev/null || true)"
          if [ -z "$cfg" ]; then
            mj_doctrine_fail wiring "$name" "git config $target is not set in this clone, so git merges $prog's paths with the default driver and conflicts on every one" "just derive-merge-driver"
          elif ! printf '%s' "$cfg" | grep -qF "$prog"; then
            mj_doctrine_fail wiring "$name" "git config $target is set but does not name $prog: '$cfg'" "just derive-merge-driver"
          elif [ -z "$resolved" ]; then
            mj_doctrine_fail wiring "$name" "git config $target names '$path' and it is not an executable here" "ls -l $path"
          else mj_doctrine_ok wiring "$name" "declared in this clone as $target"; fi ;;
        ci)
          if [ ! -f "$MJ_ROOT/$target" ]; then mj_doctrine_fail wiring "$name" "ci file $target does not exist" \
            "grep -n 'wired_by' $(mj_rel "$MJ_POLICY_FILE")"
          elif ! grep -qE "${prog}[[:space:]]+$arg0" "$MJ_ROOT/$target"; then mj_doctrine_fail wiring "$name" "$target does not invoke $prog $arg0" "grep -n $prog $target"
          else mj_doctrine_ok wiring "$name" "invoked from $target"; fi ;;
        provider-hook)
          # A hook that exists is not a hook that captures. The state comes from driving a
          # synthetic payload through the shim the provider would run, so "wired" and
          # "verified" stay different words with different evidence behind them.
          #
          # A provider is wired for more than one thing, so the target may name which:
          # provider-hook:<provider> is the prompt archive, provider-hook:<provider>:session
          # the episode boundary. They fail independently and are therefore declared
          # independently — a repository that captures every prompt and never marks an
          # episode must not read as wired.
          local cst creason cprov="$target" caspect=prompt
          case "$target" in *:*) cprov="${target%%:*}"; caspect="${target#*:}" ;; esac
          case "$caspect" in
            prompt|session)
              cst="$(mj_capture_state "$cprov" "$caspect")"; creason="${cst#*"$MJ_TAB"}"; cst="${cst%%"$MJ_TAB"*}"
              if [ "$cst" = verified ]; then mj_doctrine_ok wiring "$name" "$creason"
              else mj_doctrine_fail wiring "$name" "$cst — $creason" "majordomus capture status"; fi ;;
            *) mj_doctrine_fail wiring "$name" "unknown provider-hook aspect '$caspect' (provider-hook:<provider> | provider-hook:<provider>:session)" \
                 "grep -n 'wired_by' $(mj_rel "$MJ_POLICY_FILE")" ;;
          esac ;;
        toolchain)
          # A gate whose script builds the executable cannot be wired by a file: the thing it
          # needs is on the machine, and nothing in git says so. `scripts/derive` (and with it
          # `majordomus prs repair --apply`, the pre-commit derived-current gate and the
          # integration executor) builds the Rust executable, and on a machine with no cargo
          # it fails late — after the merge, after the lock — with `env: cargo: No such file`.
          # On 2026-10-10 nine repairs were refused that way on one clone while the other
          # two machines of the mesh held the toolchain. The target names the file that pins
          # the channel, so the toolchain doctor asks for is the toolchain CI uses.
          local tfile="$MJ_ROOT/$target" tchan tcargo tver
          tchan="$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$tfile" 2>/dev/null | head -1)"
          tcargo="$(command -v cargo 2>/dev/null || true)"
          [ -z "$tcargo" ] && [ -x "$HOME/.cargo/bin/cargo" ] && tcargo="$HOME/.cargo/bin/cargo"
          if [ ! -f "$tfile" ]; then
            mj_doctrine_fail wiring "$name" "toolchain file $target does not exist" "grep -n 'wired_by' $(mj_rel "$MJ_POLICY_FILE")"
          elif [ -z "$tchan" ]; then
            mj_doctrine_fail wiring "$name" "$target pins no channel, so the toolchain this machine builds with is nobody's decision" "grep -n channel $target"
          elif [ -z "$tcargo" ]; then
            mj_doctrine_fail wiring "$name" "cargo is not installed on this machine, so $prog cannot build the executable and every derive and \`prs repair --apply\` here fails" \
              "install rustup from https://rustup.rs (its minimal profile); it then reads $target"
          else
            # rustup keeps the pinned channel as its own installation; asking it is the only
            # answer that does not trigger an install as a side effect of the question.
            local trustup; trustup="$(dirname "$tcargo")/rustup"; [ -x "$trustup" ] || trustup="$(command -v rustup 2>/dev/null || true)"
            if [ -n "$trustup" ]; then
              if "$trustup" toolchain list 2>/dev/null | grep -q "^${tchan}\b"; then
                mj_doctrine_ok wiring "$name" "pinned toolchain $tchan installed ($tcargo)"
              else mj_doctrine_fail wiring "$name" "cargo is present but the pinned toolchain $tchan is not installed, and a derive here would fetch it mid-merge" "rustup toolchain install $tchan"; fi
            else
              tver="$("$tcargo" --version 2>/dev/null | awk '{print $2}')"
              if [ "$tver" = "$tchan" ]; then mj_doctrine_ok wiring "$name" "cargo $tver is the pinned $tchan ($tcargo)"
              else mj_doctrine_fail wiring "$name" "cargo is $tver and $target pins $tchan; without rustup the pin cannot be fetched" "install rust $tchan"; fi
            fi
          fi ;;
        manual) mj_doctrine_skip wiring "$name" "wired_by: manual — documented, not verified" ;;
        *) mj_doctrine_fail wiring "$name" "unknown wired_by kind '$kind' (git-hook:<name> | git-config:<key> | ci:<path> | provider-hook:<provider>[:session] | toolchain:<file> | manual)" \
             "grep -n 'wired_by' $(mj_rel "$MJ_POLICY_FILE")" ;;
      esac
    fi
    i=$((i+1))
  done
  [ "$i" = 0 ] && mj_doctrine_fail wiring "policy" "no enforcement entries declared; nothing is wired to run majordomus" \
    "grep -n 'enforcement:' $(mj_rel "$MJ_POLICY_FILE")"

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
    # Under a personal layer a target git tracks is the project's own file. It was not
    # written by update and will not be, so it is not held to a stamp it never had.
    if mj_personal_is_the_projects "$tgt"; then
      mj_doctrine_skip projection "$tgt" "tracked, so it is the project's; a personal layer leaves it alone (point this projection at a file of your own)"
      j=$((j+1)); continue
    fi
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
#
# Both questions are asked of the generated content only: the whole file in file mode, the
# region in region mode. The host document around a region is the repository's own, written
# by people before or beside the layer; it may carry rule bullets and a rules section, and no
# edit of the policy could remove them, so a finding there could never be fixed by what
# doctor tells the reader to run. The pointer is read from the same span for the same reason
# turned round: the region is what a worker is promised, and a host that happens to name
# .ai/README.md does not stand in for a region that lost it. Absent or malformed markers are
# the projection doctrine's finding and nothing is judged here.
mj_validate_bootstrap() {
  local j=0 tgt mode bad=0 owned region
  region="$(mktemp "${TMPDIR:-/tmp}/mj.bs.XXXXXX")"
  if mj_personal_active; then
    # The chain starts at the project's README, and a personal layer changes nothing the
    # project shares: the README is the repository's, the layer is one person's. The agent
    # finds AGENTS.md beside it all the same.
    mj_doctrine_skip bootstrap "README.md" "not asked to name AGENTS.md: the layer is personal, and the README is the project's"
  elif [ -f "$MJ_ROOT/README.md" ]; then
    if grep -q 'AGENTS\.md' "$MJ_ROOT/README.md"; then mj_doctrine_ok bootstrap "README.md" "names AGENTS.md"
    else mj_doctrine_fail bootstrap "README.md" "does not name AGENTS.md; a reader cannot find the agent bootstrap" "grep -n AGENTS.md README.md"; bad=1; fi
  fi
  while [ -n "$(mj_pol "projections.$j.target")" ]; do
    tgt="$(mj_pol "projections.$j.target")"; mode="$(mj_projection_mode "$j")"; j=$((j+1))
    [ -f "$MJ_ROOT/$tgt" ] || continue   # projection_integrity reports absence
    mj_personal_is_the_projects "$tgt" && continue   # the project's file, like its README
    owned="$MJ_ROOT/$tgt"
    if [ "$mode" = region ]; then
      mj_region_extract "$MJ_ROOT/$tgt" > "$region" 2>/dev/null || continue   # projection_integrity reports the markers
      owned="$region"
    fi
    if ! grep -q '\.ai/README\.md' "$owned"; then
      mj_doctrine_fail bootstrap "$tgt" "does not point at .ai/README.md; a worker reading it never reaches the layer" "grep -n '.ai/README.md' $tgt"; bad=1
    elif grep -qE '^\| *`?(profile|routine|implementation)`? *\||^- \*\*[A-Za-z].*\*\*|^### (Rules|Ten rules|Lifecycle|Finish contract)' "$owned"; then
      mj_doctrine_fail bootstrap "$tgt" "carries a rule corpus of its own (a profile table, rule bullets or a rules section); rules live under .ai/repo/rules/" "grep -nE '^- \*\*|^### ' $tgt"; bad=1
    fi
    mj_bootstrap_rule_refs "$tgt" "$mode" "$(mj_pol "projections.$((j-1)).provider")" || bad=1
  done
  rm -f "$region"
  [ "$bad" = 0 ] && [ "$j" -gt 0 ] && mj_doctrine_ok bootstrap "$j projection(s)" "each points at .ai/README.md, carries no rule of its own and names only rules in force"
  return 0
}

# A generated file that names a rule the repository does not have fails for the reason one
# that carries a rule of its own does: the rule exists in that file and nowhere else, and a
# worker who reads it goes looking for a rule that is not there. So every rule id the
# generated content names resolves in the effective set, as `majordomus rules list` lists it.
# The links check beside the budget is the same guard for paths; this one is for rule ids,
# which are not links and which it therefore never saw.
#
# The generated content only — the whole file in file mode, the region in region mode. The
# host document around a region is the repository's own prose, which no generator promised
# anything about (the budget doctrine judges the same span for the same reason), and prose
# says `project.json` often enough that reading it would teach people to ignore the finding.
# A malformed region is the projection doctrine's finding and is not read here. Line numbers
# are the file's, so a reader can go to them.
#
# The grade is whether `majordomus update` fixes it, because in an adopter the pre-commit
# hook is doctor, and a FAIL that appears on upgrade would refuse every commit until the
# person found out what to run:
#   WARN  the content still matches its stamp — an older template wrote it — and what update
#         renders from the current template no longer names the id: a stale bootstrap, and
#         `majordomus update` is the whole fix. Under watch, drift.
#   FAIL  the current template names the id itself, so update would write it again; or the
#         content no longer matches its stamp, so somebody wrote the line and update refuses
#         to replace it.
# What update would render is asked of update's own renderer (mj_render_current), never
# rebuilt here.
#
# mj_bootstrap_rule_refs TARGET MODE PROVIDER -> 0 when every reference resolves, 1 after
# reporting each
mj_bootstrap_rule_refs() {
  local tgt="$1" mode="$2" prov="$3" line ref found rendered current="" st tpl lvl bad=0
  [ "$mode" = region ] && { mj_region_extract "$MJ_ROOT/$tgt" >/dev/null 2>&1 || return 0; }
  found="$(mj_rule_refs_unresolved "$MJ_ROOT/$tgt" "$mode")"
  [ -n "$found" ] || return 0
  # the ids the current template would write again, each once, space-delimited; in a
  # subshell, because the renderer loads every profile and doctor keeps its own state
  rendered="$(mktemp "${TMPDIR:-/tmp}/mj.rr.XXXXXX")"
  if ( mj_render_current "$prov" ) > "$rendered" 2>/dev/null; then
    current=" $(mj_rule_refs_unresolved "$rendered" file | cut -f2 | LC_ALL=C sort -u | tr '\n' ' ')"
  fi
  rm -f "$rendered"
  st="$(mj_projection_status "$tgt" "$mode" "$prov")"; st="${st%% *}"
  tpl="$(mj_provider_template "$prov" 2>/dev/null || true)"; tpl="$(mj_rel "$tpl")"
  while IFS="$MJ_TAB" read -r line ref; do
    bad=1
    case "$current" in
      *" $ref "*)
        mj_doctrine_fail rule-refs "$tgt" \
          "$tgt line $line names rule $ref, which is not in the effective rule set, and the current template $tpl names it too, so majordomus update would write it again (add the rule under .ai/repo/rules/project/, or drop it from the template)" \
          "sed -n '${line}p' $tgt; majordomus rules show ${ref%@*}"
        continue ;;
    esac
    if [ "$st" = ok ]; then
      if [ "$MJ_DOCTRINE_CMD" = watch ]; then lvl=mj_drift; else lvl=mj_warn; fi
      "$lvl" rule-refs "$tgt" \
        "stale bootstrap: $tgt line $line names rule $ref, which is not in the effective rule set; an older template wrote it and the current one does not, so run majordomus update" \
        "majordomus update"
    else
      mj_doctrine_fail rule-refs "$tgt" \
        "$tgt line $line names rule $ref, which is not in the effective rule set, in content that no longer matches its stamp, so somebody wrote it and majordomus update will not replace it (add the rule under .ai/repo/rules/project/, or remove the line)" \
        "sed -n '${line}p' $tgt; majordomus rules show ${ref%@*}"
    fi
  done <<< "$found"
  return "$bad"
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
  elif [ -z "$always" ]; then mj_doctrine_fail budget "policy" "no projection is marked always_loaded: true" \
    "grep -n 'always_loaded' $(mj_rel "$MJ_POLICY_FILE")"; fi
  rm -f "$owned"

  # In the commit hook the builder is proved only when the commit can change what it reads
  # (I4265): it composes the whole context, about a quarter of an adopter's doctor run, and a
  # commit that stages nothing under .ai/ or in a provider file cannot break it. Said by name,
  # with the command that runs it, because an absent check must not read as a passing one.
  if [ "${MAJORDOMUS_DOCTOR_CONTEXT:-}" = commit ] \
     && ! mj_git diff --cached --name-only 2>/dev/null | grep -qE '^(\.ai/|AGENTS\.md$|CLAUDE\.md$|GEMINI\.md$)'; then
    mj_info context "builder" "not read in the commit hook (MAJORDOMUS_DOCTOR_CONTEXT=commit): nothing staged under .ai/ or in a provider file can change it; run doctor on its own" "majordomus doctor"
    return 0
  fi
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
  # The sessions section is tracked and shared where the three above are local, and it is
  # capped for the same reason: a store nothing bounds grows until a reader pays for it. A
  # layer that declares no sessions section owes no cap for one. What `session archive`
  # moved aside is not counted — that is what the command is for.
  if [ -n "${MJ_SESSIONS_DIR:-}" ]; then
  if ! cap="$(mj_pol_req session.retention_max_files)"; then mj_doctrine_fail retention "sessions" "policy declares no session.retention_max_files" "add it under session: in $(mj_rel "$MJ_POLICY_FILE"); see share/skeleton/policy.yaml"; else
  hc="$(mj_session_record_names | wc -l | tr -d ' ')"
  if [ "$hc" -le "$cap" ]; then mj_doctrine_ok retention "sessions" "$hc records, cap $cap"; else mj_doctrine_fail retention "sessions" "$hc records over cap $cap" "majordomus session archive --dry-run"; fi; fi
  fi

  return 0
}

# Reasoning (ADR 0098). Reasoning is operational with no advisor at all, so absence of an
# optional advisor is reported as information — never a warning, never a failure — and the
# independent review capacity is a count. What does fail is a finding of `reasoning check`:
# provider-independent code that names an advisor, CI that names a model credential, a
# record claiming a review nobody planned. Read through the executable, never rebuilt here.
# The CI and claims checks judge what this repository's own workflows and documents say,
# so the executable runs them only where project.advisors-are-optional is declared; an
# adopter is told so, as information (I1996). Every hint names a command an adopter has.
mj_report_reasoning() {
  # shellcheck source=rust_bin.sh
  . "$MJ_LIB_DIR/rust_bin.sh"
  local bin share advisors check
  bin="$(mj_rust_bin "$MJ_HOME")"
  if [ ! -x "$bin" ]; then
    mj_info reasoning "-" "the executable is not built, so advisor availability is not reported here (majordomus reasoning advisors)"
    return 0
  fi
  share="$(mj_rust_share "$MJ_ROOT")"
  advisors="$( ( [ -z "$share" ] || export MAJORDOMUS_SHARE="$share"
                 "$bin" reasoning advisors --repo "$MJ_ROOT" --format json ) 2>/dev/null )" || {
    mj_info reasoning "-" "advisor availability could not be read (majordomus reasoning advisors)"; return 0; }
  mj_ok reasoning "share/advisors.yaml" "$(printf '%s' "$advisors" | jq -r '"operational, mode \(.mode.mode) (\(.mode.source)); independent review capacity \(.available) advisor(s)"')"
  printf '%s' "$advisors" | jq -r '.advisors[] | [.id, .status, .reason] | @tsv' | \
    while IFS="$(printf '\t')" read -r id status reason; do
      mj_info advisor "$id" "$status ($reason) — optional"
    done
  # exit 10 is the check's verdict on a finding, not a failure to run: the document is read
  # either way, and an empty one is the failure to run
  check="$( ( [ -z "$share" ] || export MAJORDOMUS_SHARE="$share"
              "$bin" reasoning check --repo "$MJ_ROOT" --format json ) 2>/dev/null )" || true
  if [ -z "$check" ]; then
    mj_info reasoning "-" "reasoning check could not run (majordomus reasoning check)"
  elif [ "$(printf '%s' "$check" | jq -r '.ok')" = true ]; then
    mj_ok reasoning "-" "reasoning check: $(printf '%s' "$check" | jq -r '.checks | join(", ")') — no finding"
  else
    printf '%s' "$check" | jq -r '.findings[] | [.check, .subject, .message] | @tsv' | \
      while IFS="$(printf '\t')" read -r c subject message; do
        mj_fail reasoning "$subject" "$c: $message" "majordomus reasoning check"
      done
  fi
  if [ -n "$check" ] && [ "$(printf '%s' "$check" | jq -r '.checks | index("ci") == null')" = true ]; then
    mj_info reasoning "-" "ci, claims: not run; they judge Majordomus's own workflows and documents and apply where project.advisors-are-optional is declared (majordomus reasoning check)"
  fi
  return 0
}

# The boundary between the layer's halves, judged by the executable against the layouts their
# contracts declare (I2871): a tracked file the ignore rules match under a contract that
# excludes runtime state, or a copy of authored knowledge in the local half. Exit 10 is the
# check's verdict on a finding, not a failure to run; an empty answer is.
mj_report_layout() {
  # shellcheck source=rust_bin.sh
  . "$MJ_LIB_DIR/rust_bin.sh"
  local bin share report
  bin="$(mj_rust_bin "$MJ_HOME")"
  if [ ! -x "$bin" ]; then
    mj_info layout "-" "the executable is not built, so the layout boundary is not judged here (majordomus layout check)"
    return 0
  fi
  share="$(mj_rust_share "$MJ_ROOT")"
  report="$( ( [ -z "$share" ] || export MAJORDOMUS_SHARE="$share"
               "$bin" layout check --repo "$MJ_ROOT" --format json ) 2>/dev/null )" || true
  if [ -z "$report" ] || [ "$(printf '%s' "$report" | jq -r '.measured')" != true ]; then
    mj_info layout "-" "the layout boundary could not be judged (majordomus layout check)"
  elif [ "$(printf '%s' "$report" | jq -r '.passes')" = true ]; then
    mj_ok layout "-" "$(printf '%s' "$report" | jq -r '"layout check: \(.checked) path(s) judged against their declared layouts; no finding"')"
  else
    printf '%s' "$report" | jq -r '.findings[] | [.path, .contract, .excludes, .reason] | @tsv' | \
      while IFS="$(printf '\t')" read -r path contract excludes reason; do
        mj_fail layout "$path" "violates $contract: excludes $excludes; $reason" "majordomus layout check"
      done
  fi
  return 0
}

mj_report_environment() {
  local env="bash ${BASH_VERSION%%(*}"; env="$env, git $(git --version | awk '{print $3}')"
  mj_has jq && env="$env, jq $(jq --version 2>/dev/null | sed 's/jq-//')" || env="$env, jq absent"
  mj_has shellcheck && env="$env, shellcheck present"
  mj_info env "-" "$env"
  mj_report_clone
  return 0
}

# The claim-coverage report (I3322): which sessions edited a file no claim covered, recently.
#
# The edit guard is wired into the provider's pre-tool hook, and while the policy's
# `session.edit_guard_mode` is `advisory` it refuses nothing: it says the edit was unclaimed
# and appends one line — time, session, path — to .ai/local/state/guard/unclaimed.log in the
# checkout the file belongs to. Refusal (I3323) is turned on when this report names nobody,
# because refusing while it names somebody stops that somebody mid-edit. So the report is
# read the way the guard writes it: every checkout of the repository — a session started in
# one checkout edits files of the others, and each records its own — and a session counts
# while its newest unclaimed edit is younger than `session.stranded_after`, the policy's one
# word for how long a silent session is still taken to be there. Older lines are history and
# are not read as somebody editing now.
#
# Its reach is this machine: the records are each checkout's local state, and a session on
# another machine of the mesh is reported by that machine's doctor. It is a report and never
# a failure: an unclaimed edit is what advisory mode exists to let through.
mj_report_edit_claims() {
  local window secs now cutoff wt rows=""
  if [ "$(mj_pol session.edits_require_claim)" != true ]; then
    mj_info guard "claim coverage" "session.edits_require_claim is off, so the edit guard judges no edit here"
    return 0
  fi
  window="$(mj_pol session.stranded_after)"
  secs="$(mj_duration_secs "$window" 2>/dev/null)" || secs=""
  if [ -z "$secs" ]; then
    mj_info guard "claim coverage" "not read: the policy declares no session.stranded_after, which is what says a session is still there" "majordomus doctor"
    return 0
  fi
  now="$(mj_epoch "$(mj_now)")" || now=""
  [ -n "$now" ] || { mj_info guard "claim coverage" "not read: the clock could not be read as an epoch"; return 0; }
  cutoff=$((now - secs))
  cutoff="$(date -u -r "$cutoff" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u -d "@$cutoff" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null)"
  # every checkout of the repository, this one included even where git lists no worktree,
  # each spelt physically so that one checkout reached by two spellings is read once
  local here; here="$(cd "$MJ_ROOT" && pwd -P)"
  while IFS= read -r wt; do
    [ -f "$wt/.ai/local/state/guard/unclaimed.log" ] || continue
    # RFC 3339 instants in UTC compare as strings; a line that is not three fields is skipped
    rows="$rows$(awk -F '\t' -v c="$cutoff" -v w="$wt" 'NF == 3 && $1 >= c { printf "%s\t%s\t%s\t%s\n", $2, $1, $3, w }' \
      "$wt/.ai/local/state/guard/unclaimed.log" 2>/dev/null)
"
  done <<EOF
$( { printf '%s\n' "$MJ_ROOT"; git -C "$MJ_ROOT" worktree list --porcelain 2>/dev/null | sed -n 's/^worktree //p'; } \
  | while IFS= read -r wt; do (cd "$wt" 2>/dev/null && pwd -P); done | awk '!seen[$0]++')
EOF
  if [ -z "$(printf '%s' "$rows" | tr -d '\n')" ]; then
    mj_ok guard "claim coverage" "no session edited a file without a claim in the last $window (session.edit_guard_mode: $(mj_pol session.edit_guard_mode | grep . || echo advisory))"
    return 0
  fi
  # one line per session: how many unclaimed edits, the newest, and where
  local session count newest path checkout
  while IFS="$(printf '\t')" read -r session count newest path checkout; do
    [ -n "$session" ] || continue
    mj_warn guard "$session" \
      "$count unclaimed edit(s) in the last $window, the newest at $newest: $path in $([ "$checkout" = "$here" ] && printf "this checkout" || printf "%s" "$checkout")" \
      "majordomus mesh claim $path --session $session --advisory"
  done <<EOF
$(printf '%s' "$rows" | awk -F '\t' 'NF == 4 { n[$1]++; if ($2 >= t[$1]) { t[$1] = $2; p[$1] = $3; w[$1] = $4 } }
  END { for (s in n) printf "%s\t%s\t%s\t%s\t%s\n", s, n[s], t[s], p[s], w[s] }' | LC_ALL=C sort)
EOF
  return 0
}

# Two facts about this clone that no gate over a tree can see, because they are not in the
# tree. Both were learned the hard way in one afternoon of eight sessions sharing a
# repository, and both cost more than any defect in the code did.
mj_report_clone() {
  git -C "$MJ_ROOT" rev-parse --git-dir >/dev/null 2>&1 || return 0

  # The derived merge driver is NOT checked here: `wiring/derived-merge-driver` already
  # decides it, and a second check of one fact is the defect this repository spends its
  # time removing. It was written here first and removed on reading doctor's own output.
  #
  # Work that exists on one disk. A branch whose commits reach no remote is not backed up,
  # is invisible to every other session, and cannot be folded by whoever is integrating —
  # fifteen commits over sixty-eight files sat in exactly that state for three days.
  #
  # One walk, whatever the number of branches (I2993): the commits some branch reaches and no
  # remote does, each with its parents, then every branch's tip. A branch's count is the
  # commits of that set its tip reaches, walked inside the set: a commit a remote reaches has
  # only ancestors a remote reaches, so the walk never has to leave it. The commits a walk has
  # seen are the branch's, not its tip's: two branches at one commit are counted alike. One `rev-list` per
  # branch took minutes on a clone of six hundred branches, in every commit hook.
  local unpushed
  unpushed="$({
    git -C "$MJ_ROOT" rev-list --parents --branches --not --remotes 2>/dev/null
    echo '--'
    git -C "$MJ_ROOT" for-each-ref --format='%(objectname) %(refname:short)' refs/heads 2>/dev/null
  } | awk '
    $0 == "--" { tips = 1; next }
    !tips { held[$1] = 1; parents[$1] = $0; next }
    ($1 in held) {
      count = 0; top = 1; stack[1] = $1
      while (top > 0) {
        c = stack[top--]
        if (!(c in held) || ((NR, c) in seen)) continue
        seen[NR, c] = 1; count++
        m = split(parents[c], p, " ")
        for (j = 2; j <= m; j++) stack[++top] = p[j]
      }
      printf " %s(%d)", $2, count
    }')"
  if [ -n "$unpushed" ]; then
    mj_warn clone "unpushed" \
      "commits on no remote:${unpushed} — work on one disk is invisible to every other session and cannot be integrated" \
      "git push -u origin <branch>"
  else
    mj_ok clone "unpushed" "every local commit is on a remote"
  fi

  # Records a trunk episode staged and no branch has carried yet (ADR 0091, amendment of
  # 2026-10-15). Named, not judged: the outbox is the trunk's correct place for them, and a
  # clone that never branches again must still say what it holds. Silent when empty.
  local ob os oc
  ob="$(mj_outbox_counts)"; os="${ob% *}"; oc="${ob#* }"
  if [ "$os" != 0 ] || [ "$oc" != 0 ]; then
    mj_info clone "outbox" \
      "$os session record(s) and $oc knowledge candidate(s) wait in $(mj_rel "$(mj_outbox_dir)")/ for a branch episode to carry them" \
      "ls $(mj_rel "$(mj_outbox_dir)")/*/"
  fi
  mj_report_uncommitted
  mj_report_trunk_distance
  mj_report_release_currency
}

# Whether the trunk's landed work is released (release.currency, I3070; the owner's order of
# 2026-10-19: a measured version after every session and milestone). The measurement is the
# executable's, read here and judged nowhere else. Current is OK. Owed work, landed and
# waiting for the release seat, is a WARN, and overdue, past the six hours it may wait, is a
# WARN saying so. Unknown (no trunk, no release this clone can read) is INFO. A clone with no
# remote-tracking branch says so the same way whether or not the executable is at hand,
# because the use-case catalogue records this line, as it does trunk's.
mj_report_release_currency() {
  if [ -z "$(git -C "$MJ_ROOT" for-each-ref --count=1 --format='%(refname)' refs/remotes 2>/dev/null)" ]; then
    mj_info clone "release" "no remote-tracking branch, so whether the trunk's landed work is released cannot be measured"
    return 0
  fi
  # shellcheck source=rust_bin.sh
  . "$MJ_LIB_DIR/rust_bin.sh"
  local bin share out stance summary
  bin="$(mj_rust_bin "$MJ_HOME")"
  if [ ! -x "$bin" ] || ! mj_has jq; then
    mj_info clone "release" "the executable or jq is absent, so whether the trunk is released is not measured here (majordomus run release.currency)"
    return 0
  fi
  share="$(mj_rust_share "$MJ_ROOT")"
  out="$( ( [ -z "$share" ] || export MAJORDOMUS_SHARE="$share"
            "$bin" run release.currency --input '{"next":false}' --quiet --format json --repo "$MJ_ROOT" ) 2>/dev/null | jq -c '.output // empty' 2>/dev/null )" || true
  stance="$(printf '%s' "$out" | jq -r '.stance // empty' 2>/dev/null)"
  summary="$(printf '%s' "$out" | jq -r '.summary // empty' 2>/dev/null)"
  case "$stance" in
    current) mj_ok clone "release" "$summary" ;;
    owed) mj_warn clone "release" "$summary" "the release seat releases it: majordomus run release.currency" ;;
    overdue) mj_warn clone "release" "OVERDUE: $summary" "the release seat releases it now: majordomus run release.currency" ;;
    unknown) mj_info clone "release" "$summary" "majordomus run release.currency" ;;
    *) mj_info clone "release" "whether the trunk is released could not be read (majordomus run release.currency)" ;;
  esac
  return 0
}

# How far this checkout is from the remote trunk (project.a-checkout-serves-the-current-trunk).
# On 2026-10-16 the primary checkout was 779 commits behind origin/master and its shared
# server answered every session from that tree; nothing said so. The measurement and the
# judgement are the executable's (`worktree trunk`, also in `worktree status`): the checkout's
# HEAD and the local trunk branch counted against the remote-tracking trunk, as of the last
# fetch, judged against the policy's checkout.trunk_distance with no threshold invented here.
# Rendered once: stale FAILs — and with it the commit hook — lagging WARNs, unjudged and
# unmeasured are INFO. A merge in progress counts what it brings in, so the commit that
# makes a branch current is never refused for the tree it replaces. The local trunk of a
# linked worktree is somebody else's checkout to move, so it is at most a WARN here.
mj_report_trunk_distance() {
  # A clone with no remote-tracking branch has nothing to be measured against, and says so
  # the same way whether or not the executable is at hand (the catalogue records this line).
  if [ -z "$(git -C "$MJ_ROOT" for-each-ref --count=1 --format='%(refname)' refs/remotes 2>/dev/null)" ]; then
    mj_info clone "trunk" "no remote-tracking branch, so this checkout's distance from the remote trunk cannot be measured"
    return 0
  fi
  # shellcheck source=rust_bin.sh
  . "$MJ_LIB_DIR/rust_bin.sh"
  local bin share out standing message remedy local_standing
  bin="$(mj_rust_bin "$MJ_HOME")"
  if [ ! -x "$bin" ] || ! mj_has jq; then
    mj_info clone "trunk" "the executable or jq is absent, so this checkout's distance from the remote trunk is not judged here (majordomus worktree trunk)"
    return 0
  fi
  share="$(mj_rust_share "$MJ_ROOT")"
  # exit 10 is the reading's verdict on a stale checkout, not a failure to answer
  out="$( ( [ -z "$share" ] || export MAJORDOMUS_SHARE="$share"
            "$bin" worktree trunk --repo "$MJ_ROOT" --format json ) 2>/dev/null )" || true
  standing="$(printf '%s' "$out" | jq -r '.standing // empty' 2>/dev/null)"
  if [ -z "$standing" ]; then
    mj_info clone "trunk" "this checkout's distance from the remote trunk could not be read (majordomus worktree trunk)"
    return 0
  fi
  message="$(printf '%s' "$out" | jq -r '.message')"
  remedy="$(printf '%s' "$out" | jq -r '.remedy // "majordomus worktree trunk"')"
  case "$standing" in
    current) mj_ok clone "trunk" "$message" ;;
    lagging) mj_warn clone "trunk" "$message" "$remedy" ;;
    stale) mj_fail clone "trunk" "$message" "$remedy" ;;
    *) mj_info clone "trunk" "$standing: $message" "majordomus worktree trunk" ;;
  esac
  local_standing="$(printf '%s' "$out" | jq -r 'if .on_trunk then "" else .local_trunk_standing end')"
  case "$local_standing" in
    lagging|stale)
      mj_warn clone "local-trunk" \
        "$(printf '%s' "$out" | jq -r '"the local \(.trunk) is \(.local_trunk.behind) commit(s) behind \(.remote_trunk) (\(.local_trunk_standing)): a branch made from it starts from a tree that is not the trunk"')" \
        "$remedy" ;;
  esac
  return 0
}

# Work that exists in one worktree's files and in no commit (I1995). On 2026-10-07 and
# 2026-10-08 finished work sat as uncommitted and staged files in the worktrees of sessions
# that had ended, and was found only because somebody read those directories. The reading
# is the executable's (`worktree uncommitted`): it decides what holds the work — an active
# task, an open episode that is not stranded, a process inside — and how old is abandoned
# (the policy's session.stranded_after). Here it is only rendered: one WARN per abandoned
# worktree with its path, its count and its age, and the command that would put the work on
# a remote ref. Doctor runs none of it; the reading itself writes nothing.
#
# The reading takes one `git status` per linked worktree — about 15 s over the 300 of this
# repository — and the pre-commit hook runs doctor on every commit. So the hook says where it
# runs (MAJORDOMUS_DOCTOR_CONTEXT=commit, the one switch mj_cmd_doctor accepts) and the
# reading is left out there, by name: a line says it was not run and which command runs it,
# because a check that is silently absent reads exactly like one that passed.
mj_report_uncommitted() {
  if [ "${MAJORDOMUS_DOCTOR_CONTEXT:-}" = commit ]; then
    mj_info clone "uncommitted" "not read in the commit hook (MAJORDOMUS_DOCTOR_CONTEXT=commit): it takes one git status per linked worktree; run it on its own" "majordomus worktree uncommitted"
    return 0
  fi
  # A repository with no linked worktree has nowhere for such work to sit, and says so the
  # same way whether or not the executable is at hand: the use-case catalogue records this
  # line, and a line that depended on the environment made derived data differ between a
  # workstation and CI (validate 38673947429, catalogue.json).
  if [ "$(git -C "$MJ_ROOT" worktree list --porcelain 2>/dev/null | grep -c '^worktree ')" -le 1 ]; then
    mj_ok clone "uncommitted" "no linked worktree, so no uncommitted work can sit in one"
    return 0
  fi
  # shellcheck source=rust_bin.sh
  . "$MJ_LIB_DIR/rust_bin.sh"
  local bin share out path paths age remedy
  bin="$(mj_rust_bin "$MJ_HOME")"
  if [ ! -x "$bin" ] || ! mj_has jq; then
    mj_info clone "uncommitted" "the executable or jq is absent, so uncommitted work in linked worktrees is not reported here (majordomus worktree uncommitted)"
    return 0
  fi
  share="$(mj_rust_share "$MJ_ROOT")"
  out="$( ( [ -z "$share" ] || export MAJORDOMUS_SHARE="$share"
            "$bin" worktree uncommitted --repo "$MJ_ROOT" --format json ) 2>/dev/null )" || out=""
  if [ -z "$out" ]; then
    mj_info clone "uncommitted" "uncommitted work in linked worktrees could not be read (majordomus worktree uncommitted)"
    return 0
  fi
  while IFS="$(printf '\t')" read -r path paths age remedy; do
    [ -n "$path" ] || continue
    mj_warn clone "$path" \
      "$paths uncommitted path(s), the oldest changed $age ago, held by no active task, open episode or process — finished work on one disk" \
      "$remedy"
  done <<EOF
$(printf '%s' "$out" | jq -r '.worktrees[] | select(.standing == "abandoned") | [.path, .paths, (.age // "an unknown time"), .remedy] | @tsv')
EOF
  if [ "$(printf '%s' "$out" | jq -r '.tallies.abandoned')" = 0 ]; then
    mj_ok clone "uncommitted" "$(printf '%s' "$out" | jq -r '"\(.tallies.uncommitted) linked worktree(s) hold uncommitted work, none abandoned against \(.threshold.declared // "no threshold") (\(.tallies.held) held, \(.tallies.recent) recent)"')"
  fi
  # a here-document, not a pipe: a finding counted inside a pipeline's subshell is lost
  local f
  while IFS= read -r f; do
    [ -n "$f" ] && mj_info clone "uncommitted" "$f"
  done <<EOF
$(printf '%s' "$out" | jq -r '.findings[]')
EOF
  return 0
}

# a hook file plus every file in its <hook>.d/ dispatch directory, in dispatch order
mj_hook_candidates() {
  printf '%s\n' "$1"
  if [ -d "$1.d" ]; then find "$1.d" -type f 2>/dev/null | LC_ALL=C sort; fi
  return 0
}
# does the hook line that invokes "<prog> <arg0>" name an executable binary?
mj_hook_binary_ok() {
  local hookfile="$1" arg0="$2" prog="${3:-majordomus}" tok
  for tok in $(grep -E "${prog}[[:space:]]+$arg0" "$hookfile" | grep -oE "[^[:space:]\"']*$prog"); do
    case "$tok" in
      /*) [ -x "$tok" ] && return 0 ;;
      */*) [ -x "$MJ_ROOT/$tok" ] && return 0 ;;
      "$prog") mj_has "$prog" && return 0 ;;
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
             | grep -oE 'mj_pol(_req)? +[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)*' | sed -E 's/mj_pol(_req)? +//' | LC_ALL=C sort -u); do
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
  # the scope: what a worker reads of this repository. Named by the manifest, it must exist,
  # carry only keys the schema declares (share/allow/scope.txt is generated from it) and be
  # the version this distribution reads; unnamed, the executable's default applies, which is
  # said rather than assumed
  if [ -n "$MJ_SCOPE_FILE" ]; then
    if [ ! -f "$MJ_SCOPE_FILE" ]; then
      mj_doctrine_fail layout "$(mj_rel "$MJ_SCOPE_FILE")" "named by the manifest as section 'scope' but absent" "majordomus init --extend"; bad=1
    else
      local sflat sk
      sflat="$(mktemp "${TMPDIR:-/tmp}/mj.scope.XXXXXX")"
      if ! mj_yaml_flatten "$MJ_SCOPE_FILE" > "$sflat" 2>/dev/null; then
        mj_doctrine_fail layout "$(mj_rel "$MJ_SCOPE_FILE")" "does not parse" "majordomus scope"; bad=1
      elif [ "$(mj_yget "$sflat" version)" != "1" ]; then
        mj_doctrine_fail layout "$(mj_rel "$MJ_SCOPE_FILE")" "version '$(mj_yget "$sflat" version)' is not 1" "majordomus scope"; bad=1
      elif sk="$(mj_yaml_unknown_keys "$sflat" "$MJ_ALLOW_DIR/scope.txt")"; [ -n "$sk" ]; then
        mj_doctrine_fail layout "$(mj_rel "$MJ_SCOPE_FILE")" "unknown key(s): $(printf '%s' "$sk" | tr '\n' ' ')" "majordomus scope"; bad=1
      else
        mj_doctrine_ok layout "$(mj_rel "$MJ_SCOPE_FILE")" "version 1; $(grep -c '^in\.' "$sflat") in pathspec(s); every key is one the schema declares"
      fi
      rm -f "$sflat"
    fi
  else
    mj_info layout ".ai/repo/scope.yaml" "the manifest names no scope section; the executable reads the distribution's default (majordomus init --extend adds it)"
  fi
  [ -f "$MJ_AI_DIR/README.md" ] || { mj_doctrine_fail layout ".ai/README.md" "the protocol entrypoint is absent; the layer is not readable without the tool" "majordomus init --extend"; bad=1; }
  rel="$(mj_rel "$MJ_AI_LOCAL_DIR")"
  if ! mj_git check-ignore -q "$rel/state/current.yaml" 2>/dev/null; then
    mj_doctrine_fail layout "$rel/" "is not ignored by git; local state would travel with the branch" "printf '%s/\\n' $rel >> .gitignore"; bad=1
  elif [ -n "$(mj_git ls-files -- "$rel" 2>/dev/null | head -n 1)" ]; then
    mj_doctrine_fail layout "$rel/" "carries tracked files; the local half is this checkout's own" "git ls-files $rel"; bad=1
  else mj_doctrine_ok layout "$rel/" "ignored, and nothing under it is tracked"; fi
  # a personal layer is held to what it claims on every doctor, not only when it was made
  if mj_personal_active; then mj_personal_verify doctrine || bad=1; fi
  if [ -f "$MJ_ROOT/.majordomus/policy.yaml" ]; then
    mj_doctrine_fail layout ".majordomus/policy.yaml" "pre-.ai project data beside the .ai layer; two layouts cannot both be authoritative" "majordomus migrate"; bad=1
  fi
  [ "$bad" = 0 ] && mj_doctrine_ok layout "$(mj_rel "$MJ_AI_DIR")/" "every section the manifest names exists; the layer is readable without the tool"
  return 0
}

# Three directories were checked here as one class, and they are two classes.
#
# The prompts section is **tracked**: the manifest names it, the layer is incomplete
# without it, and no command creates it on demand. Absence is a finding.
#
# The two record stores are **checkout-local** — under .ai/local/, which .gitignore
# excludes — and they are created on demand, by their own writers and by `majordomus
# update`, which does create both (measured: `create .ai/local/state/handovers/`). So every
# new worktree was born with two findings that named a remedy for a cause that did not
# exist, and every worker in it read them on every run. Two standing warnings that no
# action clears are not a signal; they teach a reader to skim the report.
#
# The signal is not lost, because it was never here. The condition that matters is that
# episodes have opened in this checkout and the writers have stopped, and
# `mj_validate_lifecycle` already reports exactly that — conditioned on there having been
# activity to judge, which is the half this check had no way of knowing. What is left here
# is absence, reported as absence, in the same words doctor already uses for a local store
# nothing has written to yet.
mj_validate_layout() {
  local d n
  if [ -d "$MJ_PROMPTS_DIR" ]; then mj_doctrine_ok layout "$(mj_rel "$MJ_PROMPTS_DIR")" "present"
  else mj_doctrine_fail layout "$(mj_rel "$MJ_PROMPTS_DIR")" "missing; the manifest names it and no command creates it on demand" "majordomus update"; fi
  for d in "$MJ_STATE_DIR/handovers" "$MJ_STATE_DIR/checkpoints"; do
    if [ -d "$d" ]; then
      n="$(find "$d" -name '*.md' 2>/dev/null | wc -l | tr -d ' ')"
      mj_doctrine_ok layout "$(mj_rel "$d")" "present, $n record(s)"
    else
      mj_doctrine_ok layout "$(mj_rel "$d")" "absent: nothing has been written to this checkout-local store; its writer and majordomus update both create it"
    fi
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
    [ -f "$f" ] || continue
    mj_is_context_doc "$f" && continue          # the section's own README is not an asset
    n=$((n + 1))
    reason="$(mj_prompt_validate "$f")" || { mj_doctrine_fail prompt "$(basename "$f" .md)" "$(printf '%s' "$reason" | tr '\n' ';' | sed 's/;$//')" "majordomus prompt show $(basename "$f" .md)"; bad=1; }
  done
  [ "$n" -gt 0 ] && [ "$bad" = 0 ] && mj_doctrine_ok prompt "$n asset(s)" "front matter valid, every token known"
  return 0
}

# ---------------------------------------------------------------- lifecycle writtenness
# Whether the lifecycle is still *writing*, which is a different question from whether its
# stores can be read.
#
# Every check that existed before this one asked the second question. `retention` counted
# the files and found them under the cap. `layout` found the directories present.
# `resolver` found a record and reported its divergence label. All three passed, every day,
# for the six days in which this repository wrote no checkpoint and no handover at all
# (ADR 0052) — because a store that nothing writes to is perfectly reachable, perfectly
# well-formed and perfectly under its retention cap.
#
# The invariant here is the one that spans both halves: *if lifecycle events are arriving,
# the derived state they produce must advance.* Episodes were opening and closing that whole
# week, which is the evidence that makes the absence of records a finding rather than a
# quiet period. A repository where nothing at all is happening is not failing; one that is
# demonstrably busy and has written nothing a future worker could resume from is.
mj_validate_lifecycle() {
  # No watch-specific path. The other validators that take one do so because their full
  # output is too long for a drift report; this one emits four lines and the question it
  # answers — is the lifecycle still writing — is exactly what a watch is for.
  local ledger="$MJ_STATE_DIR/ledger.jsonl"
  if [ ! -f "$ledger" ]; then
    mj_doctrine_ok lifecycle "ledger" "no ledger yet; nothing has happened here to be silent about"
    return 0
  fi

  # Is anything happening? Episodes opening is the cheapest evidence of a live lifecycle,
  # and it is the evidence that was present throughout the outage.
  #
  # Counted on this branch only, because the records below are resolved for this worktree
  # and branch. Counting every episode the checkout ever opened made a branch created a
  # minute ago owe a checkpoint and a handover on its first commit: the episodes were the
  # trunk's, the absence was the new branch's, and the pre-commit hook refused the commit.
  local branch opens; branch="$(mj_git_branch)"
  opens="$(grep -F '"event":"session.started"' "$ledger" 2>/dev/null | grep -cF "\"branch\":\"$branch\"" || true)"
  : "${opens:=0}"
  if [ "$opens" -lt 2 ]; then
    mj_doctrine_ok lifecycle "activity" "$opens episode(s) opened here; too few to judge whether the writers stopped"
    return 0
  fi

  # The two records a future worker resumes from. Absence is judged against the same
  # thresholds everything else reads, so there is one place to change the answer.
  local d
  for d in checkpoints handovers; do
    if ! mj_resolve_latest "$MJ_STATE_DIR/$d" ""; then
      mj_doctrine_fail lifecycle "$d" \
        "$opens episode(s) have opened on $branch here and no $d record exists for this worktree and branch" \
        "majordomus ${d%s} --list"
      continue
    fi
    mj_freshness "$MJ_RES_CREATED" >/dev/null
    case "$MJ_FRESH_STATE" in
      stale|invalid)
        mj_doctrine_fail lifecycle "$d" \
          "the newest record is $MJ_FRESH_STATE ($MJ_FRESH_REASON) while episodes go on opening; the writer has stopped" \
          "majordomus ${d%s} --list" ;;
      unknown)
        mj_doctrine_fail lifecycle "$d" \
          "the newest record cannot be dated ($MJ_FRESH_REASON), so nothing can say whether the writer is still running" \
          "majordomus ${d%s} --list" ;;
      *)
        mj_doctrine_ok lifecycle "$d" "newest record is $MJ_FRESH_STATE, $MJ_FRESH_REASON" ;;
    esac
  done

  # A receipt with no resulting event beside it. The receipt exists precisely so that an
  # adapter which arrived and did nothing is distinguishable from one that never fired, and
  # a count that keeps climbing while the records do not is that adapter.
  local got failed
  got="$(grep -c '"event":"provider.event.received"' "$ledger" 2>/dev/null || true)"; : "${got:=0}"
  failed="$(grep -c '"event":"provider.event.failed"' "$ledger" 2>/dev/null || true)"; : "${failed:=0}"
  if [ "$failed" -gt 0 ]; then
    mj_doctrine_fail lifecycle "receipts" \
      "$failed lifecycle event(s) arrived and did not complete their work" \
      "majordomus history --event provider.event.failed"
  elif [ "$got" -gt 0 ]; then
    mj_doctrine_ok lifecycle "receipts" "$got lifecycle event(s) received, none failed"
  fi

  # Episodes that opened and were never closed. An episode is a conversation boundary; one
  # that outlives every threshold the policy has is a client that went away without saying
  # so, and it is not closed here — a diagnostic that silently mutated state would be the
  # watchdog this design refuses. It is named, so that recovery is a decision somebody makes.
  local open_dir="$MJ_STATE_DIR/sessions-open" f stranded=0 newest_open=""
  if [ -d "$open_dir" ]; then
    for f in "$open_dir"/*.yaml; do
      [ -f "$f" ] || continue
      mj_freshness "$(sed -n 's/^started_at: //p' "$f" | head -n 1)" >/dev/null
      case "$MJ_FRESH_STATE" in
        stale|invalid) stranded=$((stranded + 1)); newest_open="$MJ_FRESH_REASON" ;;
      esac
    done
  fi
  if [ "$stranded" -gt 0 ]; then
    mj_doctrine_fail lifecycle "episodes" \
      "$stranded open episode(s) older than the policy's stale threshold (oldest: $newest_open); their clients are gone and nothing closed them" \
      "majordomus session list"
  else
    mj_doctrine_ok lifecycle "episodes" "no episode has outlived the stale threshold"
  fi
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
  # A verdict states what it is a verdict about. "122 rule(s) resolve in one deterministic
  # order" is true and is read as covering the whole set, while the doctrine check below
  # examines only the subset carrying a validator — so the two numbers are printed beside
  # each other here, and neither can be mistaken for the other.
  local tally total enforced blocking blocking_unenforced
  tally="$(mj_rule_tally)"
  total="${tally%% *}"; tally="${tally#* }"
  enforced="${tally%% *}"; tally="${tally#* }"
  blocking="${tally%% *}"; blocking_unenforced="${tally#* }"
  n="$total"
  mj_doctrine_ok rules "$n rule(s)" "resolve in one deterministic order; vendored baseline plus project rules, no override — $enforced carry a validator, $((n - enforced)) are normative text this tool does not check"
  # The number the doctrine check cannot report, because those rules are not in its
  # registry at all: a rule declared blocking with no x-majordomus block. Nothing in this
  # tool runs for it. It may still be held by a CI gate, a behavioural case or a reviewer,
  # and scripts/ci/rule-proof-check is what decides that; what is certain from here is
  # only that the dispatcher will never stop a command on its behalf. Reported rather than
  # failed, because the honest count is large and predates the report; the gate ratchets it.
  if [ "$blocking_unenforced" -gt 0 ]; then
    mj_info rules "blocking without a validator" \
      "$blocking_unenforced of $blocking blocking rule(s) carry no x-majordomus block, so no validator runs for them and the doctrine verdict below says nothing about them" \
      "majordomus rules list | grep -v 'enforced by'"
  else
    mj_doctrine_ok rules "blocking" "all $blocking blocking rule(s) carry a validator"
  fi
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
#
# Two halves, and the difference between them is where the evidence lives.
#
# The wiring half — validator defined, command dispatches, blocking failure propagates —
# reads lib/, which every distribution ships. It is verifiable wherever the tool runs and
# runs everywhere.
#
# The evidence half — the test case that proves the rule, the claim it is filed under, the
# CI that runs the runner — reads test/, docs/CLAIMS.yaml and .github/, which are the
# vendor's source tree and which `scripts/release-package` deliberately does not ship: the
# archive carries bin/ lib/ libexec/ share/ LICENSE and a RELEASE.json stamp. So in an
# installed distribution those paths are absent by construction, and reporting their
# absence as a rule violation told an adopting repository it was missing 107 files it was
# never given and does not owe. That was the whole of `docs/ADOPTION_FIRST_RUN.md`.
#
# The discriminator is the stamp, not the paths. `release-package` writes RELEASE.json and
# `release-verify` refuses an archive without it; a source tree has none. Skipping on "the
# file is not there" would have been the same diff and a check that can never fail
# anywhere, including here — which is the failure mode section 6 of that document names.
# `test/cases/97_vendor_evidence.sh` holds the line: it proves the evidence is still
# checked in a source tree and that a rotted test path is still a failure.
mj_validate_doctrine_wiring() {
  local lib="$MJ_LIB_DIR" root="$MJ_HOME"
  local i=0 id val cls fn cmd t c bad=0 n=0
  # Is this the vendor's source tree, or a packaged distribution serving someone else?
  # mj_validate_command_coverage answers the same question for the same reason ("a rule
  # about Majordomus's own suite, so it applies only in the repository that carries one").
  # It sets MJ_DOCTRINE_SKIPPED because the whole of it is unverifiable away from home;
  # this doctrine is only half unverifiable, so it must not — the wiring half still runs,
  # and a rule that is written down but not wired must still fail in an adopter.
  local packaged=0
  [ -f "$root/RELEASE.json" ] && packaged=1
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
    # the tests that prove it must exist — the vendor's evidence, in the vendor's source
    if [ "$packaged" = 0 ]; then
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
    fi
    i=$((i+1))
  done

  # 2. the other direction — a validator no doctrine declares
  local f declared=" "
  i=0; while mj_doc_row "$i"; do declared="$declared$MJ_DR_VAL "; i=$((i+1)); done
  for f in $(grep -rhoE '^mj_validate_[a-z_]+\(\)' "$lib" | sed -e 's/^mj_validate_//' -e 's/()//' | LC_ALL=C sort -u); do
    case "$declared" in *" $f "*) ;; *) mj_doctrine_fail doctrine "mj_validate_$f" "validator exists but no rule declares it; it runs under no rule" "grep -rn 'validator: $f' $(mj_rel "$MJ_RULES_DIR")"; bad=1 ;; esac
  done

  # 3. CI must run the test runner, without swallowing it. The vendor's CI, in the vendor's
  # source: an adopting repository owes no `.github/workflows/validate.yml` of ours.
  if [ "$packaged" = 0 ]; then
    local ci="$root/.github/workflows/validate.yml"
    if [ ! -f "$ci" ]; then mj_doctrine_fail doctrine "ci" "no .github/workflows/validate.yml; nothing runs the doctrine tests on integration" "ls .github/workflows/"; bad=1
    elif ! grep -qE 'bash test/run\.sh' "$ci"; then mj_doctrine_fail doctrine "ci" "validate.yml does not run test/run.sh" "grep -n 'test/run.sh' .github/workflows/validate.yml"; bad=1
    elif grep -E 'bash test/run\.sh' "$ci" | grep -qE '\|\|[[:space:]]*(true|:)|continue-on-error'; then
      mj_doctrine_fail doctrine "ci" "validate.yml runs test/run.sh but does not let it fail the job" "grep -n -A2 'test/run.sh' .github/workflows/validate.yml"; bad=1
    elif ! grep -E 'bash test/run\.sh' "$ci" | grep -v -- '--runs-in' | grep -q -- '--no-skips'; then
      # a run in which a case declined still passed: 102 skipped "no zsh" on every CI run. The
      # whole suite's invocation is the one that must refuse, not a --runs-in step beside it.
      mj_doctrine_fail doctrine "ci" "validate.yml runs test/run.sh without --no-skips; a case that declines would pass the job" "grep -n 'test/run.sh' .github/workflows/validate.yml"; bad=1
    fi
    # and every job a case's skip defers to holds that case: a skip excused because another job
    # reads its subject is excused for nothing if no job does. The header block only, as
    # test/run.sh reads it, so a case that writes the header into a fixture declares nothing.
    if [ -f "$ci" ]; then
      # one awk over every case rather than one per case: each file's header ends at its first
      # line that is neither blank nor a comment, and yields its first such declaration
      local sj sjobs=""
      for sj in $(awk 'FNR == 1 { over = 0 }
                       over { next }
                       /^[[:space:]]*$/ { next }
                       /^#/ { if (match($0, /^# majordomus-skip-runs-in: *[a-z0-9-]+/)) {
                                v = $0; sub(/^# majordomus-skip-runs-in: */, "", v); sub(/[^a-z0-9-].*$/, "", v)
                                print v; over = 1 } ; next }
                       { over = 1 }' "$root"/test/cases/*.sh 2>/dev/null); do
        case " $sjobs " in *" $sj "*) ;; *) sjobs="$sjobs $sj" ;; esac
      done
      for sj in $sjobs; do
        grep -qE "bash test/run\.sh --no-skips --runs-in $sj([^a-z0-9-]|\$)" "$ci" \
          || { mj_doctrine_fail doctrine "ci" "a case defers its skip to the '$sj' job, and no step in validate.yml runs 'bash test/run.sh --no-skips --runs-in $sj'" "grep -rln '^# majordomus-skip-runs-in: $sj' test/cases"; bad=1; }
      done
    fi
    # and the runner must run every case, not a list that a new case can miss
    if ! grep -qE 'cases/\*\.sh|cases/\*' "$root/test/run.sh"; then
      mj_doctrine_fail doctrine "runner" "test/run.sh does not glob test/cases/; a new case would not run" "grep -n cases test/run.sh"; bad=1
    fi
  fi

  if [ "$packaged" = 1 ]; then
    # Named, not silent: a reader has to be able to tell this apart from a check that passed.
    mj_doctrine_skip doctrine "vendor evidence" \
      "$(sed -n 's/.*"tag": "\([^"]*\)".*/\1/p' "$root/RELEASE.json" 2>/dev/null | head -n 1) is a packaged distribution, so the tests, claims and CI each rule cites are not here to read; they are verified in the package's own source tree, and this repository owes none of them" \
      "cat $root/RELEASE.json"
  fi

  # State the denominator, always. This line used to read "41 doctrines — validator,
  # dispatch, propagation, test and CI resolve for every one", beside another line in the
  # same run reporting 122 rules. "Every one" meant every one of 41 and said nothing about
  # the other 81, and no reader could tell that from the sentence. The subset is now named
  # in the subject, and the remainder is named in the detail, so the verdict cannot be
  # read as covering rules it never looked at.
  local reg_total reg_rest
  reg_total="$(mj_rule_count)"
  reg_rest=$((reg_total - n))
  [ "$bad" = 0 ] && mj_doctrine_ok doctrine "$n of $reg_total rule(s) carry a validator" \
    "$([ "$packaged" = 1 ] && printf 'validator, dispatch and propagation resolve for every one of the %s' "$n" || printf 'validator, dispatch, propagation, test and CI resolve for every one of the %s' "$n")$([ "$reg_rest" -gt 0 ] && printf '; the remaining %s declare no validator and this check does not examine them' "$reg_rest")"
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
# for it. While a hand-written roadmap table survives anywhere, every release it lists has to
# be one a milestone was released in — inferred from the release records (released_in), never typed —
# and every such release has to appear in it: a table that can neither invent a release nor
# hide one. When the table is gone the check holds trivially, which is the point: this
# refuses the regression, it does not require the table.
mj_validate_roadmap() {
  local readme canon doc missing extra id
  mj_project_doctrine_load || return 0
  readme="$MJ_ROOT/README.md"
  [ -f "$readme" ] || { mj_doctrine_ok roadmap "README.md" "absent; nothing can duplicate the roadmap"; return 0; }
  if ! grep -q '^## Roadmap' "$readme"; then
    mj_doctrine_ok roadmap "README.md" "no authored roadmap section; the roadmap is only a projection"
    return 0
  fi
  doc="$(awk '/^## Roadmap/{f=1;next} /^## /{f=0} f' "$readme" \
         | sed -n 's/^| *\**v\{0,1\}\([0-9][0-9.]*\)\** *|.*/v\1/p' | LC_ALL=C sort -u)"
  # A section that lists no releases is prose pointing at the projection, not a second
  # authority. The rule is about an authored release list, not about the heading: a partial
  # list is the dangerous case, and no list at all is the intended end state. It is decided
  # before any milestone's release is looked up, because there is nothing to compare them
  # with: each lookup walks the history, and all of them took a minute of every doctor in a
  # repository whose README lists none (I2995).
  if [ -z "$doc" ]; then
    mj_doctrine_ok roadmap "README.md" "the roadmap section lists no releases; it points at the projection rather than restating it"
    return 0
  fi
  # only a DONE milestone can have been released
  canon="$(for id in $(awk -F'\t' '$1=="M" && $3=="DONE" { print $2 }' "$MJ_PJ/model.tsv"); do
             mj_pj_m_released_in "$id"; printf '\n'; done | { grep -v '^unreleased$' || :; } | LC_ALL=C sort -u)"
  missing="$(LC_ALL=C comm -23 <(printf '%s\n' "$canon") <(printf '%s\n' "$doc") | tr '\n' ' ')"
  extra="$(LC_ALL=C comm -13 <(printf '%s\n' "$canon") <(printf '%s\n' "$doc") | tr '\n' ' ')"
  missing="${missing% }"; extra="${extra% }"
  if [ -n "$extra" ]; then
    mj_doctrine_fail roadmap "README.md" "lists release(s) no milestone was released in: $extra" "majordomus plan roadmap"
    return 0
  fi
  if [ -n "$missing" ]; then
    mj_doctrine_fail roadmap "README.md" "omits release(s) the records show a milestone was released in: $missing" "majordomus plan roadmap"
    return 0
  fi
  mj_doctrine_ok roadmap "README.md" "every release it lists carried a milestone, and no release that did is hidden from it"
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
  # the catalogue is the layer's own: use cases and applications under the manifest's
  # sections, validated by the same function `majordomus usecase validate` runs
  mj_uc_load
  if [ -z "$MJ_UC_DIR" ] || [ ! -d "$MJ_UC_DIR" ]; then
    mj_doctrine_skip catalogue "-" "the manifest names no use-cases section; nothing to resolve"; MJ_DOCTRINE_SKIPPED=1; return 0
  fi
  mj_uc_validate_all
  [ "$MJ_UC_BAD" = 0 ] && mj_doctrine_ok catalogue "$MJ_UC_N use case(s), $MJ_AP_N application(s)" "every command, doctrine, claim, responsibility, category, setup and cross-reference resolves, both directions"
  return 0
}


# ---------------------------------------------------------------- schema integrity
# Two things this repository relied on were true only because the code happened to do them.
#
# The order the rules apply in was in lib/rules.sh and nowhere a person could read, so which
# of two rules wins was answerable only by running it. The rule states it; this reports it,
# so that the statement and the behaviour are checked against each other rather than merely
# both existing.
#
# And a schema was optional. A kind declared without one carries its metadata through
# unvalidated, silently, looking exactly like a kind that validates — every one of those is a
# place where a typo in front matter becomes data. The same hole runs the other way: a schema
# file no kind names describes nothing, is applied to nothing, and drifts away from the shape
# it was written for with nothing to notice. Both directions are checked here because only
# the pair means "everything the tool reads has a contract".
#
# The exemption is the format's and not the author's: a kind whose files are plain text has
# no metadata to validate, and says so by declaring no schema. A kind of any other format
# without one is a gap.
#
# A schema is applied in one of four ways, because the layer does not have one shape. The
# tracked half is indexed, so a kind names the schema and the index enforces it. The local
# half is not indexed at all, so no kind can name one; there the schema is applied through
# the allow-list `generate allow` derives from it, read by the command that loads the file.
# A third is read by a validator written for that one schema, which names the identifier
# directly (lib/capture.sh and majordomus.capture/v1). The fourth is a generated document's
# contract under share/schemas/generated/, applied by the generator, which reads the
# directory rather than any file by name (apps/majordomus-cli/src/generate.rs), and asked
# from the other side by test/cases/52_generated_artifact_typing.sh. Any of the four counts.
# None of them, and the file describes nothing.
mj_validate_schema_integrity() {
  local kinds="$MJ_SHARE_DIR/kinds.yaml" sdir="$MJ_SHARE_DIR/schemas" flat
  [ -f "$kinds" ] || { mj_doctrine_fail schema "share/kinds.yaml" "absent; nothing declares what the tool reads" "ls $MJ_SHARE_DIR"; return 0; }

  # The order, reported from the resolver rather than restated: a second implementation of
  # the ordering here would be the thing most likely to disagree with it.
  #
  # There is no failing branch. A set that does not resolve has no order, so no doctrine can
  # be dispatched at all — the loader in lib/doctrine.sh refuses before any validator runs,
  # naming the dependency that is missing. Repeating that check here would be a second answer
  # to a question already answered, in a function that could not have been reached to ask it.
  mj_doctrine_ok schema "rules" "$(mj_rule_count) rule(s) resolve in one order: the vendored baseline in manifest order, then the project's, as a dependency graph"

  # every kind that carries metadata declares a schema, and every schema is named by a kind
  flat="$(mktemp "${TMPDIR:-/tmp}/mj.kinds.XXXXXX")"
  if ! mj_yaml_flatten "$kinds" > "$flat" 2>/dev/null; then
    mj_doctrine_fail schema "share/kinds.yaml" "does not parse" "majordomus doctor"; rm -f "$flat"; return 0
  fi

  local k fmt sch unschemad="" named=" " n=0
  for k in $(sed -n 's/^kinds\.\([a-z0-9_-]*\)\.format=.*/\1/p' "$flat" | LC_ALL=C sort -u); do
    n=$((n + 1))
    fmt="$(mj_yget "$flat" "kinds.$k.format")"
    sch="$(mj_yget "$flat" "kinds.$k.schema")"
    if [ -n "$sch" ]; then named="$named$sch "
    elif [ "$fmt" != text ]; then unschemad="$unschemad $k"
    fi
  done
  rm -f "$flat"

  if [ -n "$unschemad" ]; then
    mj_doctrine_fail schema "share/kinds.yaml" \
      "$(printf '%s' "$unschemad" | wc -w | tr -d ' ') kind(s) carry metadata and declare no schema, so their front matter is never validated:$unschemad" \
      "grep -n -A4 '^  ${unschemad# }:' share/kinds.yaml"
  else
    mj_doctrine_ok schema "share/kinds.yaml" "$n kind(s); every one that carries metadata declares a schema"
  fi

  # Every schema, wherever it sits. The files are two directories deep — the identifier
  # `majordomus.adr/v1` fixes the path share/schemas/majordomus/adr/adr.v1.schema.json — so
  # a one-level glob here examined nothing and reported it as a clean bill of health.
  local f base id allow orphan="" bad="" m=0 gen=0 list facts="" fok fid fallow
  list="$(find "$sdir" -name '*.schema.json' 2>/dev/null | LC_ALL=C sort)"
  # Whether each file parses, and the two fields read from it, for every schema in one
  # interpreter rather than two or three per file: the answers are mj_json_ok's and
  # mj_json_string's, which each started python3 again for one file, and that start-up was
  # most of what doctor spent here. Without python3 the per-file readers below still answer.
  if command -v python3 >/dev/null 2>&1; then
    facts="$(mktemp "${TMPDIR:-/tmp}/mj.schemas.XXXXXX")"
    printf '%s\n' "$list" | python3 -c "$MJ_SCHEMA_FACTS_PY" > "$facts" 2>/dev/null || { rm -f "$facts"; facts=""; }
  fi
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    base="$(basename "$f" .schema.json)"; m=$((m + 1))
    if [ -n "$facts" ]; then
      IFS= read -r -d '' fok <&3; IFS= read -r -d '' fid <&3; IFS= read -r -d '' fallow <&3
      [ "$fok" = 1 ] || { bad="$bad $base"; continue; }
    else
      mj_json_ok "$f" || { bad="$bad $base"; continue; }
    fi
    # The contracts of the generated documents are applied by the generator, which reads the
    # whole directory (apps/majordomus-cli/src/generate.rs) rather than any file by name, and
    # test/cases/52_generated_artifact_typing.sh asks the same question from the other side:
    # every generated document has a contract here. No kind can name one.
    case "$f" in
      "$sdir"/generated/*) gen=$((gen + 1)); continue ;;
    esac
    if [ -n "$facts" ]; then id="$(mj_schema_identifier "$f" "$fid")"; else id="$(mj_schema_identifier "$f")"; fi
    case "$named" in
      *" $id "*) continue ;;
    esac
    if [ -n "$facts" ]; then allow="$(mj_schema_allow "$f" "$fallow")"; else allow="$(mj_schema_allow "$f")"; fi
    # `if`, not `a && b && continue`: bin/majordomus runs under `set -e`, where an && list
    # that fails as a whole ends the run rather than falling through to the next test.
    if [ -n "$allow" ] && mj_allow_applied "$allow"; then continue; fi
    # Or the tool names the identifier itself, which is what a validator written for one
    # schema looks like (lib/capture.sh names majordomus.capture/v1). Matched against the
    # source rather than a list kept here, so wiring one up is what makes it count.
    if mj_schema_named_in_source "$id"; then continue; fi
    orphan="$orphan $base"
  done 3< "${facts:-/dev/null}" <<SCHEMAS
$list
SCHEMAS
  [ -z "$facts" ] || rm -f "$facts"

  if [ -n "$bad" ]; then
    mj_doctrine_fail schema "share/schemas" "$(printf '%s' "$bad" | wc -w | tr -d ' ') schema(s) do not parse as JSON:$bad" "python3 -m json.tool share/schemas/*/*/${bad# }.schema.json"
  elif [ -n "$orphan" ]; then
    mj_doctrine_fail schema "share/schemas" \
      "$(printf '%s' "$orphan" | wc -w | tr -d ' ') schema(s) are applied by nothing — no kind names them and no allow-list the tool reads is derived from them:$orphan" \
      "grep -n 'schema:' share/kinds.yaml   # wire it to a kind, or delete the file"
  elif [ "$MJ_JSON_PARSER" = none ]; then
    # nothing here could parse a file, so "valid JSON" is not a thing this run established:
    # it says what it did check and that the parse was not asked
    mj_info schema "share/schemas" "$m schema(s), each applied; none was parsed, because neither python3 nor jq can run here — that every one is valid JSON was not established" "python3 -c 'import json'; jq -n 1"
  else
    mj_doctrine_ok schema "share/schemas" "$m schema(s), each valid JSON and each applied — by a kind, through an allow-list the tool reads, by a validator that names it, or as a generated document's contract ($gen)"
  fi
  return 0
}

# The schema identifier a file declares, `<vendor>.<name>/v<n>`. Its own `x-majordomus-schema`
# is the answer where it carries one; where it does not, the identity fixes the path and the
# path gives the identity back — share/schemas/majordomus/adr/adr.v1.schema.json is
# majordomus.adr/v1 — so the file is read first and the path is the fallback, never the
# other way round.
mj_schema_identifier() { # <file> [<its x-majordomus-schema, already read>]
  local id vendor name ver
  if [ $# -ge 2 ]; then id="$2"; else id="$(mj_json_string "$1" x-majordomus-schema)"; fi
  if [ -n "$id" ]; then printf '%s' "$id"; return 0; fi
  name="$(basename "$(dirname "$1")")"
  vendor="$(basename "$(dirname "$(dirname "$1")")")"
  ver="$(basename "$1" .schema.json)"; ver="${ver##*.}"
  printf '%s.%s/%s' "$vendor" "$name" "$ver"
}

# The allow-list a schema is the source of, by name. Declared as `x-majordomus-allow`; where
# it is not, the directory the schema sits in is the name, because that is what
# `generate allow` writes.
mj_schema_allow() { # <file> [<its x-majordomus-allow, already read>]
  local a
  if [ $# -ge 2 ]; then a="$2"; else a="$(mj_json_string "$1" x-majordomus-allow)"; fi
  [ -n "$a" ] || a="$(basename "$(dirname "$1")")"
  printf '%s' "$a"
}

# Does the tool name this schema identifier anywhere in its own source? lib/ and the crate
# only, for the reason mj_allow_applied gives: a walk from the repository root would descend
# into the Rust build directory, which is large, changes constantly, and can tell nobody
# what the tool reads.
mj_schema_named_in_source() {
  [ -n "$1" ] || return 1
  grep -rqF "$1" "$MJ_LIB_DIR" "$MJ_HOME/apps/majordomus-cli/src" 2>/dev/null
}

# The JSON parser this process can actually run: python3, jq, or none. Found once.
#
# "Is python3 on PATH" is not that question. A version manager's shim is on PATH whether or
# not it can start anything: asdf answers "No version is set for command python3" and exits
# non-zero in any directory with no pinned Python, and pyenv does the same. Asked only
# whether the command existed, the two readers below took the shim, read its failure as the
# file's — every schema the tool ships "does not parse as JSON" — and never reached the jq
# that was there. So each candidate is started on nothing first, and the one that runs is
# used.
MJ_JSON_PARSER=""
mj_json_parser_find() {
  [ -z "$MJ_JSON_PARSER" ] || return 0
  if python3 -c 'import json' >/dev/null 2>&1; then MJ_JSON_PARSER=python3
  elif jq -n 1 >/dev/null 2>&1; then MJ_JSON_PARSER=jq
  else MJ_JSON_PARSER=none; fi
}

# One top-level string field of a JSON file, or nothing. A parser that cannot run is not a
# reason to invent an answer: with neither python3 nor jq able to start, the field is
# unknown, and an unknown identifier falls back to the path.
mj_json_string() {
  mj_json_parser_find
  case "$MJ_JSON_PARSER" in
    python3)
      python3 -c 'import json,sys
try:
    v = json.load(open(sys.argv[1])).get(sys.argv[2], "")
except Exception:
    v = ""
sys.stdout.write(v if isinstance(v, str) else "")' "$1" "$2" 2>/dev/null ;;
    jq) jq -r --arg k "$2" 'if (.[$k]? | type) == "string" then .[$k] else "" end' "$1" 2>/dev/null ;;
  esac
}

# The batch form of mj_json_ok and mj_json_string over the paths on stdin, one per line:
# for each, "1" or "0" (does it parse), then its x-majordomus-schema and x-majordomus-allow,
# each NUL-terminated and each what "$(mj_json_string ...)" would have held — a string field
# or nothing, without the trailing newlines and NUL bytes a command substitution drops.
MJ_SCHEMA_FACTS_PY='import json,sys
def field(d, k):
    try:
        v = d.get(k, "")
    except Exception:
        v = ""
    return (v if isinstance(v, str) else "").replace("\0", "").rstrip("\n")
for p in sys.stdin.read().split("\n"):
    if not p:
        continue
    try:
        d = json.load(open(p)); ok = "1"
    except Exception:
        d = None; ok = "0"
    sys.stdout.write(ok + "\0" + field(d, "x-majordomus-schema") + "\0" + field(d, "x-majordomus-allow") + "\0")'

# Is this schema applied through its generated allow-list? True when something in the tool
# reads share/allow/<name>.txt, which is the only way a schema reaches the local half of the
# layer. Matched against the source rather than a list kept here, so wiring one up is the
# thing that makes it count.
mj_allow_applied() {
  [ -f "$MJ_ALLOW_DIR/$1.txt" ] || return 1
  # lib/ and the crate's sources only: a recursive walk from the repository root would
  # descend into the Rust build directory, which is large, changes constantly, and cannot
  # tell anyone anything about what the tool reads
  grep -rqE "(ALLOW_DIR|share/allow)/$1\.txt" "$MJ_LIB_DIR" "$MJ_HOME/apps/majordomus-cli/src" 2>/dev/null
}

# Does this file parse as JSON? Asked of a parser that runs (mj_json_parser_find): what a
# parser that could not start says about a file is nothing about the file. With no parser
# able to run the question cannot be put, and the file is not called broken on that account.
mj_json_ok() {
  mj_json_parser_find
  case "$MJ_JSON_PARSER" in
    python3) python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "$1" >/dev/null 2>&1 ;;
    jq) jq -e . "$1" >/dev/null 2>&1 ;;
    *) return 0 ;;
  esac
}

# What doctor reaches, said where its usage is (I2707). Everything doctor checks is
# governance: the policy, the profiles, the wiring of each declared enforcement, the
# projections, the records and their retention. It reads nothing about the checkout's server,
# its lease or the mesh, and its name suggests that it does, so a clean report used to be
# read as a healthy runtime. Those are read elsewhere, and the help names where. The same
# words are doctor's summary in share/commands.yaml and in bin/majordomus's usage, and
# docs/CLI.md says them; case 3050 holds the four together.
mj_doctor_reach() {
  cat <<'EOF'
majordomus doctor checks governance only: the policy, the profiles, the wiring of every
declared enforcement, the projections, the records and their retention. It does not read
the checkout's server, its lease or the mesh. The runtime is read by:
  majordomus serve status   is the server answering for this checkout current, or stale?
  majordomus mesh doctor    the mesh: identity, trust, link, runtime, journal
  GET /api/v1/health        health.report: the server's own report, the mesh included
  GET /api/v1/ready         can this process answer: lease held, executable current
EOF
}
