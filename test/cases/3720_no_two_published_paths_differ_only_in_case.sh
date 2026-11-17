# majordomus-covers: none
# No two published paths differ only in case (I3447). A case-insensitive filesystem, the default
# on a Mac, holds only one of two such paths, so the archive of a publication that carries both
# does not extract there and a deploy from that disk keeps one of them. The UI snapshot of
# 82ac259d0ae carried issues/_status/ready beside issues/_status/READY, because /ui/work/ linked
# the whole list by the lowercased status.
#
# What is proved: scripts/site-check's casefold check (13l) refuses a publication holding two
# paths equal once case is folded, naming both and only the topmost of a nested clash; a
# publication whose paths differ in more than case passes; a missing publication is exit 12; and
# /ui/work/ links the issue list by the status as the plan declares it. The publication is given
# as a listing, because the clash cannot be created on the case-insensitive disk a Mac runs on.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/site-check"
L="$PWD/listing"

# --- a clash is refused, naming both paths, once
printf '%s\n' index.html issues issues/_status issues/_status/READY issues/_status/READY/index.html \
  issues/_status/ready issues/_status/ready/index.html > "$L"
rc=0; "$CHECK" --casefold "$L" > out.txt 2>&1 || rc=$?
[ "$rc" = 10 ] || { echo "    a clash exited $rc, not 10"; cat out.txt; exit 1; }
grep -qF 'FAIL casefold     /issues/_status/READY and /issues/_status/ready differ only in case' out.txt \
  || { echo "    the refusal does not name both paths:"; cat out.txt; exit 1; }
[ "$(grep -c '^FAIL casefold' out.txt)" = 1 ] || { echo "    a nested clash was reported more than once:"; cat out.txt; exit 1; }

# --- a file clash beside a directory clash: each named
printf '%s\n' a/X/index.html a/x/index.html ui/Work.html ui/work.html > "$L"
"$CHECK" --casefold "$L" > out.txt 2>&1 || true
grep -qF '/a/X/index.html and /a/x/index.html' out.txt && grep -qF '/ui/Work.html and /ui/work.html' out.txt \
  || { echo "    two separate clashes were not both named:"; cat out.txt; exit 1; }

# --- negative: paths that differ in more than case pass
printf '%s\n' index.html issues/_status/READY/index.html issues/_status/BLOCKED/index.html readme README.md > "$L"
expect_exit 0 "$CHECK" --casefold "$L"

# --- a built directory is read the same way, and no publication is exit 12
mkdir -p pub/a pub/b && : > pub/a/index.html && : > pub/b/index.html
expect_exit 0 "$CHECK" --casefold "$PWD/pub"
expect_exit 12 "$CHECK" --casefold "$PWD/no-such-publication"

# --- /ui/work/ links the whole list by the status as declared, not lowercased
W="$ROOT/apps/majordomus-cli/src/ui/work.rs"
grep -qE 'format!\("\{\}\?status=\{\}", super::issues::ROUTE, status\)' "$W" \
  || { echo "    /ui/work/ does not link the issue list by the declared status"; exit 1; }
! grep -q 'status.to_lowercase()' "$W" || { echo "    /ui/work/ still lowercases the status in a link"; exit 1; }

echo "    casefold: two published paths equal once case is folded are refused, naming both"
