# majordomus-covers: finish
# majordomus-negative: finish
# Completed work does not leave the trunk on its version (ADR 0106).
#
# The policy declares a minor cadence, so a change set that carries work owes, over the
# version the trunk declares, at least a minor — and at least what the public contract
# requires, which this fixture cannot measure (it has published nothing), so the cadence is
# what binds here; the precedence of a contract's major over the cadence is decided by
# release::obligation::decide and proved by its unit tests. This case walks the lifecycle
# around the predicate, in a disposable repository whose trunk it moves by hand:
#
#   unselected          the policy does not name the requirement      nothing advanced
#   partial             unfinished work owes no version               nothing advanced
#   completed           the obligation is satisfied first             1.4.0 -> 1.5.0, once
#   retry               finish again, advance again                   still 1.5.0, one ledger line
#   override            --level patch under the minimum               REFUSED, nothing written
#   override above      --exact 1.9.0                                 accepted (then put back)
#   concurrent          B advanced from the same trunk A landed on    owed again, 1.6.0
#   behind              a branch below the trunk's version            REFUSED, nothing written
#   projections only    a derived path, a release record              owed nothing
#   no trunk            the trunk does not resolve                    unverified, exit 12
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_BIN="$BIN" MAJORDOMUS_SHARE="$ROOT/share"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj801.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' > "$S/note.md"

declared() { sed -n 's/^version = "\(.*\)"$/\1/p' apps/majordomus-cli/Cargo.toml | head -n 1; }
advances() { grep -c '"event":"release.advanced"' .ai/local/state/ledger.jsonl 2>/dev/null || true; }
obligation() { "$BIN" release obligation --repo . "$@"; }

mkdir -p apps/majordomus-cli lib docs/generated .ai/repo/releases
printf '[package]\nname = "majordomus-cli"\nversion = "1.4.0"\n' > apps/majordomus-cli/Cargo.toml
printf '[[package]]\nname = "majordomus-cli"\nversion = "1.4.0"\n' > apps/majordomus-cli/Cargo.lock
printf 'docs/generated/** merge=derived\n' > .gitattributes
echo '{}' > docs/generated/x.json
echo 'a() { :; }' > lib/a.sh
git add -A >/dev/null && git commit -qm base
BASE="$(git rev-parse HEAD)"
git update-ref refs/remotes/origin/master "$BASE"

# --------------------------------------------------- the policy is what turns it on
expect_exit 0 "$MJ" start "unselected" --scope lib/
echo 'b() { :; }' >> lib/a.sh
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -d .ai" --note "$S/note.md"
[ "$(declared)" = 1.4.0 ] || { echo "    an unselected requirement advanced the version to $(declared)"; exit 1; }
git checkout -q -- lib/a.sh

sed -i.bak 's/^    - note_present$/    - note_present\n    - version_advanced/' .ai/repo/policy.yaml
printf '\nrelease:\n  cadence: minor\n' >> .ai/repo/policy.yaml
rm -f .ai/repo/policy.yaml.bak
git add -A >/dev/null && git commit -qm policy
git update-ref refs/remotes/origin/master HEAD

# --------------------------------------------------- the verdict, before anything is done
git checkout -qb feature/a
echo 'c() { :; }' >> lib/a.sh
expect_exit 10 obligation
expect_grep 'state +owed'
expect_grep 'minimum +1\.5\.0'
expect_grep 'carries +work \(1 authored'
expect_grep 'cadence is minor'
LAST_OUT="$(obligation --format json || true)"
[ "$(jq -r .id <<<"$LAST_OUT")" = "feature/a@1.4.0" ] || { echo "    the obligation's identity is not the branch at the trunk's version: $LAST_OUT"; exit 1; }

# --------------------------------------------------- unfinished work owes nothing
expect_exit 0 "$MJ" start "partial" --scope lib/,apps/,docs/,share/,site/
expect_exit 0 "$MJ" finish --outcome partial --verify-command "test -d .ai" --note "$S/note.md"
[ "$(declared)" = 1.4.0 ] || { echo "    a partial outcome advanced the version to $(declared)"; exit 1; }
[ "$(advances)" = 0 ] || { echo "    a partial outcome recorded an advance"; exit 1; }

# --------------------------------------------------- an override may not undershoot
expect_exit 10 "$BIN" release bump --repo . --level patch
expect_grep 'below 1\.5\.0, the minimum the version obligation'
[ "$(declared)" = 1.4.0 ] || { echo "    a refused bump wrote $(declared)"; exit 1; }

# --------------------------------------------------- completed: advanced once, then judged
expect_exit 0 "$MJ" start "completed" --scope lib/,apps/,docs/,share/,site/
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -d .ai" --note "$S/note.md"
expect_grep 'version 1\.4\.0 -> 1\.5\.0'
[ "$(declared)" = 1.5.0 ] || { echo "    completed work left the version at $(declared)"; exit 1; }
grep -q 'version = "1.5.0"' apps/majordomus-cli/Cargo.lock || { echo "    the lock's record was not advanced with the manifest"; exit 1; }
[ "$(advances)" = 1 ] || { echo "    expected one release.advanced line, found $(advances)"; exit 1; }
grep '"event":"release.advanced"' .ai/local/state/ledger.jsonl | grep -q '"obligation":"feature/a@1.4.0"' \
  || { echo "    the advance does not carry the obligation's identity"; exit 1; }
expect_exit 0 obligation
expect_grep 'state +satisfied'

# --------------------------------------------------- a retry advances nothing
expect_exit 15 "$MJ" finish --outcome completed --verify-command "test -d .ai" --note "$S/note.md"
expect_exit 0 "$BIN" release advance --repo .
expect_grep 'holds \(satisfied\).*nothing written'
expect_exit 0 "$BIN" release advance --repo .
[ "$(declared)" = 1.5.0 ] || { echo "    a retried advance moved the version to $(declared)"; exit 1; }
[ "$(advances)" = 1 ] || { echo "    a retry recorded another advance ($(advances))"; exit 1; }

# --------------------------------------------------- an override above the minimum is allowed
expect_exit 0 "$BIN" release bump --repo . --exact 1.9.0
[ "$(declared)" = 1.9.0 ] || { echo "    --exact 1.9.0 above the minimum was not written"; exit 1; }
git checkout -q -- apps/majordomus-cli/Cargo.toml apps/majordomus-cli/Cargo.lock 2>/dev/null || true
"$BIN" release advance --repo . >/dev/null
[ "$(declared)" = 1.5.0 ] || { echo "    the advance is not reproducible after a reset: $(declared)"; exit 1; }
git add -A >/dev/null && git commit -qm "feat: a"
A="$(git rev-parse HEAD)"

# --------------------------------------------------- two branches, one trunk
# B starts from the same trunk and advances from it, as it would have before A landed.
git checkout -q -b feature/b "$(git rev-parse refs/remotes/origin/master)"
echo 'd() { :; }' > lib/b.sh
expect_exit 0 "$BIN" release advance --repo .
[ "$(declared)" = 1.5.0 ] || { echo "    B did not advance from the trunk it started on"; exit 1; }
git add -A >/dev/null && git commit -qm "feat: b"
# A lands. B merges the trunk: both advanced to 1.5.0, so the merge is clean — and wrong.
git update-ref refs/remotes/origin/master "$A"
git merge -q --no-edit refs/remotes/origin/master
expect_exit 10 obligation
expect_grep 'trunk +origin/master [0-9a-f]+ declares 1\.5\.0'
expect_grep 'minimum +1\.6\.0'
expect_exit 0 "$BIN" release advance --repo .
[ "$(declared)" = 1.6.0 ] || { echo "    B reused A's version: $(declared)"; exit 1; }
git add -A >/dev/null && git commit -qm "chore(release): b advances over a"
# the merge gate's own question, against the first parent the merge lands on
git checkout -q -b trunk-after-b "$A"
git merge -q --no-ff --no-edit feature/b
expect_exit 0 obligation --base HEAD^1
expect_grep 'state +satisfied'

# --------------------------------------------------- a tree behind the trunk is refused
git update-ref refs/remotes/origin/master HEAD
git checkout -q -b feature/c "$A"
echo 'e() { :; }' > lib/c.sh
expect_exit 10 obligation
expect_grep 'state +behind'
expect_exit 10 "$BIN" release advance --repo .
expect_grep 'REFUSED'
[ "$(declared)" = 1.5.0 ] || { echo "    a tree behind its trunk was advanced to $(declared)"; exit 1; }
git checkout -q -- . && git clean -qfd lib

# --------------------------------------------------- projections and evidence owe nothing
git checkout -q -b chore/derive "$(git rev-parse refs/remotes/origin/master)"
echo '{"x":1}' > docs/generated/x.json
expect_exit 0 obligation
expect_grep 'carries +generated-sync'
expect_grep 'state +not-owed'
printf 'version: 1.6.0\n' > .ai/repo/releases/v1.6.0.yaml
expect_exit 0 obligation
expect_grep 'carries +release-evidence'
git checkout -q -- . && git clean -qfd .ai/repo/releases

# --------------------------------------------------- no trunk is never a pass
expect_exit 12 obligation --base refs/remotes/origin/nowhere
expect_grep 'state +unverified'
