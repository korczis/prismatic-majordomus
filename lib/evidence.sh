#!/usr/bin/env bash
# evidence — what a task owes before it may be called completed, and the evidence that
# discharges each one.
#
# A task's scope says where a worker may write; its `requires` says what the worker owes.
# The two are different promises: a change can sit entirely inside its scope and still be
# uncommitted on a laptop. The vocabulary is data in share/obligations.yaml, so a token is
# added there and not here, and doctor proves in both directions that every token names a
# reachable check and that no check hides under no token.
#
# Evidence is a ledger line, not a file. The ledger is already append-only, ordered and
# integrity-checked, and its envelope already carries the head, the branch and the session.
# What the line adds is the hash of the inputs the evidence was taken over, which is what
# lets a later reader say the evidence no longer describes this tree — stale rather than
# merely old. The vocabulary for that judgement is mj_git_label's, not a new one.
#
# A recorded line is not the first thing asked, though. Where the fact is one the tool can
# hold — the tree is clean, the remote has the commit, the trunk reaches it, the published
# site serves it — it is established live and the ledger is not consulted at all; see
# "establishing the fact" below for why that direction, and what stays on the recorded path.

MJ_OBL_FLAT=""
mj_obligations_load() {
  [ -n "$MJ_OBL_FLAT" ] && [ -f "$MJ_OBL_FLAT" ] && return 0
  local reg="$MJ_BIN_DIR/../share/obligations.yaml"
  [ -f "$reg" ] || mj_die "$MJ_EX_INTERNAL" "obligation vocabulary missing: $reg"
  MJ_OBL_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.ob.XXXXXX")"
  mj_yaml_flatten "$reg" > "$MJ_OBL_FLAT" 2>/dev/null \
    || mj_die "$MJ_EX_INTERNAL" "obligation vocabulary does not parse: $reg"
  [ "$(mj_yget "$MJ_OBL_FLAT" version)" = 1 ] || mj_die "$MJ_EX_INTERNAL" "obligation vocabulary version must be 1"
}

# These read the loaded vocabulary; mj_obligations_load must run in the calling shell
# first, for the reason mj_events_load documents.
mj_obligation_ids() { sed -n 's/^obligations\.[0-9]*\.id=//p' "$MJ_OBL_FLAT"; }
mj_obligation_known() { mj_obligation_ids | grep -Fxq "$1"; }
mj_obligation_index() {
  awk -F= -v want="$1" '/^obligations\.[0-9]+\.id=/ { v = $0; sub(/^[^=]*=/, "", v); if (v == want) { split($1, k, "."); print k[2]; found = 1; exit } }
    END { exit found ? 0 : 1 }' "$MJ_OBL_FLAT"
}
mj_obligation_field() {
  local i; i="$(mj_obligation_index "$1")" || return 1
  mj_yget "$MJ_OBL_FLAT" "obligations.$i.$2"
}
# the pathspecs an obligation's evidence is hashed over; empty for a remote fact, which is
# bound to a commit instead of to a tree
mj_obligation_inputs() {
  local i; i="$(mj_obligation_index "$1")" || return 1
  sed -n "s/^obligations\.$i\.inputs\.[0-9]*=//p" "$MJ_OBL_FLAT"
}
mj_obligation_remote() { [ "$(mj_obligation_field "$1" remote)" = "true" ]; }
# what settles this obligation without asking a worker, or `none`
mj_obligation_established_by() {
  local by; by="$(mj_obligation_field "$1" established_by)" || by=""
  [ -n "$by" ] || by=none
  printf '%s' "$by"
}

# The hash the evidence is taken over: the tracked files the obligation's pathspecs select,
# in git's order, through the one implementation in common.sh. An obligation with no inputs
# hashes to the empty string and is judged by its commit alone.
#
# `set -f` in both subshells, and it is the whole correctness of this function. The specs
# are word-split on purpose — `docs/** README.md .ai/repo/**/README.md` is three of them —
# but unquoted word-splitting also globs, and a bash that expanded them before git saw them
# was answering a different question: without `globstar` a `**` matches exactly one
# directory level, and a bare `*` skips dotfiles. Measured on this tree,
# `.ai/repo/**/README.md` selected 18 files where git selects 24, and the `implementation`
# token's `*` selected 1919 where git selects 2485 — everything under .ai/ and .github/
# outside the hash that exists to make its evidence go stale. That is this feature's own
# failure mode occurring inside the feature: a change to a file the obligation names left
# the evidence looking fresh. Disabling globbing hands git the literal `**` and `*`, which
# it matches itself, correctly, at every depth.
mj_obligation_inputs_hash() {
  local tok="$1" specs files
  specs="$(mj_obligation_inputs "$tok")"
  [ -n "$specs" ] || { printf ''; return 0; }
  # shellcheck disable=SC2086
  files="$(set -f; cd "$MJ_ROOT" && git ls-files -- $specs 2>/dev/null)"
  [ -n "$files" ] || { printf ''; return 0; }
  # shellcheck disable=SC2086
  ( set -f; cd "$MJ_ROOT" && mj_inputs_hash $files )
}

# ---------------------------------------------------------------- the task's obligations
# What the active task declared. Absent means it owes nothing beyond the rest of the
# contract, which is how every task behaved before obligations existed.
# Reads the record mj_load_current already flattened, so the caller loads once and every
# reader here sees the same bytes.
mj_task_requires() {
  [ -n "${MJ_CUR_FLAT:-}" ] || return 0
  sed -n 's/^requires\.[0-9]*=//p' "$MJ_CUR_FLAT"
}

# The most recent evidence line for a token of the active task, as
# "<head>\t<inputs_hash>\t<ts>". Nothing older is consulted: evidence is superseded by
# evidence, and the ledger keeps the history for a reader who wants it.
mj_obligation_evidence() {
  local task="$1" tok="$2"
  local ledger="$MJ_STATE_DIR/ledger.jsonl"
  [ -f "$ledger" ] || return 1
  awk -v task="$task" -v tok="$tok" '
    index($0, "\"event\":\"task.evidence\"") == 0 { next }
    index($0, "\"task\":\"" task "\"") == 0 { next }
    index($0, "\"covers\":\"" tok "\"") == 0 { next }
    { line = $0 }
    END {
      if (line == "") exit 1
      h = line; sub(/.*"head":"/, "", h); sub(/".*/, "", h)
      x = line; sub(/.*"inputs_hash":"/, "", x); sub(/".*/, "", x)
      t = line; sub(/.*"ts":"/, "", t); sub(/".*/, "", t)
      printf "%s\t%s\t%s\n", h, x, t
    }' "$ledger"
}

# ---------------------------------------------------------------- recording evidence
# majordomus evidence --covers <token> [--type <t>] --command <cmd> | --artifact <ref>
#
# The option shape is plan's, so a reader learns it once. What differs is where it lands:
# an issue's evidence is written into the issue, because the contract and its proof belong
# in one record; a task's evidence is a ledger line, because a task's record is the local
# state of one checkout and the ledger is what already survives it.
mj_evidence() {
  local covers="" etype="manual" ecmd="" eart="" eres="" json=0 task ih
  local gate="" gexit=""
  # a bare `evidence` is a question, not a mistake: it prints what it needs, as every
  # other command here does, and exits 2
  [ $# -gt 0 ] || { mj_evidence_usage >&2; return "$MJ_EX_USAGE"; }
  while [ $# -gt 0 ]; do
    case "$1" in
      --covers) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--covers needs a token"; covers="$2"; shift 2 ;;
      --covers=*) covers="${1#--covers=}"; shift ;;
      --gate) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--gate needs a gate id"; gate="$2"; shift 2 ;;
      --gate=*) gate="${1#--gate=}"; shift ;;
      --exit) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--exit needs the exit status the gate reported"; gexit="$2"; shift 2 ;;
      --exit=*) gexit="${1#--exit=}"; shift ;;
      --type) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--type needs a value"; etype="$2"; shift 2 ;;
      --type=*) etype="${1#--type=}"; shift ;;
      --command) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--command needs a command"; ecmd="$2"; shift 2 ;;
      --command=*) ecmd="${1#--command=}"; shift ;;
      --artifact) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--artifact needs a reference"; eart="$2"; shift 2 ;;
      --artifact=*) eart="${1#--artifact=}"; shift ;;
      --result) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--result needs a value"; eres="$2"; shift 2 ;;
      --result=*) eres="${1#--result=}"; shift ;;
      --json) json=1; shift ;;
      -h|--help) mj_evidence_usage; return 0 ;;
      *) mj_die "$MJ_EX_USAGE" "evidence: unknown option $1" ;;
    esac
  done
  mj_require_installed
  # A gate run and an obligation are both evidence, and both are recorded by this verb; what
  # differs is what the record is about. The two are never one invocation: a gate is a fact
  # about the validation pipeline and an obligation is a promise a task made, and a line
  # claiming to be both would be readable as neither.
  if [ -n "$gate" ]; then
    [ -z "$covers" ] || mj_die "$MJ_EX_USAGE" "evidence records a gate or an obligation, not both; drop --covers or --gate"
    [ -n "$gexit" ] || mj_die "$MJ_EX_USAGE" "evidence --gate needs --exit <status>; a gate that reported nothing is not evidence"
    # shellcheck source=gates.sh
    . "$MJ_LIB_DIR/gates.sh"
    mj_gate_prepare "$gate" "$gexit" "$ecmd" "$eres"
    mj_ledger_append task.gate "$MJ_GATE_EXTRA"
    mj_gate_report "$gate" "$gexit" "$json"
    return 0
  fi
  [ -z "$gexit" ] || mj_die "$MJ_EX_USAGE" "--exit is the exit status of a gate; it needs --gate <id>"
  mj_obligations_load
  [ -n "$covers" ] || mj_die "$MJ_EX_USAGE" "evidence needs --covers <token>; one of: $(mj_obligation_ids | tr '\n' ' ')"
  mj_obligation_known "$covers" || mj_die "$MJ_EX_USAGE" \
    "no obligation '$covers'; share/obligations.yaml declares: $(mj_obligation_ids | tr '\n' ' ')"
  [ -n "$ecmd" ] || [ -n "$eart" ] || mj_die "$MJ_EX_USAGE" "evidence needs --command or --artifact; narrative is not evidence"
  mj_load_current || mj_die "$MJ_EX_MISSING" "no active task ($(mj_rel "$MJ_STATE_DIR")/current.yaml); run: majordomus start"
  task="$(mj_cur id)"
  # The task must have declared the obligation. Recording evidence for something nobody
  # promised is how a checklist grows entries nobody asked for.
  mj_task_requires | grep -qx -- "$covers" || mj_die "$MJ_EX_REFUSED" \
    "the active task does not require '$covers' (it requires: $(mj_task_requires | paste -sd, -))"
  ih="$(mj_obligation_inputs_hash "$covers")"
  local extra="\"task\":\"$task\",\"covers\":\"$covers\",\"kind\":\"$etype\",\"inputs_hash\":\"$ih\""
  [ -n "$ecmd" ] && extra="$extra,\"command\":\"$(mj_json_esc "$ecmd")\""
  [ -n "$eart" ] && extra="$extra,\"artifact\":\"$(mj_json_esc "$eart")\""
  [ -n "$eres" ] && extra="$extra,\"result\":\"$(mj_json_esc "$eres")\""
  mj_ledger_append task.evidence "$extra"
  if [ "$json" = 1 ]; then
    printf '{"task":"%s","covers":"%s","kind":"%s","inputs_hash":"%s"}\n' "$task" "$covers" "$etype" "$ih"
  else
    printf 'evidence: %s recorded for %s%s\n' "$covers" "$task" \
      "$([ -n "$ih" ] && printf ' (inputs %s)' "$(printf '%s' "$ih" | cut -c1-12)")"
    # Recording a token the tool establishes is allowed and mostly without effect, and
    # saying so is kinder than refusing it: this line is exactly what a checkout that
    # *cannot* establish the fact — no remote configured, no route to the published site —
    # falls back to, so the option has to stay. What it must not do is let a worker walk
    # away believing they have discharged something.
    [ "$(mj_obligation_established_by "$covers")" = none ] || printf \
      '          note: %s is established from %s where it can be, and this record is read only where it cannot\n' \
      "$covers" "$(mj_obligation_established_by "$covers")"
  fi
}

mj_evidence_usage() {
  cat <<USAGE
usage: majordomus evidence --covers <token> [--type <kind>] (--command <cmd> | --artifact <ref>) [--result <r>] [--json]
       majordomus evidence --gate <id> --exit <status> [--command <cmd>] [--result <r>] [--json]

  Record that one obligation the active task declared has been discharged. The evidence is
  a ledger line carrying the hash of the files the obligation names, so that changing any
  of them makes the evidence stale rather than merely old.

  --covers    the obligation; share/obligations.yaml declares them
  --type      how it was taken (test, build, ci, artifact, manual); default manual
  --command   the command that produced it — narrative is not evidence
  --artifact  a reference the evidence points at, such as a published URL
  --result    what it said, when a command's output is the point
  --gate      a validation gate of .ai/repo/ci/gates.yaml that has just reported, instead of
              an obligation. The line carries the hash of the files that select that gate, so
              a run stops discharging it the moment one of them changes. `majordomus check`
              reports every gate the task's change set selects; a gate that has never
              reported is `queued` and never `pass`.
  --exit      the gate's exit status; 0 is a pass and anything else refuses `completed`

  Some obligations are not recorded at all. A token whose established_by in
  share/obligations.yaml is not \`none\` — commit, push, target, pages — is settled live at
  HEAD by check and finish, and a record of it is read only in a checkout that cannot settle
  it, such as one with no remote configured or no route to the published site.

  exit 0 recorded, 2 on usage, 11 when no task is active or the task did not promise it
USAGE
}

# ---------------------------------------------------------------- establishing the fact
# Six of the eleven tokens name a fact that lives outside the working tree, and most of
# those are facts a machine already holds. Git knows whether the task's changes are still
# in the tree or in the history, whether a remote-tracking ref reaches the head, and
# whether the trunk does. `scripts/pages verify` knows whether the published site serves
# this commit — it has known since it was written, and until now nothing but the Pages
# workflow had ever asked it. Asking a worker to type any of that into a ledger is asking
# them to transcribe an answer the tool can read.
#
# So a token whose `established_by` names something is settled here, live, and no recorded
# evidence is consulted for it. Establishment beats recording in both directions: it
# discharges without a ledger line, and it refuses one that says otherwise, because a
# hand-recorded `commit` against a dirty tree is not evidence of anything.
#
# Live rather than recorded, deliberately, and the argument runs both ways.
#
# For recording: a ledger line says when the fact was true, which is what an auditor reading
# the history a month later wants, and it keeps one mechanism instead of two.
#
# For live, which is what this does: the whole of this file exists because a record outlives
# the thing it described, and the cure it applies everywhere else is recomputation, never a
# timestamp — `mj_obligation_inputs_hash` recomputes, the site's `source_hash` recomputes.
# A git fact recomputes in milliseconds and is always a statement about now. Recording it
# would manufacture exactly the staleness this file was written to remove, and then need
# the staleness machinery to take it away again — a round trip whose only product is a
# window during which the record and the repository disagree. The ledger already holds what
# a worker *did*: every start, checkpoint, evidence and finish is in it. It is not where the
# tool writes down what it can look up. And a hand-recorded remote fact is exactly the thing
# a worker can be wrong about in the direction that flatters them.
#
# The staleness discipline is kept rather than dropped. A fact established here is taken at
# HEAD by construction, which is `mj_git_label`'s `exact`, and `exact` is the word the
# finding says. The other three labels keep their meaning on the fallback path below, which
# is what an obligation nothing here can settle still runs through. No fifth vocabulary.
#
# Each of these prints "<message><TAB><reproduce>" and exits 0 established, 1 refuted,
# 2 undecidable — nothing here could settle it, so the recorded evidence is consulted as it
# was before.
mj_obl_say() { printf '%s\t%s\n' "$1" "$2"; }

# The task's changes are in the history rather than in the tree. What counts as the task's
# changes is the task's own scope: a worker cannot be held to a session record a hook wrote
# under .ai/, and a task whose scope *is* .ai/ must be held to exactly that. A task that
# declared no scope falls back to everything outside the layer's own directory, which is
# what `mj_validate_scope` already excludes for the same reason.
mj_obl_est_commit() {
  local head base dirty f s inside out="" n=0 first=""
  head="$(mj_git_head)"
  [ "$head" = NONE ] && { mj_obl_say "the checkout has no commit at all; nothing of this task is in any history" "git log -1"; return 1; }
  dirty="$(mj_git status --porcelain=v1 2>/dev/null | cut -c4- | sed 's/^.* -> //')"
  local scope_list; scope_list="$(mj_ylist "$MJ_CUR_FLAT" scope 2>/dev/null || true)"
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    if [ -n "$scope_list" ]; then
      inside=0
      for s in $scope_list; do mj_path_contains "$s" "$f" && { inside=1; break; }; done
      [ "$inside" = 1 ] || continue
    else
      mj_is_ai_path "$f" && continue
    fi
    n=$((n + 1)); [ -n "$first" ] || first="$f"
  done <<EOF
$dirty
EOF
  if [ "$n" -gt 0 ]; then
    out="$n file(s) the task touched are still in the working tree, not in the branch's history (${first}"
    [ "$n" -gt 1 ] && out="$out and $((n - 1)) more"
    mj_obl_say "$out)" "git status --porcelain"
    return 1
  fi
  base="$(mj_cur head)"
  if [ -n "$base" ] && [ "$base" != NONE ] && [ "$base" = "$head" ]; then
    mj_obl_say "the tree is clean and no commit was made since the task started (${head:0:12}); there is nothing of this task in the history to be committed" "git log --oneline $base..HEAD"
    return 1
  fi
  mj_obl_say "exact: the tree is clean and ${head:0:12} carries the task's changes" "git status --porcelain"
  return 0
}

# The remote-tracking refs are read as they stand. Nothing here fetches: a validator that
# went to the network on every `check` would make a diagnostic depend on connectivity, and
# a fetch is a write to the object store. The reading is safe in the direction that matters
# — a tracking ref cannot contain a commit the remote never received, so a stale ref can
# only say "not yet" when the answer is "yes", never the reverse.
mj_obl_est_push() {
  local head up ref
  head="$(mj_git_head)"
  [ "$head" = NONE ] && { mj_obl_say "the checkout has no commit" "git log -1"; return 2; }
  [ -n "$(mj_git remote 2>/dev/null)" ] || { mj_obl_say "the checkout has no remote, so a push cannot be established here" "git remote -v"; return 2; }
  up="$(mj_git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' 2>/dev/null || true)"
  if [ -n "$up" ] && mj_git merge-base --is-ancestor "$head" "refs/remotes/$up" 2>/dev/null; then
    mj_obl_say "exact: $up contains ${head:0:12}" "git rev-parse $up"
    return 0
  fi
  ref="$(mj_git for-each-ref --contains "$head" --format='%(refname:short)' refs/remotes 2>/dev/null | head -1)"
  if [ -n "$ref" ]; then
    mj_obl_say "exact: $ref contains ${head:0:12}${up:+, though the branch tracks $up}" "git branch -r --contains $head"
    return 0
  fi
  mj_obl_say "no remote-tracking ref reaches ${head:0:12}${up:+ (the branch tracks $up)}; the commit has not reached the remote" "git push"
  return 1
}

# The trunk reaches the commit. The default branch is git's own record of it —
# refs/remotes/<remote>/HEAD, written by clone and by `git remote set-head` — and not a
# name written down anywhere here: a repository whose trunk is `main` must not need this
# file edited.
mj_obl_est_target() {
  local head remote def
  head="$(mj_git_head)"
  [ "$head" = NONE ] && { mj_obl_say "the checkout has no commit" "git log -1"; return 2; }
  remote="$(mj_git rev-parse --abbrev-ref --symbolic-full-name '@{upstream}' 2>/dev/null || true)"
  remote="${remote%%/*}"
  [ -n "$remote" ] || remote="$(mj_git remote 2>/dev/null | head -1)"
  [ -n "$remote" ] || { mj_obl_say "the checkout has no remote, so integration cannot be established here" "git remote -v"; return 2; }
  def="$(mj_git symbolic-ref --short "refs/remotes/$remote/HEAD" 2>/dev/null || true)"
  [ -n "$def" ] || { mj_obl_say "the checkout records no default branch for '$remote', so integration cannot be established here" "git remote set-head $remote -a"; return 2; }
  if mj_git merge-base --is-ancestor "$head" "refs/remotes/$def" 2>/dev/null; then
    mj_obl_say "exact: $def reaches ${head:0:12}" "git merge-base --is-ancestor HEAD $def"
    return 0
  fi
  mj_obl_say "$def does not reach ${head:0:12}; the work is not on the trunk" "git log --oneline $def..HEAD"
  return 1
}

# Publication, asked of the published site. `scripts/pages verify` is the probe and there is
# no second one here: it already polls the identity document the site serves until `.commit`
# equals a given commit, and writing another would be a second answer to one question.
# `--timeout 0` is what turns its poll into the single probe a validator can afford — the
# loop reads the site before it looks at the clock — and its exit codes carry the three
# outcomes this needs: 0 serves it, 10 serves something else, 12 could not be reached.
mj_obl_est_pages() {
  local head out rc=0
  head="$(mj_git_head)"
  [ "$head" = NONE ] && { mj_obl_say "the checkout has no commit" "git log -1"; return 2; }
  [ -x "$MJ_ROOT/scripts/pages" ] || { mj_obl_say "this repository has no scripts/pages, so publication cannot be established here" "ls scripts/pages"; return 2; }
  out="$(cd "$MJ_ROOT" && ./scripts/pages verify --commit "$head" --timeout 0 --quiet 2>&1)" || rc=$?
  case "$rc" in
    0)  mj_obl_say "exact: the published site serves ${head:0:12}" "scripts/pages verify --commit HEAD" ; return 0 ;;
    10) mj_obl_say "$(printf '%s' "$out" | sed -n 's/^pages verify: //p' | tail -1)" "scripts/pages verify --commit HEAD" ; return 1 ;;
    *)  mj_obl_say "the published site could not be reached (scripts/pages verify exited $rc), so publication cannot be established here" "scripts/pages verify --commit HEAD" ; return 2 ;;
  esac
}

# The dispatcher. The vocabulary declares *that* a token can be established and by what;
# which function does it is this one line, keyed by the token's own id, so a token declared
# establishable with nothing to establish it is a reported defect rather than a silent pass.
mj_obligation_establish() {
  local tok="$1" by fn
  by="$(mj_obligation_established_by "$tok")"
  [ "$by" != none ] || { mj_obl_say "$(mj_obligation_field "$tok" unestablished)" ""; return 2; }
  fn="mj_obl_est_$tok"
  mj_is_function "$fn" || { mj_obl_say "share/obligations.yaml says '$by' establishes '$tok', and no $fn exists to do it" "grep -rn $fn lib/"; return 2; }
  "$fn"
}

# ---------------------------------------------------------------- the judgement
# The judgement about one obligation of one task, with no verdict attached: prints
# "<state><TAB><message><TAB><reproduce>" and returns 0 only when the state is `pass`.
#
#   pass   discharged: established true here, or evidence that still describes this tree
#   unmet  owed and not discharged, or established false
#   stale  evidence exists and no longer describes this tree or this commit
#
# Whether a shortfall *refuses* is not decided here. `check` asks through mj_obl_verdict,
# where the outcome decides; a live scenario reports the state as its step result. Both
# reach the same answer about the same tree because there is one judgement and not two
# (ADR 38) — a second implementation is how `check` and a gate come to disagree about
# whether the same work is finished.
mj_obligation_judge() {
  local task="$1" tok="$2" by est est_rc emsg erep ev head_rec ih_rec ih_now label repro
  mj_obligations_load
  if ! mj_obligation_known "$tok"; then
    printf 'unmet\t%s\t%s\n' "share/obligations.yaml does not declare '$tok'" "majordomus evidence --help"; return 1
  fi
  repro="majordomus evidence --covers $tok --command '$(mj_obligation_field "$tok" discharged_by)'"
  by="$(mj_obligation_established_by "$tok")"
  est=""; est_rc=0
  est="$(mj_obligation_establish "$tok")" || est_rc=$?
  emsg="${est%%"$MJ_TAB"*}"; erep="${est#*"$MJ_TAB"}"; if [ "$erep" = "$est" ]; then erep=""; fi
  case "$est_rc" in
    0) printf 'pass\t%s\t%s\n' "$emsg" "$erep"; return 0 ;;
    1) printf 'unmet\t%s\t%s\n' "$emsg" "$erep"; return 1 ;;
  esac
  if ! ev="$(mj_obligation_evidence "$task" "$tok")"; then
    printf 'unmet\t%s\t%s\n' "owed, and no evidence was recorded$([ "$by" != none ] && [ -n "$emsg" ] && printf ' (%s)' "$emsg")" "$repro"; return 1
  fi
  head_rec="$(printf '%s' "$ev" | cut -f1)"; ih_rec="$(printf '%s' "$ev" | cut -f2)"
  if mj_obligation_remote "$tok"; then
    label="$(mj_git_label "$head_rec" "$(mj_git_branch)")"
    case "$label" in
      exact) printf 'pass\t%s\t\n' "discharged at this commit"; return 0 ;;
      advanced) printf 'stale\t%s\t%s\n' "the evidence names $(printf '%s' "$head_rec" | cut -c1-12), and the branch has moved since; the fact it proved is about the older commit" "$repro"; return 1 ;;
      *) printf 'stale\t%s\t%s\n' "the evidence was taken in a $label context ($(printf '%s' "$head_rec" | cut -c1-12)); it does not describe this branch" "$repro"; return 1 ;;
    esac
  fi
  ih_now="$(mj_obligation_inputs_hash "$tok")"
  if [ "$ih_rec" = "$ih_now" ]; then
    printf 'pass\t%s\t\n' "discharged over inputs $(printf '%s' "$ih_now" | cut -c1-12)"; return 0
  fi
  printf 'stale\t%s\t%s\n' "the evidence was taken over inputs $(printf '%s' "$ih_rec" | cut -c1-12) and this tree hashes to $(printf '%s' "$ih_now" | cut -c1-12); it no longer describes what it proved" "$repro"
  return 1
}

# ---------------------------------------------------------------- doctrine validator
# Dispatched by check and finish through majordomus.obligation-closure. Skipped for every
# outcome but completed, as the verification and profile lines already are: a task that
# reports itself blocked is being honest, and refusing it would teach a worker to lie.
# One place decides whether a shortfall refuses or merely reports: the outcome does.
# mj_obl_verdict <subject> <message> <reproduce>
mj_obl_verdict() {
  if [ "${MJ_FINISH_OUTCOME:-}" = completed ]; then mj_doctrine_fail obligation "$1" "$2" "$3"
  else mj_doctrine_skip obligation "$1" "$2 (not refused: the outcome is not completed)"; fi
}

mj_validate_obligations() {
  local toks tok rel j jstate jrest jmsg jrep
  mj_load_current || { mj_doctrine_skip obligation "-" "no active task; nothing owes anything"; return 0; }
  toks="$(mj_task_requires)"
  if [ -z "$toks" ]; then
    mj_doctrine_skip obligation "$(mj_cur id)" "the task declares no obligations"
    return 0
  fi
  # Every obligation is judged on every run, so that `check` can say an evidence has gone
  # stale before a worker builds an afternoon on it. Only work that claims to be done is
  # refused: an outcome that is not completed is a worker being honest, and refusing it
  # would teach the worker to claim completed instead. `mj_obl_verdict` carries that
  # distinction so the judgement itself is written once.
  mj_obligations_load
  rel="$(mj_rel "$MJ_STATE_DIR")/current.yaml"
  for tok in $toks; do
    if ! mj_obligation_known "$tok"; then
      mj_obl_verdict "$rel" "the task requires '$tok', which share/obligations.yaml does not declare" "majordomus evidence --help"
      continue
    fi
    # The judgement is mj_obligation_judge's, so that a live scenario asserting the same
    # token reaches the same answer. What is decided here is only what a shortfall costs.
    # the state word decides, not the exit code: judge returns non-zero for both unmet
    # and stale, and those are one verdict here and two words in a live scenario
    j="$(mj_obligation_judge "$(mj_cur id)" "$tok")" || true
    jstate="${j%%"$MJ_TAB"*}"; jrest="${j#*"$MJ_TAB"}"
    jmsg="${jrest%%"$MJ_TAB"*}"; jrep="${jrest#*"$MJ_TAB"}"; if [ "$jrep" = "$jrest" ]; then jrep=""; fi
    case "$jstate" in
      pass) mj_doctrine_ok obligation "$tok" "$jmsg" ;;
      *) mj_obl_verdict "$tok" "$jmsg" "$jrep" ;;
    esac
  done
  return 0
}

# The dispatcher's entry point. `evidence` is the verb because recording is what a worker
# does here; the obligations themselves are read through `check`, which already reports
# what is outstanding, rather than through a second listing command nobody would run.
mj_cmd_evidence() { mj_evidence "$@"; }
