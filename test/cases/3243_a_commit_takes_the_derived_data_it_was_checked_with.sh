# majordomus-covers: none
# A commit takes the derived data it was checked with (I2859). `scripts/pages current` reads
# the working tree and a commit takes the index: a derived file staged from one derive and
# rewritten by the next (status MM) passed the pre-commit hook while the commit carried the
# older copy (db7a99455d4, corrected by a255ff14f70). With --staged, the hook's form, a path
# .gitattributes marks `merge=derived` whose index differs from its working tree refuses.
#
#   1  a clean clone passes, with and without --staged
#   2  a stale derived file in the index under a current working tree passes the plain check:
#      the blind spot, kept as the control that the case sets up what it claims to
#   3  the same tree refuses with --staged, naming the file
#   4  an unstaged change to a file that is not derived does not refuse
#
# It runs on a clone of this checkout, as case 56 does: the derived data and the attributes
# that mark it exist nowhere else. Nothing in the checkout itself is written.
. "$ROOT/test/lib.sh"

W="$T/w"
git clone -q --shared "$ROOT" "$W" 2>/dev/null || { echo "    could not clone the checkout"; exit 1; }
(cd "$W" && git config user.email t@example.com && git config user.name t) \
  || { echo "    could not configure the clone"; exit 1; }
SRC="site/data/generated/source.json"
case "$(git -C "$W" check-attr merge -- "$SRC")" in
  *": merge: derived") ;;
  *) echo "    $SRC is not marked merge=derived; this case needs a derived file"; exit 1 ;;
esac

# ---------------------------------------------------------------- 1. a clean clone
(cd "$W" && scripts/pages current >/dev/null 2>&1) \
  || { echo "    the committed tree is not current; run scripts/derive and commit the result"; exit 1; }
(cd "$W" && scripts/pages current --staged >/dev/null 2>&1) \
  || { echo "    --staged refused a clean clone"; exit 1; }

# ---------------------------------------------------------------- 2. the blind spot
jq '.source_hash = "0000000000000000000000000000000000000000000000000000000000000000"' \
  "$W/$SRC" > "$T/stale.json"
cp "$T/stale.json" "$W/$SRC"
git -C "$W" add -- "$SRC"
git -C "$W" show "HEAD:$SRC" > "$W/$SRC"
(cd "$W" && scripts/pages current >/dev/null 2>&1) \
  || { echo "    the plain check refused a current working tree; the setup is not the blind spot"; exit 1; }

# ---------------------------------------------------------------- 3. --staged refuses
rc=0; out="$(cd "$W" && scripts/pages current --staged 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    --staged answered $rc over a stale index; expected 10"; printf '    | %s\n' "$out"; exit 1; }
case "$out" in
  *"differ between the index and the working tree"*"$SRC"*) ;;
  *) echo "    the refusal does not name $SRC"; printf '    | %s\n' "$out"; exit 1 ;;
esac

# ---------------------------------------------------------------- 4. only derived files count
git -C "$W" add -- "$SRC"   # the index takes the current copy back
echo "# a person's unstaged edit" >> "$W/README.md"
(cd "$W" && scripts/pages current --staged >/dev/null 2>&1) \
  || { echo "    --staged refused an unstaged change to a file that is not derived"; exit 1; }

echo "    a commit carries the derived data it was checked with: a stale index refuses with --staged, the plain check and other files are unchanged"
