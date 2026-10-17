# majordomus-covers: none
# claims: derived-data-current
# A committed derivation survives a merge that touches none of its inputs (I1409, evidence
# derivation_survives_an_unrelated_merge).
#
# The 2026-09-10 failure was a derived artifact that hashed the whole tree, so the next merge
# invalidated it "not because it is wrong, but because the tree moved". The site data is now a
# projection of a declared input list (scripts/generate-site-data --inputs) fingerprinted by
# content, and the registry half by the files the generator reads. This case holds that
# property from both sides, on a clone of this commit:
#
#   1  the committed tree is current (case 56 owns the failure if it is not)
#   2  a side branch and a trunk commit, each adding a file outside every canonical input, are
#      merged: the fingerprint is unchanged and `scripts/pages current` still passes
#   3  the merge's first parent, the commit before it, is still current: a derivation committed
#      at one commit describes that commit after the trunk moves on
#   4  the negative control: a commit that touches one canonical input makes the tree stale
#      (exit 10) and moves the fingerprint, so 2 and 3 are not vacuous
#
# The registry half of `scripts/pages current` runs where MAJORDOMUS_BIN names an executable
# (CI); without one it is reported as skipped and the site half alone is held.
. "$ROOT/test/lib.sh"

W="$T/tree"
git clone -q --no-local "$ROOT" "$W" 2>/dev/null || { echo "    could not clone HEAD"; exit 1; }
cd "$W" || exit 1
git config user.email t@example.com && git config user.name t \
  || { echo "    could not configure the clone"; exit 1; }

current() { scripts/pages current >/dev/null 2>&1; }
fingerprint() { scripts/pages fingerprint 2>/dev/null | tail -n 1; }
is_input() { scripts/generate-site-data --inputs | grep -qxF "$1"; }

# 1 -------------------------------------------------------------------------------------
current || { echo "    the committed tree is not current; case 56 owns that failure"; exit 1; }
before="$(fingerprint)"
[ -n "$before" ] || { echo "    scripts/pages fingerprint printed nothing"; exit 1; }

# 2 -------------------------------------------------------------------------------------
for f in unrelated-side.txt unrelated-trunk.txt; do
  is_input "$f" && { echo "    $f is a canonical input; the case needs a path outside them"; exit 1; }
done
trunk="$(git rev-parse --abbrev-ref HEAD)"
git checkout -q -b side \
  && echo side > unrelated-side.txt && git add unrelated-side.txt && git commit -q -m side \
  && git checkout -q "$trunk" \
  && echo trunk > unrelated-trunk.txt && git add unrelated-trunk.txt && git commit -q -m trunk \
  && git merge -q --no-ff side -m "an unrelated merge" \
  || { echo "    could not build the unrelated merge"; exit 1; }
after="$(fingerprint)"
[ "$after" = "$before" ] || {
  echo "    a merge touching no canonical input moved the fingerprint: ${before:0:12} -> ${after:0:12}"; exit 1; }
current || {
  echo "    the derived data went stale across a merge that touches none of its inputs"
  scripts/pages current 2>&1 | sed 's/^/    | /'; exit 1; }

# 3 -------------------------------------------------------------------------------------
git checkout -q HEAD^1 || exit 1
current || { echo "    the commit before the merge no longer reads as current"; exit 1; }
git checkout -q "$trunk" || exit 1

# 4 -------------------------------------------------------------------------------------
input="$(scripts/generate-site-data --inputs | grep '^\.ai/repo/project/issues/' | head -n 1)"
[ -n "$input" ] || { echo "    no issue record among the canonical inputs"; exit 1; }
printf '\n# a change to a canonical input\n' >> "$input"
git commit -q -am "a canonical input changes" || { echo "    could not commit the control"; exit 1; }
rc=0; scripts/pages current >/dev/null 2>&1 || rc=$?   # the runner uses bash -e
[ "$rc" = 10 ] || { echo "    a changed canonical input should read as stale (exit 10), got $rc"; exit 1; }
[ "$(fingerprint)" != "$before" ] || { echo "    a changed canonical input left the fingerprint unchanged"; exit 1; }
exit 0
