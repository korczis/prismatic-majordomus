# majordomus-covers: none
# claims: derived-data-current
# A fixture whose object database has no history still gets a verdict on its derived data
# (I1409, evidence fixture_without_history_evaluates).
#
# Case 56 clones rather than exports because an export failed: the published changelog is
# projected from the commits between release records, a `git archive` into a fresh `git init`
# holds the tree and none of those commits, and `majordomus generate --check` composed a
# changelog with every section emptied, then called the committed one stale. A verdict the
# fixture could not have reached is not a verdict; and the same composition, written by
# `majordomus generate` in a shallow or exported checkout, replaced a whole changelog with a
# husk. This case holds the repaired behaviour on an export of this commit:
#
#   1  the export really has no history: a release record names a commit it does not hold,
#      so what follows is not vacuous
#   2  both halves of `scripts/pages current` pass, and the gate says that the changelog was
#      kept as committed and not judged, naming the history it lacks
#   3  `majordomus generate` writes nothing over the committed tree: no husk of a changelog
#   4  the negative control: a source file the registry indexes, moved, makes the gate exit 10
#      and name what is stale, so 2 is not a gate that has stopped judging
. "$ROOT/test/lib.sh"

RB="$(rust_bin)" || rust_bin_exit $?

W="$T/export"
mkdir -p "$W"
git -C "$ROOT" archive HEAD | tar -x -C "$W" || { echo "    could not export HEAD"; exit 1; }
cd "$W" || exit 1
# a repository, not a bare tree: the registry half discovers its index with `git ls-files`
git init -q . && git config user.email t@example.com && git config user.name t \
  && git add -A && git commit -q -m "an export of one commit, without its history" \
  || { echo "    could not make the export a repository"; exit 1; }

# 1 -------------------------------------------------------------------------------------
named="$(sed -n 's/^commit:[[:space:]]*\([0-9a-f]\{40\}\).*/\1/p' .ai/repo/releases/*.yaml | head -n 1)"
[ -n "$named" ] || { echo "    no release record names a commit; the case has nothing to hold"; exit 1; }
if git cat-file -e "$named^{commit}" 2>/dev/null; then
  echo "    the export holds release commit $named; it has history and proves nothing"; exit 1
fi
[ "$(git rev-list --count HEAD)" = 1 ] || { echo "    the export has more than one commit"; exit 1; }

# 2 -------------------------------------------------------------------------------------
rc=0; out="$(MAJORDOMUS_BIN="$RB" scripts/pages current 2>&1)" || rc=$?
[ "$rc" = 0 ] || {
  echo "    a current tree without history was not judged current (exit $rc)"
  printf '    | %s\n' "$out"; exit 1; }
case "$out" in
  *"is current for this tree"*) ;;
  *) echo "    the site half did not report the tree as current: $out"; exit 1 ;;
esac
case "$out" in
  *"registry projections are in sync"*) ;;
  *) echo "    the registry half was not judged: $out"; exit 1 ;;
esac
case "$out" in
  *changelog*"not judged"*) ;;
  *) echo "    the gate did not say the changelog went unjudged for want of history: $out"; exit 1 ;;
esac

# 3 -------------------------------------------------------------------------------------
MAJORDOMUS_BIN="$RB" "$RB" generate --repo "$W" >/dev/null 2>"$T/generate.err" || {
  echo "    generate failed in the export"; sed 's/^/    | /' "$T/generate.err"; exit 1; }
[ -z "$(git status --porcelain)" ] || {
  echo "    generate without history rewrote the committed tree:"
  git status --porcelain | sed 's/^/    | /'; exit 1; }

# 4 -------------------------------------------------------------------------------------
printf '\n# a line that moves the registry\n' >> lib/context_docs.sh
rc=0; out="$(MAJORDOMUS_BIN="$RB" scripts/pages current 2>&1)" || rc=$?
[ "$rc" = 10 ] || {
  echo "    a moved source file did not make the gate exit 10 (got $rc)"
  printf '    | %s\n' "$out"; exit 1; }
case "$out" in
  *registry.json*) ;;
  *) echo "    the refusal does not name what is stale: $out"; exit 1 ;;
esac
exit 0
