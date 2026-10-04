# majordomus-covers: none
# scripts/unblock brings a branch up to date with the trunk the way every landing must since
# ADR 0106 §8b: the version line merges through its own driver and the merged tree is
# advanced against the trunk it now contains, so the branch it pushes never carries a number
# the trunk already has.
#
# The branch advanced once from 1.10.0 to 1.11.0; meanwhile the trunk took two landings and
# reached 1.12.0. A text merge of the two manifests conflicts on the version line — this case
# shows that first, with git alone — and unblock must instead merge cleanly, advance to
# 1.13.0, commit that as a release advance, and push it, leaving the branch's own checkout
# alone. A repository whose merge policy marks nothing `merge=version` (case 111's fixture)
# is not asked to advance at all.
#
# A local bare repository is the remote, the derive is stubbed through UNBLOCK_DERIVE, and the
# executable the script runs is the one under test, through a bin/majordomus-cli of the
# fixture's own.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN

S="$(mktemp -d "${TMPDIR:-/tmp}/mj876.XXXXXX")"; trap 'rm -rf "$S"' EXIT
W="$S/work"
git init -q --bare "$S/remote.git"
git init -q "$W"
cd "$W" || exit 1
git symbolic-ref HEAD refs/heads/master
git config user.email t@example.com
git config user.name t
"$MJ" init >/dev/null; "$MJ" update >/dev/null
printf '\nrelease:\n  cadence: minor\n' >> .ai/repo/policy.yaml
mkdir -p scripts bin apps/majordomus-cli
cp "$ROOT/scripts/unblock" "$ROOT/scripts/merge-derived" scripts/
# the fixture's launcher stands in for a distribution: unblock clears MAJORDOMUS_SHARE (an
# inherited one names some other checkout), and this repository has no share of its own
printf '#!/bin/sh\nMAJORDOMUS_SHARE="%s" exec "%s" "$@"\n' "$ROOT/share" "$RB" > bin/majordomus-cli
chmod +x scripts/unblock scripts/merge-derived bin/majordomus-cli
git config merge.derived.name "derived"
git config merge.derived.driver "$W/scripts/merge-derived %O %A %B %P"
git remote add origin "$S/remote.git"
printf 'apps/majordomus-cli/Cargo.toml merge=version\napps/majordomus-cli/Cargo.lock merge=version\n' >> .gitattributes
printf '[package]\nname = "majordomus-cli"\nversion = "1.10.0"\n' > apps/majordomus-cli/Cargo.toml
printf '[[package]]\nname = "majordomus-cli"\nversion = "1.10.0"\n' > apps/majordomus-cli/Cargo.lock
git add -A && git commit -qm base
git push -q origin HEAD:refs/heads/master && git fetch -q origin

version_at() { git show "$1:apps/majordomus-cli/Cargo.toml" | sed -n 's/^version = "\(.*\)"$/\1/p'; }
advance_and_commit() { "$RB" release advance --base origin/master >/dev/null; git commit -qam "chore(release): advance"; }

# the branch: one piece of work, advanced from 1.10.0
git checkout -q -b feature/b
echo 'fn b() {}' > b.rs && git add b.rs && git commit -qm 'feat: b'
advance_and_commit
git push -q origin HEAD:refs/heads/feature/b

# the trunk: two landings, each advanced
git checkout -q master
for w in c d; do
  echo "fn $w() {}" > "$w.rs" && git add "$w.rs" && git commit -qm "feat: $w"
  advance_and_commit
  git push -q origin HEAD:refs/heads/master && git fetch -q origin
done
[ "$(version_at origin/master)" = 1.12.0 ] && [ "$(version_at origin/feature/b)" = 1.11.0 ] \
  || { echo "    the fixture is not 1.11.0 against 1.12.0"; exit 1; }
branch_tip="$(git rev-parse origin/feature/b)"

# ---- without the driver, git alone conflicts on the version line ------------------------
git checkout -q -B probe origin/feature/b
if git -c merge.version.driver=false merge -q --no-edit origin/master >/dev/null 2>&1; then
  echo "    a text merge of 1.11.0 against 1.12.0 did not conflict; the case would prove nothing"; exit 1
fi
git merge --abort; git checkout -q master; git branch -q -D probe

# ---- unblock merges through the driver and advances past the trunk ---------------------
expect_exit 0 env UNBLOCK_DERIVE=true UNBLOCK_MERGEABLE_WAIT=0 scripts/unblock feature/b
expect_grep 'release advance against origin/master'
git fetch -q origin
[ "$(version_at origin/feature/b)" = 1.13.0 ] \
  || { echo "    unblock pushed $(version_at origin/feature/b), not 1.13.0"; exit 1; }
git merge-base --is-ancestor origin/master origin/feature/b \
  || { echo "    the pushed branch does not contain the trunk"; exit 1; }
git merge-base --is-ancestor "$branch_tip" origin/feature/b \
  || { echo "    the push rewrote the branch instead of moving it forward"; exit 1; }
git log -1 --format=%s origin/feature/b | grep -q '^chore(release): the version advances' \
  || { echo "    the advance was not committed as a release advance:"; git log -1 --format=%s origin/feature/b; exit 1; }
grep -q 'version = "1.13.0"' <(git show origin/feature/b:apps/majordomus-cli/Cargo.lock) \
  || { echo "    the lock was not advanced with the manifest"; exit 1; }
[ "$(git rev-parse --abbrev-ref HEAD)" = master ] && [ -z "$(git status --porcelain --untracked-files=no)" ] \
  || { echo "    unblock touched the checkout it was run from"; git status --short; exit 1; }

echo "    1.11.0 against 1.12.0: git alone conflicts; unblock merges, advances to 1.13.0, pushes"
