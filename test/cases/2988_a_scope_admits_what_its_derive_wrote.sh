# majordomus-covers: check finish
# claims: scope-enforcement
# A task scopes the canonical file it changes. The projections a derive writes from it are
# the generator's to place, and the repository already says which paths those are: git's
# `merge=derived` attribute. The scope check admits such a path when the task also changed
# something canonical, and judges everything else as it always did.
#
# What it must not become:
#
#   a way around the scope. A projection committed with no canonical change beside it was
#   written by nothing this task did, and is refused as before.
#
#   a second list. The check asks git for the attribute; a repository that declares nothing
#   derived admits nothing.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib gen other
echo a > lib/a; echo '{}' > gen/x.json; echo '{}' > "gen/a b.json"; echo o > out.md; echo z > other/z
printf 'gen/** merge=derived\nout.md merge=derived\n' > .gitattributes
git add -A >/dev/null; git commit -qm base
"$MJ" start "t" --scope lib >/dev/null
OUT="$(mktemp "${TMPDIR:-/tmp}/mj.case.XXXXXX")"   # outside the tree: a file here would be a touched path
expect_exit 0 "$MJ" check

# ---------------------------------------------------------------- projections alone: refused, as before
echo '{"n":1}' > gen/x.json
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +gen/x.json — outside claimed scope \(lib\)'

# ---------------------------------------------------------------- beside the canonical change: admitted
echo b > lib/a
echo '{"n":1}' > "gen/a b.json"; echo p > out.md
expect_exit 0 "$MJ" check
expect_grep 'OK +scope .* 1 touched file\(s\) within scope, and 3 derived from them \(merge=derived\)'
expect_no_grep 'outside claimed scope'

# ---------------------------------------------------------------- outside both: refused exactly as before
echo y > other/z
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +other/z — outside claimed scope \(lib\)'
expect_no_grep 'FAIL scope +gen/'
expect_no_grep 'FAIL scope +out.md'
git checkout -q -- other/z

# ---------------------------------------------------------------- committed, and at finish
git add -A >/dev/null; git commit -qm "the change and what the derive wrote" >/dev/null
expect_exit 0 "$MJ" check
expect_grep '3 derived from them \(merge=derived\)'
"$MJ" finish --check > "$OUT" 2>&1 || true
grep -q 'outside claimed scope' "$OUT" && { echo "    finish --check refused a derived path"; grep scope "$OUT"; exit 1; }
grep -Eq 'OK +scope ' "$OUT" || { echo "    finish --check did not pass the scope"; grep -i scope "$OUT"; exit 1; }

# ---------------------------------------------------------------- nothing declared derived: nothing admitted
git rm -q --cached .gitattributes >/dev/null; rm -f .gitattributes
git commit -qm "no declaration" >/dev/null
"$MJ" check > "$OUT" 2>&1 && { echo "    a path nothing declares derived was admitted"; exit 1; }
grep -Eq 'FAIL scope +gen/x.json — outside claimed scope' "$OUT" \
  || { echo "    the undeclared projection was not refused by name"; grep scope "$OUT"; exit 1; }
grep -Eq 'FAIL scope +gen/a b.json — outside claimed scope' "$OUT" \
  || { echo "    a path with a space was not reported whole"; grep scope "$OUT"; exit 1; }
exit 0
