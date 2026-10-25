# majordomus-covers: none
# Rule project.portable-shell sets the floor every shell unit runs on: bash 3.2 and BSD
# userland, so no associative arrays, no mapfile and no GNU-only flags (I3862). Until this
# case the rule named only case 08, which scans for eval and network clients, so the floor
# was held by review alone while the proof graph counted the rule proven.
#
# Every tracked shell unit is read: a file under bin/, lib/, scripts/, share/, .githooks/ or
# test/ whose first line names a shell, and every test/cases/*.sh. A comment line is not
# read. Refused:
#
#   an associative array        declare, local or typeset with -A
#   a bash 4 builtin            mapfile and readarray
#   a GNU-only flag             stat -c, date -d and readlink -f, unless the same line carries
#                               the BSD form (stat -f, date -v or -j, or a fallback after ||),
#                               as `stat -c %Y f || stat -f %m f` does; grep -P, sed -r,
#                               xargs -r, find -printf and cp --parents always
#
# The scanner is held to a fixture first: one unit carrying each construct and one carrying
# only the guarded forms. A scanner that stopped finding them therefore cannot pass for a
# clean tree. shellcheck -x -s bash, the rule's last clause, is the shell-lint gate's, and
# this case holds the gate to being declared.
. "$ROOT/test/lib.sh"
command -v awk >/dev/null 2>&1 || { echo "    awk is not installed"; exit 1; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3480.XXXXXX")"
trap 'rm -rf "$S"' EXIT

# floor_findings FILE... -> one line per finding: `<file>:<line>: <what>`
floor_findings() {
  LC_ALL=C awk '
    FNR == 1 { file = FILENAME }
    /^[ \t]*#/ { next }
    {
      l = $0
      if (l ~ /(^|[^A-Za-z0-9_])(declare|local|typeset)[ \t]+-[A-Za-z]*A/)
        print file ":" FNR ": an associative array"
      if (l ~ /(^|[^A-Za-z0-9_.-])(mapfile|readarray)([ \t]|$)/)
        print file ":" FNR ": mapfile or readarray, a bash 4 builtin"
      if (l ~ /(^|[^A-Za-z0-9_.-])stat[ \t]+-c[ \t]/ && l !~ /stat[ \t]+-f[ \t]/)
        print file ":" FNR ": stat -c with no BSD stat -f on the line"
      if (l ~ /(^|[^A-Za-z0-9_.-])date[ \t]+-d[ \t]/ && l !~ /date[ \t]+-[vj]/)
        print file ":" FNR ": date -d with no BSD date -v or -j on the line"
      if (l ~ /(^|[^A-Za-z0-9_.-])readlink[ \t]+-f[ \t]/ && l !~ /[|][|]/)
        print file ":" FNR ": readlink -f with no fallback after ||"
      if (l ~ /(^|[^A-Za-z0-9_.-])grep[ \t]+-[A-Za-z]*P/)
        print file ":" FNR ": grep -P, which BSD grep does not have"
      if (l ~ /(^|[^A-Za-z0-9_.-])sed[ \t]+-r[ \t]/)
        print file ":" FNR ": sed -r, GNU for sed -E"
      if (l ~ /(^|[^A-Za-z0-9_.-])xargs[ \t]+-[A-Za-z]*r[ \t]/)
        print file ":" FNR ": xargs -r, which BSD xargs does not have"
      if (l ~ /(^|[^A-Za-z0-9_.-])find[ \t].*-printf/)
        print file ":" FNR ": find -printf, which BSD find does not have"
      if (l ~ /(^|[^A-Za-z0-9_.-])cp[ \t]+--parents/)
        print file ":" FNR ": cp --parents, which BSD cp does not have"
    }' "$@"
}

# ---------------------------------------------------------------- the scanner, on a known answer
# The fixture's constructs are assembled from parts, so this file holds none of them literally.
mkdir -p "$S/fixture"
{
  printf '#!/usr/bin/env bash\n'
  printf '%s -%s VERDICT=()\n' declare A
  printf 'f() { %s -%s seen; }\n' local A
  printf '%s lines < f\n' mapfile
  printf '%s -t lines < f\n' readarray
  printf 'm="$(stat -%s %%Y f)"\n' c
  printf 'd="$(date -%s yesterday)"\n' d
  printf 'p="$(readlink -%s f)"\n' f
  printf 'grep -%s x f\n' P
  printf 'sed -%s s/a/b/ f\n' r
  printf 'printf x | xargs -%s echo\n' r
  printf 'find . -name x -%s %%p\n' printf
  printf 'cp --%s a b\n' parents
} > "$S/fixture/bad.sh"
{
  printf '#!/usr/bin/env bash\n'
  printf '# %s -%s in a comment is not code\n' declare A
  printf 'declare -a ORDER=()\n'
  printf 'm="$(stat -%s %%Y f 2>/dev/null || stat -f %%m f)"\n' c
  printf 't="$(date -v-2H +%%s 2>/dev/null || date -%s %s +%%s)"\n' d "'2 hours ago'"
  printf 'p="$(readlink -%s f 2>/dev/null || printf %%s f)"\n' f
  printf 'grep -E x f\n'
} > "$S/fixture/good.sh"
floor_findings "$S/fixture/bad.sh" > "$S/bad.out"
want='an associative array
an associative array
mapfile or readarray, a bash 4 builtin
mapfile or readarray, a bash 4 builtin
stat -c with no BSD stat -f on the line
date -d with no BSD date -v or -j on the line
readlink -f with no fallback after ||
grep -P, which BSD grep does not have
sed -r, GNU for sed -E
xargs -r, which BSD xargs does not have
find -printf, which BSD find does not have
cp --parents, which BSD cp does not have'
got="$(sed 's/^[^:]*:[0-9]*: //' "$S/bad.out")"
[ "$got" = "$want" ] || {
  echo "    the scanner does not find what the fixture carries; it is not trustworthy:"
  sed 's/^/    | /' "$S/bad.out"; exit 1; }
floor_findings "$S/fixture/good.sh" > "$S/good.out"
[ ! -s "$S/good.out" ] || {
  echo "    the scanner refuses a guarded form or a comment:"; sed 's/^/    | /' "$S/good.out"; exit 1; }
echo "    the scanner finds every construct the fixture carries, and passes its guarded forms"

# ---------------------------------------------------------------- the tree
git -C "$ROOT" ls-files -- bin lib scripts share .githooks test > "$S/tracked"
: > "$S/units"
while IFS= read -r f; do
  [ -f "$ROOT/$f" ] || continue
  case "$f" in
    # this case: its scanner's messages name every construct it refuses, so it is the one
    # unit that cannot be read by it (the fixture above holds the scanner to its answer)
    test/cases/3480_the_shell_floor_is_bash_3_2_and_bsd_userland.sh) continue ;;
    test/cases/*.sh) printf '%s\n' "$ROOT/$f" >> "$S/units"; continue ;;
  esac
  case "$(head -n 1 "$ROOT/$f" 2>/dev/null)" in
    '#!'*sh|'#!'*'env bash'*|'#!'*'env sh'*) printf '%s\n' "$ROOT/$f" >> "$S/units" ;;
  esac
done < "$S/tracked"
n="$(wc -l < "$S/units" | tr -d ' ')"
[ "$n" -gt 0 ] || { echo "    no shell unit was found to read; the discovery is broken"; exit 1; }
: > "$S/tree.out"
while IFS= read -r u; do floor_findings "$u" >> "$S/tree.out"; done < "$S/units"
if [ -s "$S/tree.out" ]; then
  echo "    a shell unit breaks the floor project.portable-shell sets:"
  sed "s#^$ROOT/#    | #" "$S/tree.out"; exit 1
fi
echo "    every shell unit ($n read) stands on bash 3.2 and BSD userland"

# ---------------------------------------------------------------- shellcheck is the gate's half
awk '/^  - id: shell-lint$/ { g = 1; next } g && /^  - id: / { g = 0 } g && /^    runs: scripts\/ci\/shell-lint$/ { found = 1 } END { exit found ? 0 : 1 }' \
  "$ROOT/.ai/repo/ci/gates.yaml" \
  || { echo "    the shell-lint gate, which holds shellcheck -x -s bash, is not declared"; exit 1; }
echo "    shellcheck -x -s bash is the shell-lint gate's, and the gate is declared"
