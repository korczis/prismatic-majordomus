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

# The declared-derived pathspecs, one per line, cached for the process. Reading them all
# once per record is cheap; reading them once per *file* in a hundred-file tree is the shape
# that cost the scope validator a second before it hoisted its own read out of the loop.
MJ_DERIVED_PATHS=""; MJ_DERIVED_NAMES=""; MJ_DERIVED_LOADED=0
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
      MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS$(mj_ylist "$flat" out.generated.paths)"$'\n'
      MJ_DERIVED_NAMES="$MJ_DERIVED_NAMES$(mj_ylist "$flat" out.generated.names)"$'\n'
      # The never-read set, of which .ai/local/ is the entry that matters here. The whole
      # set is taken rather than that one path: a repository that declares another tree
      # unreadable has said the same thing about it, and a work product nobody may read is
      # not a work product.
      MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS$(mj_ylist "$flat" out.paths)"$'\n'
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
    MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS$t"$'\n'; j=$((j + 1))
  done

  # 4. the record stores. Nothing declares these as derived because they are not: a closed
  # session record is a tracked object of the layer and the executable indexes it. They are
  # excluded for a different reason — the tool writes them, the worker does not. The record
  # that started this file named its own three sibling records and its own site projection
  # among the 122 files it claimed to have changed. The paths come from the store resolvers
  # rather than from a list, so a repository whose manifest puts the sessions section
  # somewhere else gets the right answer without anybody remembering to update this.
  if command -v mj_session_store >/dev/null 2>&1; then
    MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS$(mj_rel "$(mj_session_store)")/"$'\n'
  fi
  MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS$(mj_rel "$MJ_STATE_DIR")/checkpoints/"$'\n'
  MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS$(mj_rel "$MJ_STATE_DIR")/handovers/"$'\n'

  # 5. the repository's own derived trees, from the merge driver's declaration. The
  # attribute line is `<pathspec><whitespace>merge=derived`; a pathspec with a space in it
  # would be quoted, and none is, so the first field is the whole of it.
  if [ -f "$MJ_ROOT/.gitattributes" ]; then
    while IFS= read -r t; do
      [ -n "$t" ] && MJ_DERIVED_PATHS="$MJ_DERIVED_PATHS$t"$'\n'
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
mj_changed_is_derived() {
  local f="$1" p n
  mj_changed_load_declarations
  while IFS= read -r p; do
    [ -n "$p" ] || continue
    case "$p" in
      */'**') [ -z "${f##"${p%'**'}"*}" ] && return 0 ;;
      */'*')  [ -z "${f##"${p%'*'}"*}" ] && return 0 ;;
      */)     [ -z "${f##"$p"*}" ] && return 0 ;;
      *)      [ "$f" = "$p" ] && return 0
              [ -z "${f##"$p"/*}" ] && return 0 ;;
    esac
  done <<EOF
$MJ_DERIVED_PATHS
EOF
  while IFS= read -r n; do
    [ -n "$n" ] || continue
    # shellcheck disable=SC2254  # the pattern is the declaration; that is the whole point
    case "${f##*/}" in $n) return 0 ;; esac
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
MJ_CHANGED_EXCLUDED=0
mj_changed_files() {
  local base="${1:-}" f n=0
  MJ_CHANGED_EXCLUDED=0
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    if mj_changed_is_derived "$f"; then MJ_CHANGED_EXCLUDED=$((MJ_CHANGED_EXCLUDED + 1)); continue; fi
    printf '%s\n' "$f"; n=$((n + 1))
  done <<EOF
$(if [ -n "$base" ] && [ "$base" != NONE ]; then mj_git_touched "$base"
   else mj_git status --porcelain=v1 2>/dev/null | cut -c4- | sed 's/^.* -> //' | LC_ALL=C sort -u | sed '/^$/d'; fi)
EOF
  return 0
}

# The same list as the block a record's front matter carries. One writer, so that a record
# and the command that explains it can never disagree about the indentation either.
mj_changed_files_block() {
  mj_changed_files "$@" | sed 's/^/  - /'
  return 0
}

# ---------------------------------------------------------------- the doctrine
# A record names the work, not the exhaust — read back off the records that exist.
#
# Advisory, and the reason is not timidity. Every record already written carries the
# unclassified list, and a record is immutable by contract: rewriting thirteen committed
# session records so that a check goes green would be fabricating history, which is the one
# thing the recovery work this validator arrived with refuses to do. So the records that
# exist are reported and the new ones are held to it by test/cases/136, which fails the
# build if a record written today names a declared-derived path.
#
# The count is what makes the finding actionable. "Some records name derived files" is a
# sentence nobody can act on; "this record names 26 of them, the newest is from today" tells
# a reader whether the classifier is working.
mj_validate_record_changed_files() {
  local d f n bad=0 total=0 worst=0 worst_f="" newest=""
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
      if [ "$n" -gt 0 ]; then
        bad=$((bad + 1))
        [ "$n" -gt "$worst" ] && { worst="$n"; worst_f="$f"; }
        newest="$f"
      fi
    done
  done
  if [ "$bad" = 0 ]; then
    mj_doctrine_ok records "$total record(s)" "every changed_files names work, not generated or local state"
  else
    mj_doctrine_fail records "$(mj_rel "$worst_f")" \
      "$bad of $total record(s) name generated or local state as the episode's work product; the worst names $worst, and the newest is $(mj_rel "$newest") — a record written before lib/changed.sh is not rewritten, because a record is immutable" \
      "majordomus session show \$(sed -n 's/^session_id: //p' $(mj_rel "$worst_f") | head -n 1)"
  fi
  return 0
}
