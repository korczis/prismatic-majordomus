#!/usr/bin/env bash
# sourced by several commands; guard against re-sourcing
[ -n "${MJ_LIB_archive:-}" ] && return 0 || MJ_LIB_archive=1
# archive — a snapshot of this repository that can leave the machine.
#
# A tree has to travel: to a language model that will read all of it at once, to a
# reviewer who cannot clone, to an auditor who must run the gates somewhere other than
# the checkout. Every one of those was being done by hand, differently each time, and
# the hand-made version got two things wrong that are not obvious until much later.
#
# The first is what to leave out. A repository this size does not fit in a model's
# context, and the half that does not fit is the half that is generated: the projections
# under docs/generated/, site/data/ and site/content/docs/ are functions of canonical
# files that are already in the archive, so carrying them spends the reader's attention
# on the same statements twice. What is derived is not decided here — .gitattributes
# already marks every one of those paths `merge=derived`, for a different reason, and
# that declaration is read rather than restated. A second list would be the defect that
# `.ai/repo/rules/project/commands-are-projections.v1.md` exists to refuse.
#
# The second is fidelity. `zip -X` drops the extra fields that carry the Unix mode, so
# the unpacked tree has no executable anywhere, and every script in it fails to run for a
# reason that belongs to the archive rather than to the repository. That is the worst
# kind of failure: it is indistinguishable, at the far end, from the repository being
# broken. So the mode of every entry is read from the git index — the authority, not the
# working tree — written into the manifest, and a restore script ships beside it that
# puts the modes back and builds a local git index, because the checks that matter here
# read `git ls-files` and answer nothing without one.
#
# What is archived is the git index and only the git index. Nothing untracked travels:
# not the build output, not the caches, and not `.ai/local/`, which is this checkout's
# own state and is never shared. That is one rule rather than a list of exclusions, and
# it is the rule that makes the command safe to point at a repository nobody has read.
#
# The profiles are share/archive.yaml; a repository may add or replace one in
# .ai/repo/archive.yaml without redefining what an archive is.

# This file is sourced by a dispatcher that sets `-e`, so every pipeline here runs with
# the failure of anything but its last stage discarded. That matters more than usual for a
# command whose whole job is fidelity: a `tar` that could not read a file, piped into a
# `tar` that unpacked what it did get, would report success and produce a short archive.
# `set -o pipefail` is set for this command's run rather than trusted to the caller. Each
# pipeline below that may legitimately fail — a `head` closing a pipe early, an `xargs`
# over an empty list — says so with `|| true` rather than relying on the option's absence.
set -o pipefail

MJ_ARCHIVE_SHIPPED="$MJ_BIN_DIR/../share/archive.yaml"

# The repository's name, which is not this directory's name: a linked worktree is named
# after its branch, and an archive of it must still say which repository it is. The remote
# decides when there is one; the checkout's own directory is the fallback.
mj_archive_repo_name() {
  local url base
  url="$(mj_git config --get remote.origin.url 2>/dev/null || true)"
  if [ -n "$url" ]; then
    base="${url##*/}"; base="${base%.git}"
    [ -n "$base" ] && { printf '%s' "$base"; return 0; }
  fi
  printf '%s' "$(basename "$MJ_ROOT")"
}

mj_archive_usage() {
  cat <<H
usage: majordomus archive [<profile>] [--out <path>] [--format zip|tar.gz]
                          [--dry-run] [--force] [--json]
       majordomus archive --list [--json]

  a snapshot of the tracked tree, for reading or checking somewhere else

  <profile>   which snapshot (default: the registry's own default)
  --out       write here instead of tmp/archives/<repo>-<profile>-<date>.<ext>
  --format    override the profile's container
  --dry-run   report what would be archived and write nothing
  --force     overwrite an existing output file
  --list      the profiles, with what each one is for

  the file set is the git index; nothing untracked is ever archived. Every entry's mode
  is recorded from the index and restored by _ARCHIVE/restore.sh inside the archive.

  exit codes: 0 ok · 2 usage · 12 no such profile, or a tool is missing
              13 internal error · 15 refused (the output exists; --force to replace it)
H
}

mj_cmd_archive() {
  local profile="" out="" format="" dry=0 force=0 list=0
  while [ $# -gt 0 ]; do case "$1" in
    --out) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--out needs a path"; out="$2"; shift 2 ;;
    --out=*) out="${1#--out=}"; shift ;;
    --format) [ $# -ge 2 ] || mj_die "$MJ_EX_USAGE" "--format needs zip or tar.gz"; format="$2"; shift 2 ;;
    --format=*) format="${1#--format=}"; shift ;;
    --dry-run) dry=1; shift ;;
    --force) force=1; shift ;;
    --list) list=1; shift ;;
    --json) shift ;;
    --help|-h) mj_archive_usage; return 0 ;;
    -*) mj_die "$MJ_EX_USAGE" "archive: unknown option $1" ;;
    *) [ -z "$profile" ] || mj_die "$MJ_EX_USAGE" "archive: one profile only"
       case "$1" in *[!A-Za-z0-9._-]*|""|.*) mj_die "$MJ_EX_USAGE" "archive: '$1' is not a profile name" ;; esac
       profile="$1"; shift ;;
  esac; done

  mj_require_installed
  mj_archive_load
  [ "$list" = 1 ] && { mj_archive_list; return 0; }
  [ -n "$profile" ] || profile="$(mj_yget "$MJ_ARCHIVE_FLAT" default)"
  [ -n "$profile" ] || mj_die "$MJ_EX_INTERNAL" "$(mj_rel "$MJ_ARCHIVE_SHIPPED") declares no default profile"
  mj_archive_run "$profile" "$out" "$format" "$dry" "$force"
}

# ---------------------------------------------------------------- the declarations
# The shipped profiles, then the repository's own overlay. A profile the repository
# declares under the same id replaces the shipped one whole: a half-overridden profile
# would be a third thing neither file describes.
mj_archive_load() {
  [ -f "$MJ_ARCHIVE_SHIPPED" ] \
    || mj_die "$MJ_EX_INTERNAL" "no archive registry at $MJ_ARCHIVE_SHIPPED"
  MJ_ARCHIVE_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.arc.XXXXXX")"
  mj_yaml_flatten "$MJ_ARCHIVE_SHIPPED" > "$MJ_ARCHIVE_FLAT" 2>/dev/null \
    || mj_die "$MJ_EX_INTERNAL" "$(mj_rel "$MJ_ARCHIVE_SHIPPED") does not parse"
  MJ_ARCHIVE_LOCAL_FLAT=""
  local overlay="$MJ_AI_REPO_DIR/archive.yaml"
  if [ -f "$overlay" ]; then
    MJ_ARCHIVE_LOCAL_FLAT="$(mktemp "${TMPDIR:-/tmp}/mj.arcl.XXXXXX")"
    mj_yaml_flatten "$overlay" > "$MJ_ARCHIVE_LOCAL_FLAT" 2>/dev/null \
      || mj_die "$MJ_EX_CONTRACT" "$(mj_rel "$overlay") does not parse"
  fi
}

mj_archive_cleanup() { rm -f "${MJ_ARCHIVE_FLAT:-}" "${MJ_ARCHIVE_LOCAL_FLAT:-}"; }

# the flat file and index of a profile, as "<flat> <index>"; empty when no such profile
mj_archive_find() { # id
  local f i
  for f in "$MJ_ARCHIVE_LOCAL_FLAT" "$MJ_ARCHIVE_FLAT"; do
    [ -n "$f" ] || continue
    i=0
    while :; do
      case "$(mj_yget "$f" "profiles.$i.id")" in
        "") break ;;
        "$1") printf '%s %s\n' "$f" "$i"; return 0 ;;
      esac
      i=$((i + 1))
    done
  done
  return 1
}

mj_archive_ids() { # every profile id, the overlay's first, each one once
  local f i id seen=""
  for f in "$MJ_ARCHIVE_LOCAL_FLAT" "$MJ_ARCHIVE_FLAT"; do
    [ -n "$f" ] || continue
    i=0
    while id="$(mj_yget "$f" "profiles.$i.id")"; [ -n "$id" ]; do
      case " $seen " in *" $id "*) ;; *) seen="$seen $id"; printf '%s\n' "$id" ;; esac
      i=$((i + 1))
    done
  done
}

mj_archive_list() {
  local id spec flat idx n=0 def
  def="$(mj_yget "$MJ_ARCHIVE_FLAT" default)"
  [ "$MJ_JSON" = 1 ] && printf '{"schema":1,"default":"%s","profiles":[' "$(mj_json_esc "$def")"
  for id in $(mj_archive_ids); do
    spec="$(mj_archive_find "$id")" || continue
    flat="${spec%% *}"; idx="${spec##* }"
    if [ "$MJ_JSON" = 1 ]; then
      [ "$n" = 0 ] || printf ','
      printf '{"id":"%s","title":"%s","summary":"%s","format":"%s","source":"%s"}' \
        "$(mj_json_esc "$id")" \
        "$(mj_json_esc "$(mj_yget "$flat" "profiles.$idx.title")")" \
        "$(mj_json_esc "$(mj_yget "$flat" "profiles.$idx.summary")")" \
        "$(mj_json_esc "$(mj_yget "$flat" "profiles.$idx.format")")" \
        "$(mj_json_esc "$(mj_yget "$flat" "profiles.$idx.source")")"
    else
      printf '%-12s %s%s\n             %s\n' "$id" \
        "$(mj_yget "$flat" "profiles.$idx.title")" \
        "$([ "$id" = "$def" ] && printf '  (default)')" \
        "$(mj_yget "$flat" "profiles.$idx.summary")"
    fi
    n=$((n + 1))
  done
  if [ "$MJ_JSON" = 1 ]; then printf '],"count":%s}\n' "$n"
  else printf 'archive: %s profile(s)\n' "$n"; fi
  mj_archive_cleanup
}

# ---------------------------------------------------------------- selection
# The index, then three subtractions and one re-inclusion, in that order. Every decision
# is recorded against the path so that --dry-run can say why a file is not there: a
# selection that can only be believed is not evidence.
mj_archive_select() { # flat idx outfile tmpdir -> "<mode>\t<path>" lines; sets MJ_ARCHIVE_DROPPED
  local flat="$1" idx="$2" out="$3" tmpd="$4"
  local derived binary

  derived="$(mj_yget "$flat" "profiles.$idx.derived")"
  binary="$(mj_yget "$flat" "profiles.$idx.binary")"

  mj_git ls-files -s > "$tmpd/index" || mj_die "$MJ_EX_INTERNAL" "git ls-files failed"
  # "<mode> <sha> <stage>\t<path>" -> "<mode>\t<path>", gitlinks (160000) dropped: a
  # submodule's content is not in this index and cannot be archived from it
  awk -F"\t" '{ split($1, a, " "); if (a[1] != "160000") printf "%s\t%s\n", a[1], $2 }' \
    "$tmpd/index" > "$tmpd/all"
  MJ_ARCHIVE_TRACKED="$(mj_lines "$tmpd/all")"
  [ "$MJ_ARCHIVE_TRACKED" -gt 0 ] \
    || mj_die "$MJ_EX_CONTRACT" "the git index lists no file; there is nothing to archive"

  # derived: .gitattributes is the declaration, read through git itself
  : > "$tmpd/derived"
  if [ "$derived" = drop ]; then
    cut -f2 "$tmpd/all" | mj_git check-attr --stdin merge 2>/dev/null \
      | sed -n 's/: merge: derived$//p' > "$tmpd/derived" || true
  fi

  # binary: by extension, from the registry's one list
  : > "$tmpd/binext"
  [ "$binary" = drop ] && mj_ylist "$MJ_ARCHIVE_FLAT" binary_extensions > "$tmpd/binext"

  mj_ylist "$flat" "profiles.$idx.exclude" > "$tmpd/exclude"
  mj_ylist "$flat" "profiles.$idx.include" > "$tmpd/include"

  MJ_ARCHIVE_DROPPED="$tmpd/dropped"
  # One pass for every decision. The shape this replaced ran two greps per tracked file
  # and took forty seconds on this repository, which is long enough that a person stops
  # running it — and a check nobody runs is not a check.
  awk -F"\t" -v D="$tmpd/derived" -v B="$tmpd/binext" \
             -v E="$tmpd/exclude" -v I="$tmpd/include" \
             -v KEEP="$out" -v DROP="$tmpd/dropped" '
    # a shell glob as an anchored regular expression: * and ? only, everything else
    # literal. * matches / as well, so "apps/**" and "apps/*" select the same subtree.
    function globre(p,   r, i, c) {
      r = ""
      for (i = 1; i <= length(p); i++) {
        c = substr(p, i, 1)
        if (c == "*") r = r ".*"
        else if (c == "?") r = r "."
        else if (index(".^$+()[]{}|\\/-", c)) r = r "\\" c
        else r = r c
      }
      return "^" r "$"
    }
    FILENAME == D { der[$0] = 1; next }
    FILENAME == B { bex[$0] = 1; nbex++; next }
    FILENAME == E { exc[++ne] = globre($0); next }
    FILENAME == I { inc[++ni] = globre($0); next }
    {
      mode = $1; path = $2; drop = ""
      if (path in der) drop = "derived"
      if (drop == "" && nbex && match(path, /\.[^.\/]+$/)) {
        if (substr(path, RSTART + 1) in bex) drop = "binary"
      }
      if (drop == "") {
        for (i = 1; i <= ne; i++) if (path ~ exc[i]) { drop = "excluded"; break }
      }
      if (drop != "") {
        for (i = 1; i <= ni; i++) if (path ~ inc[i]) { drop = ""; break }
      }
      if (drop != "") printf "%s\t%s\n", drop, path >> DROP
      else printf "%s\t%s\n", mode, path >> KEEP
    }' "$tmpd/derived" "$tmpd/binext" "$tmpd/exclude" "$tmpd/include" "$tmpd/all"
  # awk creates neither output file when it writes no line to it
  [ -f "$out" ] || : > "$out"
  [ -f "$MJ_ARCHIVE_DROPPED" ] || : > "$MJ_ARCHIVE_DROPPED"
}

# ---------------------------------------------------------------- the run
mj_archive_run() { # profile out format dry force
  local profile="$1" out="$2" format="$3" dry="$4" force="$5"
  local spec flat idx ext stage sel tmp n bytes missing=0 rc=0

  spec="$(mj_archive_find "$profile")" || {
    mj_archive_cleanup
    mj_die "$MJ_EX_MISSING" "archive: no profile '$profile' (run: majordomus archive --list)"; }
  flat="${spec%% *}"; idx="${spec##* }"

  [ -n "$format" ] || format="$(mj_yget "$flat" "profiles.$idx.format")"
  case "$format" in
    zip) ext=zip ;;
    tar.gz) ext=tar.gz ;;
    *) mj_archive_cleanup; mj_die "$MJ_EX_USAGE" "archive: format '$format' is not zip or tar.gz" ;;
  esac

  sel="$(mktemp "${TMPDIR:-/tmp}/mj.arcs.XXXXXX")"
  # The working directory is made here, in the function that removes it, so that every
  # recursive delete below is visibly of a path this function took from mktemp and of
  # nothing else (SECURITY.md: no recursive deletion; test/cases/08 holds the shape).
  # MJ_ARCHIVE_TMPD names the same directory for mj_archive_furnish.
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/mj.arcsel.XXXXXX")"
  MJ_ARCHIVE_TMPD="$tmp"
  mj_archive_select "$flat" "$idx" "$sel" "$tmp"
  n="$(mj_lines "$sel")"
  [ "$n" -gt 0 ] || {
    rm -f "$sel"; mj_archive_cleanup
    mj_die "$MJ_EX_CONTRACT" "profile '$profile' selects none of $MJ_ARCHIVE_TRACKED tracked file(s)"; }
  # `|| true`: a path in the index that the working tree no longer has makes `wc` exit
  # non-zero, and the size of the content is a thing to report, never a thing to fail on.
  # The missing paths are counted and reported separately, where they mean something.
  bytes="$( { cut -f2 "$sel" | (cd "$MJ_ROOT" && tr '\n' '\0' | xargs -0 wc -c 2>/dev/null) || true; } \
    | awk '$2 != "total" { s += $1 } END { printf "%d", s + 0 }')"

  if [ "$dry" = 1 ]; then
    mj_archive_report "$profile" "$flat" "$idx" "$n" "$bytes" "" "$ext"
    rm -f "$sel"; rm -rf "$tmp"; mj_archive_cleanup
    return 0
  fi

  if [ -z "$out" ]; then
    out="$MJ_ROOT/tmp/archives/$(mj_archive_repo_name)-$profile-$(date -u +%Y%m%d).$ext"
  fi
  case "$out" in /*) ;; *) out="$PWD/$out" ;; esac
  if [ -e "$out" ] && [ "$force" != 1 ]; then
    rm -f "$sel"; rm -rf "$tmp"; mj_archive_cleanup
    mj_die "$MJ_EX_REFUSED" "archive: $out exists (--force to replace it)"
  fi
  mkdir -p "$(dirname "$out")" || mj_die "$MJ_EX_INTERNAL" "cannot create $(dirname "$out")"

  stage="$MJ_ARCHIVE_TMPD/stage/$(mj_archive_repo_name)"
  mkdir -p "$stage"
  # copied through tar so that one process reads the list; a path the index names and the
  # working tree does not have is counted and reported, never silently absent
  local mode path
  : > "$MJ_ARCHIVE_TMPD/copy"
  while IFS="$MJ_TAB" read -r mode path; do
    if [ -f "$MJ_ROOT/$path" ]; then printf '%s\n' "$path" >> "$MJ_ARCHIVE_TMPD/copy"
    else missing=$((missing + 1)); fi
  done < "$sel"
  ( cd "$MJ_ROOT" && tar cf - -T "$MJ_ARCHIVE_TMPD/copy" ) | ( cd "$stage" && tar xf - ) \
    || mj_die "$MJ_EX_INTERNAL" "could not copy the selected files"
  # the index is the authority on the mode, not the working tree. Two batched calls, not
  # one process per file: this runs over every tracked path.
  awk -F"\t" '$1 == "100755" { print $2 }' "$sel" > "$MJ_ARCHIVE_TMPD/x"
  awk -F"\t" '$1 != "100755" { print $2 }' "$sel" > "$MJ_ARCHIVE_TMPD/r"
  ( cd "$stage" && tr '\n' '\0' < "$MJ_ARCHIVE_TMPD/x" | xargs -0 chmod 755 2>/dev/null ) || true
  ( cd "$stage" && tr '\n' '\0' < "$MJ_ARCHIVE_TMPD/r" | xargs -0 chmod 644 2>/dev/null ) || true

  mj_archive_furnish "$stage" "$profile" "$flat" "$idx" "$sel" "$n" "$missing"
  mj_archive_pack "$stage" "$out" "$ext" || rc=$?
  [ "$rc" = 0 ] || { rm -f "$sel"; rm -rf "$tmp"; mj_archive_cleanup; return "$rc"; }
  mj_archive_verify "$out" "$ext" "$stage" || rc=$?

  mj_archive_report "$profile" "$flat" "$idx" "$n" "$bytes" "$out" "$ext"
  [ "$missing" = 0 ] || mj_warn archive "$profile" \
    "$missing tracked path(s) are in the index and not in the working tree; they are not in the archive" \
    "git status --porcelain"
  rm -f "$sel"; rm -rf "$tmp"; mj_archive_cleanup
  return "$rc"
}

# ---------------------------------------------------------------- what travels with it
# Three files the reader at the far end needs and cannot reconstruct: where the tree came
# from, what is in it with what mode, and how to make it runnable again.
mj_archive_furnish() { # stage profile flat idx sel n missing
  local stage="$1" profile="$2" flat="$3" idx="$4" sel="$5" n="$6" missing="$7"
  local a="$stage/_ARCHIVE" i line
  mkdir -p "$a"

  { printf 'repository: %s\n' "$(basename "$MJ_ROOT")"
    printf 'commit:     %s\n' "$(mj_git rev-parse HEAD 2>/dev/null || echo unknown)"
    printf 'branch:     %s\n' "$(mj_git_branch)"
    printf 'tree:       %s\n' "$(mj_git_dirty)"
    printf 'remote:     %s\n' "$(mj_git remote get-url origin 2>/dev/null || echo none)"
    printf 'profile:    %s\n' "$profile"
    printf 'taken:      %s\n' "$(mj_now)"
    printf 'tool:       majordomus %s\n' "$MJ_VERSION"
    printf '\n'
    printf 'tracked in the repository: %s\n' "$MJ_ARCHIVE_TRACKED"
    printf 'in this archive:           %s\n' "$n"
    printf 'left out:                  %s\n' "$(mj_lines "$MJ_ARCHIVE_DROPPED")"
    printf 'named but not on disk:     %s\n' "$missing"
    printf '\n=== the last 80 commits ===\n'
    mj_git log --oneline -n 80 2>/dev/null || true
    printf '\n=== tags, newest first ===\n'
    mj_git tag --sort=-creatordate 2>/dev/null | head -40 || true
  } > "$a/GIT.txt"

  { printf '# %s — %s\n\n' "$(mj_archive_repo_name)" "$(mj_yget "$flat" "profiles.$idx.title")"
    printf '%s\n\n' "$(mj_yget "$flat" "profiles.$idx.summary")"
    i=0
    while line="$(mj_yget "$flat" "profiles.$idx.orientation.$i")"; [ -n "$line" ]; do
      printf '%s\n\n' "$line"; i=$((i + 1))
    done
    printf '## What is here\n\n'
    printf '| | |\n|---|---|\n'
    printf '| Profile | `%s` — %s |\n' "$profile" "$(mj_yget "$flat" "profiles.$idx.note")"
    printf '| Commit | `%s` on `%s` |\n' "$(mj_git rev-parse HEAD 2>/dev/null || echo unknown)" "$(mj_git_branch)"
    printf '| Files | %s of %s tracked |\n' "$n" "$MJ_ARCHIVE_TRACKED"
    printf '| Generated projections | %s |\n' \
      "$( [ "$(mj_yget "$flat" "profiles.$idx.derived")" = drop ] && printf 'left out — they are functions of files that are here' || printf 'kept' )"
    printf '| Binary files | %s |\n' \
      "$( [ "$(mj_yget "$flat" "profiles.$idx.binary")" = drop ] && printf 'left out' || printf 'kept' )"
    printf '\n`_ARCHIVE/GIT.txt` is where this came from, `_ARCHIVE/MANIFEST.txt` every file with its mode and size'
    [ "$(mj_yget "$flat" "profiles.$idx.restore")" = true ] \
      && printf ', and `_ARCHIVE/restore.sh` puts the modes back and creates a local git index'
    printf '.\n\n'
    printf '## What was left out, and why\n\n'
    if [ -s "$MJ_ARCHIVE_DROPPED" ]; then
      printf '| Reason | Files |\n|---|---|\n'
      cut -f1 "$MJ_ARCHIVE_DROPPED" | LC_ALL=C sort | uniq -c \
        | awk '{ printf "| %s | %s |\n", $2, $1 }'
      printf '\n`derived` means the path is marked `merge=derived` in `.gitattributes`'
      printf ' — a projection of a canonical file that is in this archive.\n\n'
    else
      printf 'Nothing tracked was left out.\n\n'
    fi
    printf 'Untracked files are never archived, so the build output, the caches and'
    printf ' `.ai/local/` — this checkout\047s own state — are absent by construction rather than by exclusion.\n'
  } > "$a/README.md"

  if [ "$(mj_yget "$flat" "profiles.$idx.restore")" = true ]; then
    mj_archive_restore_script > "$a/restore.sh"
    chmod 755 "$a/restore.sh"
  fi

  # Last, so that it lists every other file this function wrote — the script that reads
  # it included, whose mode is the one that matters most. It cannot list itself.
  #
  # One find, one wc, one awk: a `wc -c` per file would be another two thousand
  # processes for a file nobody reads twice.
  local t="$MJ_ARCHIVE_TMPD"
  ( cd "$stage" && find . -type f | sed 's|^\./||' | LC_ALL=C sort ) > "$t/mf.files"
  ( cd "$stage" && find . -type f -perm -u+x | sed 's|^\./||' | LC_ALL=C sort ) > "$t/mf.exec"
  ( cd "$stage" && tr '\n' '\0' < "$t/mf.files" | xargs -0 wc -c ) > "$t/mf.sizes"
  awk -v X="$t/mf.exec" -v S="$t/mf.sizes" '
    FILENAME == X { x[$0] = 1; next }
    { size = $1; sub(/^[ \t]*[0-9]+[ \t]+/, ""); if ($0 == "total") next
      printf "%s %9d  %s\n", ($0 in x ? "100755" : "100644"), size, $0 }' \
    "$t/mf.exec" "$t/mf.sizes" | LC_ALL=C sort -k3 > "$a/MANIFEST.txt"
}

# POSIX sh: it runs wherever the archive is opened, which is not this machine
mj_archive_restore_script() {
  cat <<'RESTORE'
#!/bin/sh
# restore.sh — make this unpacked archive behave like a checkout again.
#
# Two things are lost between `zip` and the far end. The file modes go when the reader
# discards the extra fields that carry them, and every script in the tree stops being
# executable. The git index was never there at all, and the repository's own checks read
# it — `git ls-files` is how this project enumerates itself, so without an index they do
# not fail, they examine nothing and report that nothing is wrong.
#
# This restores both, from _ARCHIVE/MANIFEST.txt, which recorded the mode of every entry
# as the original git index held it. Run it once, from the directory it sits in:
#
#     sh _ARCHIVE/restore.sh
#
# It creates one commit with no history and no remote. Anything that reads more than the
# working tree will therefore say so rather than being quietly wrong.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
man="$here/MANIFEST.txt"

[ -f "$man" ] || { echo "restore: no MANIFEST.txt beside this script" >&2; exit 1; }
cd "$root"

modes=0
missing=0
while read -r mode size path; do
  [ -n "${path:-}" ] || continue
  if [ ! -f "$path" ]; then missing=$((missing + 1)); continue; fi
  case "$mode" in
    100755) chmod 755 "$path" ;;
    *)      chmod 644 "$path" ;;
  esac
  modes=$((modes + 1))
done < "$man"

echo "restore: modes applied to $modes file(s) from $(basename "$man")"
[ "$missing" = 0 ] || echo "restore: $missing file(s) in the manifest are not in this tree"

if ! command -v git >/dev/null 2>&1; then
  echo "restore: git is not on PATH; the modes are restored, the index is not" >&2
  exit 0
fi
if [ -d .git ]; then
  echo "restore: .git already exists here; leaving it alone"
  exit 0
fi

git init -q .
git add -A
git -c user.email=restore@localhost -c user.name=restore \
    commit -q -m "the archived tree, restored" --no-verify
n="$(git ls-files | wc -l | tr -d ' ')"
echo "restore: a git index of $n file(s), one commit, no history and no remote"
echo "restore: the original commit and branch are in _ARCHIVE/GIT.txt"
RESTORE
}

# ---------------------------------------------------------------- container and proof
mj_archive_pack() { # stage out ext
  local stage="$1" out="$2" ext="$3" parent base
  parent="$(dirname "$stage")"; base="$(basename "$stage")"
  case "$ext" in
    zip)
      mj_has zip || { mj_fail archive container "zip is not on PATH" "majordomus archive --format tar.gz"; return "$MJ_EX_MISSING"; }
      # deliberately not -X: the extra fields it strips are the Unix modes, and an
      # archive whose scripts are not executable fails for reasons of its own
      ( cd "$parent" && zip -q -r -y "$out" "$base" ) \
        || { mj_fail archive container "zip failed writing $out" "cd $parent && zip -r out.zip $base"; return "$MJ_EX_INTERNAL"; } ;;
    tar.gz)
      ( cd "$parent" && tar czf "$out" "$base" ) \
        || { mj_fail archive container "tar failed writing $out" "cd $parent && tar czf out.tar.gz $base"; return "$MJ_EX_INTERNAL"; } ;;
  esac
  return 0
}

# An archive is not written until it has been read back. The count on disk and the count
# in the container must agree, and the mode of a known executable must have survived the
# round trip — that is the failure this whole file exists to answer, so it is proved here
# rather than assumed.
mj_archive_verify() { # out ext stage
  local out="$1" ext="$2" stage="$3" want got probe mode
  want="$(cd "$stage" && find . -type f | wc -l | tr -d ' ')"
  case "$ext" in
    zip)
      got="$(unzip -Z1 "$out" 2>/dev/null | grep -cv '/$' || true)"
      probe="$(unzip -Z "$out" 2>/dev/null | awk '$1 ~ /^-rwx/ { n++ } END { printf "%d", n + 0 }')" ;;
    tar.gz)
      got="$(tar tzf "$out" 2>/dev/null | grep -cv '/$' || true)"
      probe="$(tar tzvf "$out" 2>/dev/null | awk '$1 ~ /^-rwx/ { n++ } END { printf "%d", n + 0 }')" ;;
  esac
  [ -n "$got" ] && [ "$got" -gt 0 ] || {
    mj_fail archive "$(basename "$out")" "the container reads back as empty; $want file(s) were staged" \
      "unzip -l $out"; return "$MJ_EX_INTERNAL"; }
  [ "$got" = "$want" ] || {
    mj_fail archive "$(basename "$out")" "$want file(s) staged, $got in the container" \
      "unzip -l $out"; return "$MJ_EX_INTERNAL"; }
  mode="$(cd "$stage" && find . -type f -perm -u+x | wc -l | tr -d ' ')"
  if [ "$mode" -gt 0 ] && [ "$probe" = 0 ]; then
    mj_fail archive "$(basename "$out")" \
      "$mode executable file(s) went in and none came back out: the container discarded the modes" \
      "unzip -Z $out | head"
    return "$MJ_EX_INTERNAL"
  fi
  mj_ok archive "$(basename "$out")" "$got entr(ies) read back, $probe of them executable"
  return 0
}

# ---------------------------------------------------------------- saying what happened
mj_archive_report() { # profile flat idx n bytes out ext
  local profile="$1" flat="$2" idx="$3" n="$4" bytes="$5" out="$6" ext="$7"
  local dropped size=0
  dropped="$(mj_lines "$MJ_ARCHIVE_DROPPED")"
  [ -n "$out" ] && [ -f "$out" ] && size="$(wc -c < "$out" | tr -d ' ')"
  if [ "$MJ_JSON" = 1 ]; then
    printf '{"schema":1,"profile":"%s","format":"%s","tracked":%s,"archived":%s,"left_out":%s,"content_bytes":%s,"path":"%s","bytes":%s,"reasons":{' \
      "$(mj_json_esc "$profile")" "$(mj_json_esc "$ext")" "$MJ_ARCHIVE_TRACKED" "$n" "$dropped" "$bytes" \
      "$(mj_json_esc "$([ -n "$out" ] && mj_rel "$out")")" "$size"
    cut -f1 "$MJ_ARCHIVE_DROPPED" | LC_ALL=C sort | uniq -c \
      | awk '{ printf "%s\"%s\":%s", (NR > 1 ? "," : ""), $2, $1 }'
    printf '}}\n'
    return 0
  fi
  printf 'archive: profile %s — %s of %s tracked file(s), %s KB of content\n' \
    "$profile" "$n" "$MJ_ARCHIVE_TRACKED" "$((bytes / 1024))"
  if [ "$dropped" -gt 0 ]; then
    cut -f1 "$MJ_ARCHIVE_DROPPED" | LC_ALL=C sort | uniq -c \
      | awk '{ printf "         left out: %s (%s)\n", $1, $2 }'
  fi
  if [ -n "$out" ]; then
    printf '         wrote %s (%s KB)\n' "$(mj_rel "$out")" "$((size / 1024))"
  else
    printf '         nothing written (--dry-run)\n'
  fi
}
