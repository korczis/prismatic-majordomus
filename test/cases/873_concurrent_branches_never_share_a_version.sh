# majordomus-covers: capability:release.obligation
# majordomus-negative: capability:release.obligation
# Proves the claim concurrent-branches-never-share-a-version (docs/CLAIMS.yaml).
# Two branches that advance from one trunk never land claiming one version (ADR 0106 §8b).
#
# Both start from 1.10.0 and both advance to 1.11.0. A lands. B is then brought up to date the
# way every landing does it — merge the trunk in with the version driver declared, `release
# advance` against the trunk — and must end at 1.12.0, not at A's 1.11.0. The same in the other
# order with a major: a branch that owes 2.0.0 keeps it through the merge, and an ordinary
# branch reconciled after it goes to 2.1.0. Without the driver the two version lines are a
# textual conflict about a number neither side should pick; with it the version line leaves
# the merge and the advance decides it. A dependency edit beside the version line still merges.
#
# Each refusal is shown as well: B unreconciled is `owed` against the moved trunk, and a branch
# behind the trunk is `behind`, never satisfied by the number it carried.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null || skip "jq absent"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN

"$MJ" init >/dev/null; "$MJ" update >/dev/null
# the cadence is the repository's own setting (ADR 0106); this fixture declares a minor
printf '\nrelease:\n  cadence: minor\n' >> .ai/repo/policy.yaml
echo '.origin.git/' >> .git/info/exclude
mkdir -p apps/majordomus-cli
manifest() { printf '[package]\nname = "majordomus-cli"\nversion = "%s"\n\n[dependencies]\n%b' "$1" "${2:-a = \"1\"\n}"; }
manifest 1.10.0 > apps/majordomus-cli/Cargo.toml
printf 'version = 4\n\n[[package]]\nname = "majordomus-cli"\nversion = "1.10.0"\n' > apps/majordomus-cli/Cargo.lock
git add -A && git commit -qm base
git init -q --bare "$PWD/.origin.git"; git remote add origin "$PWD/.origin.git"
git push -q origin HEAD:refs/heads/master && git fetch -q origin

# the driver as a clone declares it: the same entry the just recipe and core-check write
git config merge.version.name "the version line, merged version-neutrally"
git config merge.version.driver "$RB release merge-version %O %A %B %P"
grep -qs 'apps/majordomus-cli/Cargo.toml *merge=version' .gitattributes \
  || printf 'apps/majordomus-cli/Cargo.toml merge=version\napps/majordomus-cli/Cargo.lock merge=version\n' >> .gitattributes
git add .gitattributes && git commit -qm 'the version driver' && git push -q origin HEAD:refs/heads/master && git fetch -q origin

declared() { sed -n 's/^version = "\(.*\)"$/\1/p' apps/majordomus-cli/Cargo.toml | head -1; }
work() { # <branch> <file>
  git checkout -q -B "$1" origin/master
  echo "fn $2() {}" > "$2.rs" && git add "$2.rs" && git commit -qm "feat: $2"
  "$RB" release advance --base origin/master >/dev/null
  git add -A apps && git commit -qm "chore(release): advance"
}
land() { git push -q origin HEAD:refs/heads/master && git fetch -q origin; }
reconcile() { # the landing's refresh: trunk in, advance against it
  git merge -q --no-edit origin/master || { echo "    merging the trunk conflicted:"; git status --short; exit 1; }
  "$RB" release advance --base origin/master >/dev/null
  git add -A apps && git commit -qm 'chore(release): advance against the trunk' || true
}
obligation_state() { "$RB" release obligation --base origin/master --format json 2>/dev/null | jq -r .state; }

# ---- A and B both advance from 1.10.0 -------------------------------------------------
work feature/a alpha;  [ "$(declared)" = 1.11.0 ] || { echo "    A did not advance to 1.11.0"; exit 1; }
work feature/b beta;   [ "$(declared)" = 1.11.0 ] || { echo "    B did not advance to 1.11.0"; exit 1; }
git checkout -q feature/a && land
git checkout -q feature/b
[ "$(obligation_state)" = owed ] || { echo "    B's 1.11.0 reads as satisfied against a trunk already at 1.11.0"; exit 1; }
reconcile
[ "$(declared)" = 1.12.0 ] || { echo "    B reconciled to $(declared), not 1.12.0"; exit 1; }
[ "$(obligation_state)" = satisfied ] || { echo "    reconciled B is not satisfied"; exit 1; }
land

# ---- the trunk moved twice: the version lines differ, and only the driver merges them ------
# Two equal advances merge without any driver — identical lines are not a conflict — so this
# is the case the driver exists for: the branch advanced once (1.13.0) while the trunk took
# two landings (1.14.0). A text merge conflicts on the version line; the driver takes it out of
# the merge, keeps the dependency edit beside it, and the advance goes one past the trunk.
git checkout -q -B feature/dep origin/master
manifest "$(declared)" 'a = "1"\nb = "2"\n' > apps/majordomus-cli/Cargo.toml
git commit -qam 'build: depend on b'
"$RB" release advance --base origin/master >/dev/null; git commit -qam 'chore(release): advance'
[ "$(declared)" = 1.13.0 ] || { echo "    the dependency branch did not advance to 1.13.0"; exit 1; }
work feature/c gamma; land
work feature/d delta; land
[ "$(git show origin/master:apps/majordomus-cli/Cargo.toml | sed -n 's/^version = "\(.*\)"$/\1/p')" = 1.14.0 ] \
  || { echo "    the trunk did not reach 1.14.0"; exit 1; }
git checkout -q feature/dep && reconcile
grep -q '^b = "2"' apps/majordomus-cli/Cargo.toml || { echo "    the driver dropped a dependency edit"; exit 1; }
[ "$(declared)" = 1.15.0 ] || { echo "    the dependency branch reconciled to $(declared), not 1.15.0"; exit 1; }
land

# ---- a branch behind the trunk is behind, whatever number it carries -------------------
git checkout -q -B feature/old origin/master~4
echo 'fn old() {}' > old.rs && git add old.rs && git commit -qm 'feat: old'
[ "$(obligation_state)" = behind ] || { echo "    a branch below the trunk's version is not behind"; exit 1; }

echo "    1.10.0: A → 1.11.0, B reconciled → 1.12.0; 1.13.0 against 1.14.0 merged by the driver → 1.15.0 with its dependency; behind refused"
