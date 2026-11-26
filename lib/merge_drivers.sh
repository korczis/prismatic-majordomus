# shellcheck shell=bash
# The merge drivers an adopting clone declares for what generators write (I4770).
#
# A file a generator writes whole, or a document that holds generated regions, conflicts on
# nearly every merge when git merges it as text: both sides carry what their generator
# computed from their own tree, and the merged tree is neither. Majordomus's own repository
# solved that for itself with `merge=derived` (.gitattributes, scripts/merge-derived); an
# adopter got nothing of it. This converges the same answer into any clone `update` runs in:
#
#   share/libexec/merge-derived   the driver: `ours` for a generated file, `regions` for a
#                                 document whose generated regions are taken from ours while
#                                 the text people wrote merges three ways
#   git config merge.majordomus-derived.* / merge.majordomus-regions.*   in the clone's config
#   $GIT_COMMON_DIR/info/attributes, between two markers   which paths use which driver
#
# Both live in the clone and nowhere else: nothing tracked changes, so a personal layer and a
# shared one converge the same way, and a path the repository's own .gitattributes already
# gives a merge attribute is left to it.
#
# Which paths, derived rather than listed:
#   generated whole   the scope's out.generated.paths and out.generated.names, and every
#                     projection update writes in file mode
#   generated regions every tracked text file holding a region begin line (merge-derived's
#                     two spellings), and every projection update writes in region mode
MJ_MERGE_BEGIN='# >>> majordomus merge drivers (majordomus update) >>>'
MJ_MERGE_END='# <<< majordomus merge drivers <<<'
mj_merge_attributes_file() { printf '%s/info/attributes' "$(mj_git_repo_id)"; }
mj_merge_driver_path() { printf '%s/libexec/merge-derived' "$MJ_SHARE_DIR"; }

# the managed block's current lines; nothing without a block
mj_merge_block_current() {
  local f; f="$(mj_merge_attributes_file)"
  [ -f "$f" ] || return 0
  awk -v b="$MJ_MERGE_BEGIN" -v e="$MJ_MERGE_END" '$0==b{inb=1;next} $0==e{inb=0;next} inb' "$f"
}

# the attributes file with the managed block taken out, on stdout
mj_merge_attributes_without_block() {
  local f; f="$(mj_merge_attributes_file)"
  [ -f "$f" ] || return 0
  awk -v b="$MJ_MERGE_BEGIN" -v e="$MJ_MERGE_END" '$0==b{inb=1;next} $0==e{inb=0;next} !inb' "$f"
}

# the patterns of files a generator writes whole, one a line
mj_merge_whole_patterns() {
  local flat i=0
  if [ -n "${MJ_SCOPE_FILE:-}" ] && [ -f "$MJ_SCOPE_FILE" ]; then
    flat="$(mktemp "${TMPDIR:-/tmp}/mj.mg.XXXXXX")"
    if mj_yaml_flatten "$MJ_SCOPE_FILE" > "$flat" 2>/dev/null; then
      # a path is the repository's, anchored at its root; a name is a basename at any depth
      mj_ylist "$flat" out.generated.paths | sed -E '/^$/d; s|^/?|/|'
      mj_ylist "$flat" out.generated.names
    fi
    rm -f "$flat"
  fi
  while [ -n "$(mj_pol "projections.$i.target")" ]; do
    [ "$(mj_projection_mode "$i")" = file ] && printf '/%s\n' "$(mj_pol "projections.$i.target")"
    i=$((i+1))
  done
}

# the tracked text files that hold a generated region, and the region-mode projections
mj_merge_region_paths() {
  local i=0
  mj_git grep -lIiE -e '^[[:space:]]*<!--[[:space:]]*[A-Za-z0-9_.-]+:begin([[:space:]].*)?-->[[:space:]]*$' \
    -e '^#[[:space:]]*>>>[[:space:]].*(generated|majordomus|do not edit).*[[:space:]]>>>[[:space:]]*$' 2>/dev/null || true
  while [ -n "$(mj_pol "projections.$i.target")" ]; do
    [ "$(mj_projection_mode "$i")" = region ] && mj_pol "projections.$i.target"
    i=$((i+1))
  done
}

# a pattern in gitattributes syntax, given as it is to be written: a path anchored at the root
# with a leading slash (unanchored, a pattern without a slash would match the same name in
# every directory, and resolve a hand-written file of that name to ours), a name pattern as a
# basename glob. A pattern with whitespace cannot be written and is skipped, never quoted wrong.
mj_merge_attr_line() {
  case "$1" in *[[:space:]]*|"") return 1 ;; esac
  printf '%s merge=%s\n' "$1" "$2"
}

# whether the repository itself already decides how path $1 merges (its .gitattributes, with
# the managed block out of the way so the answer is the repository's and not ours)
mj_merge_repo_decides() {
  local v; v="$(mj_git check-attr merge -- "${1#/}" 2>/dev/null | sed -E 's/^.*: merge: //')"
  # our own block's answer is not the repository's: a dry run asks with it still in place
  case "$v" in ""|unspecified|majordomus-derived|majordomus-regions) return 1 ;; esac
  return 0
}

# the block this clone should carry, on stdout, sorted and without duplicates
mj_merge_block_wanted() {
  local p
  {
    while IFS= read -r p; do
      [ -n "$p" ] || continue
      mj_merge_repo_decides "$p" && continue
      mj_merge_attr_line "$p" majordomus-derived || true
    done < <(mj_merge_whole_patterns | LC_ALL=C sort -u)
    while IFS= read -r p; do
      [ -n "$p" ] || continue
      mj_merge_repo_decides "$p" && continue
      mj_merge_attr_line "/$p" majordomus-regions || true
    done < <(mj_merge_region_paths | LC_ALL=C sort -u)
  } | awk '!seen[$1]++'
}

# Bring the clone to the drivers and the block it should have. Idempotent: a second run
# changes nothing and says so. --dry-run prints one plan line in update's own vocabulary,
# `unchanged merge-drivers` or `update merge-drivers`, and writes nothing.
# mj_merge_drivers_converge [--dry-run]   -> prints one line, returns 0
mj_merge_drivers_converge() {
  local dry=0 f stripped wanted current driver changed=0 n
  [ "${1:-}" = --dry-run ] && dry=1
  # not a git work tree: there is no clone to declare a driver in
  mj_git rev-parse --git-dir >/dev/null 2>&1 || return 0
  f="$(mj_merge_attributes_file)"; driver="$(mj_merge_driver_path)"
  [ -x "$driver" ] || { printf 'merge drivers: %s is not an executable here; nothing declared\n' "$driver" >&2; return 0; }
  stripped="$(mktemp "${TMPDIR:-/tmp}/mj.ma.XXXXXX")"; wanted="$(mktemp "${TMPDIR:-/tmp}/mj.mw.XXXXXX")"
  current="$(mktemp "${TMPDIR:-/tmp}/mj.mc.XXXXXX")"
  mj_merge_attributes_without_block > "$stripped"
  mj_merge_block_current > "$current"
  # ask the repository with our old block out of the way, so its own answer decides
  if [ "$dry" = 0 ] && [ -s "$current" ]; then
    mkdir -p "$(dirname "$f")"; cp "$stripped" "$f"
  fi
  mj_merge_block_wanted > "$wanted"
  cmp -s "$wanted" "$current" || changed=1
  local cfg_d cfg_r want_d want_r
  want_d="$driver ours %O %A %B %P"; want_r="$driver regions %O %A %B %P"
  cfg_d="$(mj_git config --get merge.majordomus-derived.driver 2>/dev/null || true)"
  cfg_r="$(mj_git config --get merge.majordomus-regions.driver 2>/dev/null || true)"
  [ "$cfg_d" = "$want_d" ] && [ "$cfg_r" = "$want_r" ] || changed=1
  n="$(grep -c . "$wanted" || true)"
  if [ "$dry" = 1 ]; then
    if [ "$changed" = 1 ]; then printf 'update merge-drivers\n'; else printf 'unchanged merge-drivers\n'; fi
    rm -f "$stripped" "$wanted" "$current"; return 0
  fi
  if [ "$changed" = 0 ]; then
    [ -s "$current" ] && { mkdir -p "$(dirname "$f")"; { cat "$stripped"; printf '%s\n' "$MJ_MERGE_BEGIN"; cat "$current"; printf '%s\n' "$MJ_MERGE_END"; } > "$f.mj-tmp" && mv "$f.mj-tmp" "$f"; }
    printf 'merge drivers: unchanged (%s path pattern(s))\n' "$n"
    rm -f "$stripped" "$wanted" "$current"; return 0
  fi
  mj_git config merge.majordomus-derived.name "generated file: resolve to ours, regenerate before committing" >/dev/null 2>&1
  mj_git config merge.majordomus-derived.driver "$want_d" >/dev/null 2>&1
  mj_git config merge.majordomus-regions.name "generated regions: ours; the text people wrote merges three ways" >/dev/null 2>&1
  mj_git config merge.majordomus-regions.driver "$want_r" >/dev/null 2>&1
  mkdir -p "$(dirname "$f")"
  {
    cat "$stripped"
    if [ -s "$wanted" ]; then printf '%s\n' "$MJ_MERGE_BEGIN"; cat "$wanted"; printf '%s\n' "$MJ_MERGE_END"; fi
  } > "$f.mj-tmp" && mv "$f.mj-tmp" "$f"
  printf 'merge drivers: declared %s path pattern(s) in %s, drivers in this clone'"'"'s git config\n' "$n" "$f"
  rm -f "$stripped" "$wanted" "$current"
}
