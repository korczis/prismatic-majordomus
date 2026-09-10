#!/usr/bin/env bash
# worktree — the doctrine's shell side: doctor's validator for the worktree layout.
#
# It decides nothing. Where the container is, which worktrees exist, and which of them are
# in the wrong place are all answered by the Rust executable's worktree service — the same
# service `majordomus worktree` renders and the MCP tools project — and this file turns that
# answer into doctor findings. There is no second derivation of the `-wt` path here, no
# second reading of `git worktree list`, and no copy of the policy.
#
# The check is machine-local by design: it looks at this machine's directories. The
# repository-level facts (the policy parses, the schema accepts it, the provider projections
# are current) are validated by the crate's suite and by `majordomus generate --check`, which
# is what CI runs. A CI checkout is a normal checkout and is never expected to sit under a
# container.
[ -n "${MJ_LIB_worktree:-}" ] && return 0 || MJ_LIB_worktree=1

# The built executable, or empty when there is none. Never builds: doctor is read-only and
# has a wall-time budget, and building a Rust crate inside it would blow both.
mj_wt_bin() {
  local candidate
  for candidate in \
    "${MAJORDOMUS_BIN:-}" \
    "$MJ_HOME/libexec/majordomus-cli" \
    "$MJ_HOME/apps/majordomus-cli/target/release/majordomus" \
    "$MJ_HOME/apps/majordomus-cli/target/debug/majordomus"; do
    [ -n "$candidate" ] && [ -x "$candidate" ] && { printf '%s' "$candidate"; return 0; }
  done
  command -v majordomus-cli 2>/dev/null && return 0
  return 1
}

# The layout doctrine: every linked worktree of this repository sits directly under the
# canonical container, and the primary checkout is exempt because it is the thing the
# container is named after.
#
# Reports one finding per worktree in the wrong place, each naming the worktree, the branch
# it holds, where linked worktrees belong, and the command that plans the move. It never
# moves anything: doctor diagnoses and the explicit commands repair.
mj_validate_worktree_layout() {
  local bin out rc kind policy path name branch head proposed reason n=0
  if ! bin="$(mj_wt_bin)"; then
    # An installation without the executable still has the rule, and saying so is better
    # than a silent pass that reads as compliance.
    mj_info worktree "$(mj_rel "$MJ_ROOT")" \
      "the worktree topology was not checked: the Rust executable is not built here" \
      "cargo build --manifest-path apps/majordomus-cli/Cargo.toml"
    MJ_DOCTRINE_SKIPPED=1
    return 0
  fi

  out="$(mktemp "${TMPDIR:-/tmp}/mj.wt.XXXXXX")"
  # The status has to be captured from the command itself: inside `if ! cmd`, `$?` is the
  # status of the negation, which is always 0 and would report every failure as a success.
  rc=0
  "$bin" worktree list --tsv --repo "$MJ_ROOT" > "$out" 2>/dev/null || rc=$?
  # 10 is the answer "there are violations", which is the case this validator exists for.
  # Anything else means the topology could not be read at all, and that is its own finding.
  if [ "$rc" != 0 ] && [ "$rc" != 10 ]; then
    mj_doctrine_fail worktree "$(mj_rel "$MJ_ROOT")" \
      "the worktree topology could not be read (exit $rc)" \
      "majordomus worktree list"
    rm -f "$out"
    return 0
  fi

  # kind, policy, path, name, branch, head, proposed path, reason — the order the executable
  # documents for this form, with `-` where a field is absent so that a run of tabs never
  # shifts a column. Nothing here derives any of these values; it reports them.
  while IFS=$'\t' read -r kind policy path name branch head proposed reason; do
    [ "$policy" = fail ] || continue
    [ -n "$path" ] && [ "$path" != "-" ] || continue
    [ "$branch" = "-" ] && branch=""
    [ "$proposed" = "-" ] && proposed=""
    [ "$reason" = "-" ] && reason=""
    n=$((n + 1))
    mj_doctrine_fail worktree "$path" \
      "${reason:-outside the canonical root}${branch:+ (branch $branch)}${proposed:+; would move to $proposed}" \
      "majordomus worktree migrate --plan"
  done < "$out"
  rm -f "$out"

  [ "$n" -gt 0 ] || mj_ok worktree "$(mj_rel "$MJ_ROOT")" \
    "every linked worktree is under the canonical root" "majordomus worktree list"
  return 0
}
