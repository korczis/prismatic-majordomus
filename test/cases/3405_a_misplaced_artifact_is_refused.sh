# majordomus-covers: init update
# A file in the half whose declared layout excludes it is refused, naming the path and the
# contract (I2871, I1102). A fresh repository is adopted with `init`, whose skeleton
# declares the tracked half canonical (excludes cache, runtime) and the local half runtime
# (excludes canonical); then each kind of misplacement is planted and must be refused, and
# each kind of rightful content must pass:
#
#   clean      a fresh adoption passes, exit 0
#   copy       a tracked canonical file copied into .ai/local         -> exit 10, names it
#   declared   a file in .ai/local declaring a canonical kind's schema -> exit 10
#   runtime    a session context in .ai/local (a runtime kind)         -> passes
#   ignored    a tracked file under .ai/repo the ignore rules match    -> exit 10, names ai.repo
#   update     a projection target outside the repository              -> refused, nothing written
# claims: none
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

expect_exit 0 "$MJ" init
git add -A && git commit -qm adopt

check() { MAJORDOMUS_SHARE="$ROOT/share" "$RB" layout check --repo "$T" "$@"; }
verdict() { local rc=0; check > out 2>&1 || rc=$?; printf '%s' "$rc"; }

rc="$(verdict)"
[ "$rc" = 0 ] || { echo "    a fresh adoption is refused (exit $rc)"; cat out; exit 1; }
echo "    ok: a fresh adoption passes"

# --- copy: the policy, authored knowledge, copied into the local half
cp .ai/repo/policy.yaml .ai/local/policy-copy.yaml
rc="$(verdict)"
[ "$rc" = 10 ] && grep -q '.ai/local/policy-copy.yaml violates manifest local: excludes canonical' out \
  && grep -q 'a copy of .ai/repo/policy.yaml' out \
  || { echo "    a copy of the policy in the local half was not refused (exit $rc)"; cat out; exit 1; }
rm .ai/local/policy-copy.yaml
echo "    ok: a copy of a tracked canonical file in the local half is refused, naming the original"

# --- declared: a document that says it is a context contract, written into the local half
mkdir -p .ai/local/notes
printf -- '---\nschema: context/v1\nid: stray\nkind: context\n---\n\n# Stray\n' > .ai/local/notes/README.md
rc="$(verdict)"
[ "$rc" = 10 ] && grep -q '.ai/local/notes/README.md violates manifest local: excludes canonical' out \
  || { echo "    a canonical document in the local half was not refused (exit $rc)"; cat out; exit 1; }
rm -r .ai/local/notes
echo "    ok: a file declaring a canonical kind in the local half is refused"

# --- runtime: a session context is the local half's own state
mkdir -p .ai/local/session-contexts
printf -- '---\nschema: session-context/v1\nsession_id: s-1\ntitle: one session\n---\n\nnotes\n' \
  > .ai/local/session-contexts/s-1.md
rc="$(verdict)"
[ "$rc" = 0 ] || { echo "    a session context in the local half was refused (exit $rc)"; cat out; exit 1; }
echo "    ok: a runtime kind in the local half passes"

# --- ignored: a scratch file the ignore rules match, forced into the tracked half
printf 'scratch\n' > .ai/repo/.tmp.scratch
git add -f .ai/repo/.tmp.scratch && git commit -qm 'a scratch file, forced'
rc="$(verdict)"
[ "$rc" = 10 ] && grep -q '.ai/repo/.tmp.scratch violates ai.repo: excludes' out \
  || { echo "    an ignored file tracked under .ai/repo was not refused (exit $rc)"; cat out; exit 1; }
echo "    ok: state git ignores, tracked under the tracked half, is refused, naming ai.repo"
check --format json > report.json 2>/dev/null || true
jq -e '.measured == true and .passes == false and (.findings | length) == 1' report.json >/dev/null \
  || { echo "    the JSON report does not say what the text said"; cat report.json; exit 1; }
git rm -q --cached .ai/repo/.tmp.scratch && rm .ai/repo/.tmp.scratch && git commit -qm 'scratch gone'

# --- update: a projection target outside the repository is refused before anything is written
pol=.ai/repo/policy.yaml
grep -q 'target: AGENTS.md' "$pol" || { echo "    the skeleton policy projects no AGENTS.md; the case needs a target to move"; exit 1; }
sed 's#target: AGENTS.md#target: ../outside-the-repository.md#' "$pol" > "$pol.new" && mv "$pol.new" "$pol"
rc=0; "$MJ" update > out 2>&1 || rc=$?
[ "$rc" != 0 ] && grep -q 'is not a path inside the repository' out && [ ! -e ../outside-the-repository.md ] \
  || { echo "    update wrote, or tried to write, outside the repository (exit $rc)"; cat out; exit 1; }
echo "    ok: update refuses a projection target outside the repository and writes nothing"
