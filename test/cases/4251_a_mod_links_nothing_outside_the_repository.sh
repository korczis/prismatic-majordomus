# majordomus-covers: capture
# A mod `capture install` writes into an adopting repository links only to what that repository
# holds or to an absolute URL (I4251). The brief mod linked `../../../../docs/BRIEF.md`, which
# resolves inside the tool's own checkout and, once written to `.claude/skills/majordomus-brief/`
# of an adopter, four levels up: outside the repository, so its link check failed on a file the
# adopter did not write.
#
#   every relative Markdown link in every file the install writes resolves inside the repository;
#   every relative link of every mod under share/mods/ stays inside the mod's own directory,
#   which is what makes the first hold wherever a mod is installed;
#   the check can fail: a mod copy with an escaping link is refused.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add . && git commit -qm base

# escaping <root> <file>...: print every relative link that leaves <root>
escaping() {
  local root="$1" f dir target resolved bad=0; shift
  root="$(cd "$root" && pwd -P)"
  for f in "$@"; do
    dir="$(cd "$(dirname "$f")" && pwd -P)"
    for target in $(grep -oE '\]\([^)#[:space:]]+' "$f" | sed 's/^](//' | grep -vE '^[a-z][a-z0-9+.-]*:'); do
      resolved="$(cd "$dir" && python3 -c 'import os,sys; print(os.path.normpath(os.path.join(os.getcwd(), sys.argv[1])))' "$target")"
      case "$resolved/" in "$root"/*) ;; *) echo "$f: $target leaves $root"; bad=1 ;; esac
    done
  done
  return "$bad"
}

expect_exit 0 "$MJ" capture install --provider claude-code
written="$(find .claude -name '*.md' -type f)"
[ -n "$written" ] || { echo "capture install wrote no Markdown under .claude" >&2; false; }
# shellcheck disable=SC2086
escaping "$T" $written || { echo "an installed mod links outside the repository" >&2; false; }

for mod in "$ROOT"/share/mods/*/*/; do
  # shellcheck disable=SC2046
  escaping "$mod" $(find "$mod" -name '*.md' -type f) \
    || { echo "a mod under share/mods links outside its own directory" >&2; false; }
done

# the check can fail
mkdir -p "$T/badmod/sub"; printf 'see [x](../../../../docs/BRIEF.md)\n' > "$T/badmod/sub/README.md"
if escaping "$T/badmod" "$T/badmod/sub/README.md" >/dev/null; then
  echo "an escaping link was not refused" >&2; false
fi
echo "ok: installed mods and every packaged mod link only inside the repository"
