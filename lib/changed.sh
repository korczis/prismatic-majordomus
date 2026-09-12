#!/usr/bin/env bash
# sourced by the record writers; guard against re-sourcing
[ -n "${MJ_LIB_changed:-}" ] && return 0 || MJ_LIB_changed=1
# changed — which of the files a dirty tree carries are the episode's work product, and
# which are Majordomus's own exhaust.
#
# Every record this tool writes — a checkpoint, a handover, a closed session — took
# `changed_files` straight from `git status --porcelain`. That list is not the work. Running
# the tool dirties the tree: `majordomus update` rewrites CLAUDE.md and AGENTS.md,
# `majordomus generate` rewrites share/allow/ and share/schemas/, `scripts/derive` rewrites
# docs/generated/ and site/data/generated/, and every command appends a line to
# .ai/local/state/ledger.jsonl. The record then reads as a claim that the episode wrote all
# of it.
#
# Measured, not supposed. The record at .ai/repo/sessions/20260910T103933Z--s-20260909152316-024f--…
# carries 122 changed_files. Among them are docs/generated/registry.json, the whole of
# site/data/generated/, eleven pages under site/content/sessions/ — and the three sibling
# records of that very episode, plus site/content/sessions/s-20260909152316-024f.md, which is
# that episode's own site projection. A record naming itself as its own work product is the
# clearest possible statement that nothing was classifying this list.
#
# Nothing here invents a list of derived paths. Five declarations already say what is
# derived, each owned by something that is tested, and this file reads all five:
#
#   .ai/repo/scope.yaml   out.generated.paths / out.generated.names — the tool's own
#                         declaration, shipped in the skeleton, so this works in any
#                         repository Majordomus supervises and not only in its own
#   .ai/repo/scope.yaml   out.paths — the never-read set, which is where .ai/local/ is
#                         declared: this checkout's state is never anybody's work product
#   .ai/repo/policy.yaml  projections[].target — CLAUDE.md, AGENTS.md, .bb/AGENTS.md, which
#                         `majordomus update` rewrites from the policy
#   .gitattributes        every path marked `merge=derived` — the repository's own list of
#                         what `scripts/derive` writes, maintained because the merge driver
#                         needs it (test/cases/57_derived_merge_driver.sh)
#   the store resolvers   `mj_session_store`, and the checkpoint and handover directories.
#                         These are not derived — a closed record is a tracked object the
#                         executable indexes — but the tool writes them and the worker does
#                         not, and the record that started this file named its own siblings
#
# A repository that declares none of them loses nothing: the classifier then excludes
# nothing and the list is what it was before. Adding a path to any of those declarations is
# what changes this, which is the point — a list of its own here would be the next place to
# forget.

# The declared-derived pathspecs, one `<declaration>\t<pathspec>` per line, cached for the
# process. Reading them all once per record is cheap; reading them once per *file* in a
# hundred-file tree is the shape that cost the scope validator a second before it hoisted
# its own read out of the loop.
#
# Each line carries the tag of the declaration it came from, so that a record can say which
# declaration excluded a path and not merely that something did. The tags are the five
# declarations above, named; they are not a sixth list of paths, and nothing below can
# exclude a path the tags alone would cover.
MJ_DERIVED_PATHS=""; MJ_DERIVED_NAMES=""; MJ_DERIVED_LOADED=0
# prefix each non-empty line of stdin with a declaration tag
mj_changed_tag() { awk -v t="$1" 'NF { print t "\t" $0 }'; }
mj_changed_load_declarations() {
  [ "$MJ_DERIVED_LOADED" = 1 ] && return 0
  MJ_DERIVED_LOADED=1
  local flat t j

  # 1 + 2. the scope declaration: what is never read, and what is generated
  if [ -n "${MJ_SCOPE_FILE:-}" ] && [ -f "$MJ_SCOPE_FILE" ]; then
    flat="$(mktemp "${TMPDIR:-/tmp}/mj.cd.XXXXXX")"
    # A scope file that does not parse is a finding doctor already reports, and it is not
    # this classifier's to raise a second time. It costs the exclusion, never the record.
    if mj_yaml_flatten "$MJ_SCOPE_FILE" > "$flat" 2>/dev/null; then
      MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS$(mj_ylist "$flat" out.generated.paths | mj_changed_tag scope-generated)"$'\n'
      MJ_DERIVED_NAMES="$MJ_DERIVED_NAMES$(mj_ylist "$flat" out.generated.names | mj_changed_tag scope-generated-name)"$'\n'
      # The never-read set, of which .ai/local/ is the entry that matters here. The whole
      # set is taken rather than that one path: a repository that declares another tree
      # unreadable has said the same thing about it, and a work product nobody may read is
      # not a work product.
      MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS$(mj_ylist "$flat" out.paths | mj_changed_tag scope-never-read)"$'\n'
    fi
    rm -f "$flat"
  fi

  # 3. the provider projections, by the policy that names them. Read here rather than
  # through `mj_projection_targets`, which is the scope doctrine's reader of the same
  # declaration and lives in lib/check.sh behind four other sources a record write has no
  # business loading. Two readers of one declaration is fine; a second declaration is not.
  mj_load_policy 2>/dev/null || true
  j=0
  while t="$(mj_pol "projections.$j.target")"; [ -n "$t" ]; do
    MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS"$'policy-projection\t'"$t"$'\n'; j=$((j + 1))
  done

  # 4. the record stores. Nothing declares these as derived because they are not: a closed
  # session record is a tracked object of the layer and the executable indexes it. They are
  # excluded for a different reason — the tool writes them, the worker does not. The record
  # that started this file named its own three sibling records and its own site projection
  # among the 122 files it claimed to have changed. The paths come from the store resolvers
  # rather than from a list, so a repository whose manifest puts the sessions section
  # somewhere else gets the right answer without anybody remembering to update this.
  if command -v mj_session_store >/dev/null 2>&1; then
    MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS"$'record-store\t'"$(mj_rel "$(mj_session_store)")/"$'\n'
  fi
  MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS"$'record-store\t'"$(mj_rel "$MJ_STATE_DIR")/checkpoints/"$'\n'
  MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS"$'record-store\t'"$(mj_rel "$MJ_STATE_DIR")/handovers/"$'\n'

  # 5. the repository's own derived trees, from the merge driver's declaration. The
  # attribute line is `<pathspec><whitespace>merge=derived`; a pathspec with a space in it
  # would be quoted, and none is, so the first field is the whole of it.
  if [ -f "$MJ_ROOT/.gitattributes" ]; then
    while IFS= read -r t; do
      [ -n "$t" ] && MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS"$'gitattributes-derived\t'"$t"$'\n'
    done < <(awk '$0 !~ /^[[:space:]]*#/ && $0 ~ /merge=derived/ { print $1 }' "$MJ_ROOT/.gitattributes")
  fi
  return 0
}

# Is repository-relative path $1 covered by one of the declarations? 0 yes, 1 no.
#
# The pathspecs come from three different vocabularies — scope.yaml's glob subset, a policy
# target that is a plain path, a gitattributes pattern — and the three agree on the forms
# that actually appear: a plain path, a trailing `/` for a directory, and a trailing `/**`
# or `/*`. Those are matched; anything else is matched literally and therefore excludes
# nothing, which is the safe direction for a filter that decides what a record omits.
#
# MJ_CHANGED_WHY is left holding the tag of the declaration that matched, so that a caller
# can say which one excluded the path. "Something excluded it" is a sentence nobody can act
# on; "gitattributes-derived excluded it" names the file to edit if the answer is wrong.
#
# It is assigned only on a match, never once per candidate. This loop runs the whole
# declaration set — about eight hundred pathspecs in this repository — against every changed
# file, so an assignment per iteration is two hundred thousand of them on the tree that named
# this work, on the record-writing path.
MJ_CHANGED_WHY=""
mj_changed_is_derived() {
  local f="$1" p n tag
  mj_changed_load_declarations
  MJ_CHANGED_WHY=""
  while IFS=$'\t' read -r tag p; do
    [ -n "$p" ] || continue
    case "$p" in
      */'**') [ -z "${f##"${p%'**'}"*}" ] && { MJ_CHANGED_WHY="$tag"; return 0; } ;;
      */'*')  [ -z "${f##"${p%'*'}"*}" ] && { MJ_CHANGED_WHY="$tag"; return 0; } ;;
      */)     [ -z "${f##"$p"*}" ] && { MJ_CHANGED_WHY="$tag"; return 0; } ;;
      *)      [ "$f" = "$p" ] && { MJ_CHANGED_WHY="$tag"; return 0; }
              [ -z "${f##"$p"/*}" ] && { MJ_CHANGED_WHY="$tag"; return 0; } ;;
    esac
  done <<EOF
$MJ_DERIVED_PATHS
EOF
  while IFS=$'\t' read -r tag n; do
    [ -n "$n" ] || continue
    # shellcheck disable=SC2254  # the pattern is the declaration; that is the whole point
    case "${f##*/}" in $n) MJ_CHANGED_WHY="$tag"; return 0 ;; esac
  done <<EOF
$MJ_DERIVED_NAMES
EOF
  return 1
}

# The episode's work product: the files a record should name.
#
#   mj_changed_files [<base commit>]
#
# Without a base it is the dirty working tree, which is what a checkpoint or a handover
# describes. With one it is `mj_git_touched`, which adds the commits this line of work made
# — the same measurement the scope doctrine judges against, so a file that scope refused and
# a file a record names are the same file.
#
# MJ_CHANGED_EXCLUDED is left holding how many were classified out, so the caller can say so
# out loud. A filter that silently shortens a list is indistinguishable from a tree that was
# cleaner than it was.
MJ_CHANGED_EXCLUDED=0; MJ_CHANGED_KEPT=0; MJ_CHANGED_EXCLUDED_TAGS=""
mj_changed_files() {
  local base="${1:-}" f
  MJ_CHANGED_EXCLUDED=0; MJ_CHANGED_KEPT=0; MJ_CHANGED_EXCLUDED_TAGS=""
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    if mj_changed_is_derived "$f"; then
      MJ_CHANGED_EXCLUDED=$((MJ_CHANGED_EXCLUDED + 1))
      # One tag appended per exclusion, tallied once at the end. A counter per declaration
      # would be six variables naming the six declarations twice; a tally per *file* would
      # be a process per file, which is the shape this file's own header warns about.
      MJ_CHANGED_EXCLUDED_TAGS="$MJ_CHANGED_EXCLUDED_TAGS$MJ_CHANGED_WHY"$'\n'
      continue
    fi
    printf '%s\n' "$f"; MJ_CHANGED_KEPT=$((MJ_CHANGED_KEPT + 1))
  done <<EOF
$(if [ -n "$base" ] && [ "$base" != NONE ]; then mj_git_touched "$base"
   else mj_git status --porcelain=v1 2>/dev/null | cut -c4- | sed 's/^.* -> //' | LC_ALL=C sort -u | sed '/^$/d'; fi)
EOF
  return 0
}

# The same list as the block a record's front matter carries. One writer, so that a record
# and the command that explains it can never disagree about the indentation either.
#
# Not a pipeline. `mj_changed_files | sed` ran the classifier in a subshell, so every count
# it set died with that subshell and MJ_CHANGED_EXCLUDED was readable by nobody: the
# variable existed, was documented, and was always 0 at every call site. A redirection to a
# file is not a subshell, which is what lets the caller state the denominator.
mj_changed_files_block() {
  local t f
  t="$(mktemp "${TMPDIR:-/tmp}/mj.cb.XXXXXX")"
  mj_changed_files "$@" > "$t"
  while IFS= read -r f; do [ -n "$f" ] && printf '  - %s\n' "$f"; done < "$t"
  rm -f "$t"
  return 0
}

# What the classifier left out, as the record states it: the count, and which declaration
# did it. Printed after the changed_files block by every writer, 0 included — a list shown
# without its denominator is indistinguishable from a tree that was clean, which is the
# misreading the whole of this file exists to stop.
#
# Call it only after mj_changed_files or mj_changed_files_block, whose counts it renders.
mj_changed_excluded_block() {
  printf 'changed_files_excluded: %s\n' "$MJ_CHANGED_EXCLUDED"
  [ "$MJ_CHANGED_EXCLUDED" = 0 ] && return 0
  printf 'changed_files_excluded_by:\n'
  printf '%s' "$MJ_CHANGED_EXCLUDED_TAGS" | sed '/^$/d' | LC_ALL=C sort | uniq -c \
    | awk '{ printf "  - %s=%s\n", $2, $1 }'
  return 0
}

# ---------------------------------------------------------------- the doctrine
# A record names the work, not the exhaust — read back off the records that exist.
#
# Two verdicts, because the corpus holds two populations and one verdict over both can only
# be wrong in one direction. Every record written before the classifier carries the
# unclassified list, and a record is immutable by contract: rewriting eighteen committed
# records so that a check goes green would be fabricating history, which is the one thing
# the recovery work this validator arrived with refuses to do. So those are reported by name
# and count, and never block.
#
# A record written *by the classifier* that still names a declared-derived path is not a
# legacy: it is a regression, and it blocks. The two are told apart by the record itself —
# `changed_files_excluded` is written by every writer on this path, 0 included, and by
# nothing that came before it — so the grandfather boundary is a property of the object
# rather than a date constant or a list of eighteen filenames somebody has to maintain. It
# closes on its own as the old records age out of the stores.
#
# The residual is a record produced by neither writer: a hand-authored one would carry no
# key and be classified legacy. That is why `test/cases/281` asserts the writers always emit
# the key, and `test/cases/136` asserts what they exclude — a missing key is then only
# producible by something that is not the tool, which the session-records doctrine already
# forbids.
#
# The counts are what make either finding actionable. "Some records name derived files" is a
# sentence nobody can act on; "this record names 26 of them, the newest is from today" tells
# a reader whether the classifier is working.
mj_validate_record_changed_files() {
  local d f n bad=0 total=0 worst=0 worst_f="" newest="" live=0 live_worst=0 live_f=""
  for d in "$(mj_session_store)" "$MJ_STATE_DIR/checkpoints" "$MJ_STATE_DIR/handovers"; do
    [ -d "$d" ] || continue
    for f in "$d"/*.md; do
      [ -f "$f" ] || continue
      mj_is_context_doc "$f" && continue
      total=$((total + 1))
      n=0
      while IFS= read -r p; do
        [ -n "$p" ] || continue
        mj_changed_is_derived "$p" && n=$((n + 1))
      done <<LIST
$(awk '/^changed_files:$/ { c = 1; next } c && /^  - / { sub(/^  - /, ""); print; next } c { exit }' "$f")
LIST
      [ "$n" -gt 0 ] || continue
      if grep -q '^changed_files_excluded:' "$f"; then
        # written by the classifier and still naming what a declaration covers
        live=$((live + 1))
        [ "$n" -gt "$live_worst" ] && { live_worst="$n"; live_f="$f"; }
      else
        bad=$((bad + 1))
        [ "$n" -gt "$worst" ] && { worst="$n"; worst_f="$f"; }
        newest="$f"
      fi
    done
  done
  if [ "$bad" -gt 0 ]; then
    mj_warn records "$(mj_rel "$worst_f")" \
      "$bad of $total record(s) predate lib/changed.sh and name generated or local state as the episode's work product; the worst names $worst, and the newest is $(mj_rel "$newest"). Grandfathered: a record is immutable, and rewriting one to turn a check green would fabricate history"
  fi
  if [ "$live" -gt 0 ]; then
    mj_doctrine_fail records "$(mj_rel "$live_f")" \
      "$live record(s) written by the classifier still name declared-derived paths as the work product; the worst names $live_worst. This is a regression, not a legacy — the writer emitted changed_files_excluded and kept the paths anyway" \
      "majordomus doctrine show majordomus.record-changed-files"
  elif [ "$bad" = 0 ]; then
    mj_doctrine_ok records "$total record(s)" "every changed_files names work, not generated or local state"
  fi
  return 0
}
