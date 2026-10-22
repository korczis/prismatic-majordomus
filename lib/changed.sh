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
  mj_changed_build_ere
  return 0
}

# The declarations' glob language, translated once into one anchored ERE.
#
# The pathspecs come from three vocabularies — scope.yaml's glob subset, a policy target
# that is a plain path, a gitattributes pattern — and all three are written in globset's
# syntax, which is what `apps/majordomus-cli/src/scope.rs` matches the same declarations
# with: `**` crosses separators, `*` does not, and a character class is a class.
#
# This reader used to implement three shapes of that syntax — a plain path, a trailing `/`,
# a trailing `/**` or `/*` — and to match anything else literally. That was described as
# the safe direction, and it is not one: a declaration written in any other shape excluded
# nothing at all, silently, and three declarations in this repository are written in shapes
# it could not read. Measured on the twenty-two records that exist:
#
#   site/content/*.md                  .gitattributes, whose managed block scripts/gitattributes
#                                      writes from the site generator's own publish list — so the
#                                      tracked, generated pages site/content/_index.md,
#                                      architecture.md and changelog.md counted as work
#   .ai/repo/benchmarks/**/baseline.*  scope.yaml out.generated.paths: a ratchet baseline, which
#                                      a benchmark run rewrites and no worker authors
#   **/target/ and eight siblings      scope.yaml out.paths. These leak nothing today, because
#                                      git does not report an ignored path as changed and every
#                                      one of them is ignored — but the declaration said one
#                                      thing and the reader did another, which is the defect
#                                      whether or not it is currently reachable
#
# What replaces it is a translation over the syntax, not a longer list of shapes: a
# declaration form nobody anticipated is translated too. The alternation is built once per
# process and applied with one match per file, which is also cheaper than walking 179
# pathspecs for every path in the tree.
#
# A trailing `/` means the directory and everything under it. A pathspec with no wildcard
# at all keeps the reading it had — the path, or anything under it — because a policy target
# is a plain path and `docs/generated` must still cover the tree below it.
mj_changed_glob_ere() {
  local p="$1" out="" i=0 c wild=0 inclass=0
  case "$p" in */) p="$p**" ;; esac
  while [ "$i" -lt "${#p}" ]; do
    c="${p:$i:1}"
    if [ "$inclass" = 1 ]; then
      out="$out$c"; [ "$c" = ']' ] && inclass=0; i=$((i + 1)); continue
    fi
    case "$c" in
      '[') out="${out}["; inclass=1; wild=1 ;;
      '*') wild=1
           if [ "${p:$i:2}" = '**' ]; then
             # `**/` at a segment boundary may match no segment at all, so that a
             # declaration of `**/target/` covers a target/ directory at the root too
             if [ "${p:$i:3}" = '**/' ]; then out="$out(.*/)?"; i=$((i + 2))
             else out="$out.*"; i=$((i + 1)); fi
           else out="${out}[^/]*"; fi ;;
      '?') out="${out}[^/]"; wild=1 ;;
      '.' | '^' | '$' | '+' | '(' | ')' | '{' | '}' | '|' | '\') out="$out\\$c" ;;
      *) out="$out$c" ;;
    esac
    i=$((i + 1))
  done
  [ "$wild" = 0 ] && out="$out(/.*)?"
  printf '%s' "$out"
}

# One alternation over every declared pathspec. Empty when a repository declares none, and
# the caller then excludes nothing — the same answer as before, reached without a match.
MJ_DERIVED_ERE=""
mj_changed_build_ere() {
  local alt="" t e
  while IFS= read -r t; do
    [ -n "$t" ] || continue
    e="$(mj_changed_glob_ere "$t")"
    [ -n "$e" ] || continue
    alt="${alt:+$alt|}$e"
  done <<EOF
$MJ_DERIVED_PATHS
EOF
  [ -n "$alt" ] && MJ_DERIVED_ERE="^($alt)\$"
  return 0
}

# Is repository-relative path $1 covered by one of the declarations? 0 yes, 1 no.
mj_changed_is_derived() {
  local f="$1" n
  mj_changed_load_declarations
  # shellcheck disable=SC2076  # the declaration is a pattern; quoting it would match it literally
  if [ -n "$MJ_DERIVED_ERE" ] && [[ "$f" =~ $MJ_DERIVED_ERE ]]; then return 0; fi
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
#
# Not a pipeline. `mj_changed_files | sed` ran the classifier in a subshell, so the count it
# left in MJ_CHANGED_EXCLUDED died with that subshell and no caller could ever read it —
# the comment above the variable promised a number that nothing was able to produce. The
# list goes through a file instead, and the note below is the thing that promise was for.
mj_changed_files_block() {
  local tmp; tmp="$(mktemp "${TMPDIR:-/tmp}/mj.cb.XXXXXX")"
  mj_changed_files "$@" > "$tmp"
  sed 's/^/  - /' "$tmp"
  rm -f "$tmp"
  mj_changed_excluded_note
  return 0
}

# What the filter left out, said out loud.
#
# A filter that silently shortens a list is indistinguishable from a tree that was cleaner
# than it was, and a record is evidence: a reader must be able to tell "this episode changed
# four files" from "this episode changed four files and regenerated ninety". The count goes
# to stderr, never into the record — the record's front matter is a closed contract whose
# allow-list refuses an unknown key, so a new key there would make every record this version
# writes unreadable to a checkout running the last one, and the derived half is in any case
# recoverable from git over the very commit range the record already names.
mj_changed_excluded_note() {
  [ "${MJ_CHANGED_EXCLUDED:-0}" -gt 0 ] || return 0
  printf '%s: %s derived or checkout-local path(s) classified out of changed_files\n' \
    "${MJ_SELF##*/}" "$MJ_CHANGED_EXCLUDED" >&2
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
