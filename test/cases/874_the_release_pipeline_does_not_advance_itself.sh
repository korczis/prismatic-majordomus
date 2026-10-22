# majordomus-covers: capability:release.obligation
# majordomus-negative: capability:release.obligation
# One piece of work causes one advance, however many follow-up commits the pipeline makes
# after it (ADR 0106: generated release evidence never creates a new obligation).
#
# The sequence a landing produces is replayed on a real trunk, and after every merge the
# automation's own step — `release advance` against the trunk — is run, exactly as a landing
# or a finish would run it:
#
#   work merged with its advance         1.4.0 -> 1.5.0          the one advance
#   a projection refresh merged          owes nothing            advance writes nothing
#   the release record of 1.5.0 merged   owes nothing            advance writes nothing
#   the record and a refresh, together   owes nothing            advance writes nothing
#   the next piece of work               owes 1.6.0              the next advance
#
# A pipeline that classified its own evidence as work would climb 1.5.0, 1.6.0, 1.7.0 ... with
# no work behind any of it; the version is read after every step, so one extra advance anywhere
# fails the case at the step that made it.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null || skip "jq absent"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN

"$MJ" init >/dev/null; "$MJ" update >/dev/null
printf '\nrelease:\n  cadence: minor\n' >> .ai/repo/policy.yaml
echo '.origin.git/' >> .git/info/exclude
mkdir -p apps/majordomus-cli docs/generated .ai/repo/releases
printf '[package]\nname = "majordomus-cli"\nversion = "1.4.0"\n' > apps/majordomus-cli/Cargo.toml
printf '[[package]]\nname = "majordomus-cli"\nversion = "1.4.0"\n' > apps/majordomus-cli/Cargo.lock
printf 'docs/generated/** merge=derived\n' >> .gitattributes
echo '{"n":0}' > docs/generated/x.json
git add -A && git commit -qm base
git init -q --bare "$PWD/.origin.git"; git remote add origin "$PWD/.origin.git"
git push -q origin HEAD:refs/heads/master && git fetch -q origin

declared() { sed -n 's/^version = "\(.*\)"$/\1/p' apps/majordomus-cli/Cargo.toml | head -1; }
trunk() { git show origin/master:apps/majordomus-cli/Cargo.toml | sed -n 's/^version = "\(.*\)"$/\1/p'; }
# a change set is made on a branch, advanced as automation would, and merged into the trunk
land() { # <branch> <message>
  "$RB" release advance --base origin/master >/dev/null
  git add -A && { git diff --cached --quiet || git commit -qm "$2"; }
  git checkout -q master 2>/dev/null || git checkout -q -b master origin/master
  git reset -q --hard origin/master
  git merge -q --no-ff --no-edit "$1"
  git push -q origin HEAD:refs/heads/master && git fetch -q origin
}
step() { # <expected trunk version> <what>
  [ "$(trunk)" = "$1" ] || { echo "    after $2 the trunk is at $(trunk), expected $1"; exit 1; }
}

git checkout -q -b feature/work origin/master
echo 'fn work() {}' > work.rs && git add work.rs && git commit -qm 'feat: work'
land feature/work 'chore(release): advance'
step 1.5.0 "the work and its advance"

git checkout -q -B feature/refresh origin/master
echo '{"n":1}' > docs/generated/x.json && git commit -qam 'chore(derive): the projections follow'
land feature/refresh 'chore(release): advance'
step 1.5.0 "a projection refresh"

git checkout -q -B release/record-v1.5.0 origin/master
printf 'schema: release/v1\nversion: 1.5.0\n' > .ai/repo/releases/v1.5.0.yaml
git add -A && git commit -qm 'chore(release): record v1.5.0'
land release/record-v1.5.0 'chore(release): advance'
step 1.5.0 "the release record"

git checkout -q -B release/record-and-refresh origin/master
printf 'schema: release/v1\nversion: 1.5.0\nnote: amended\n' > .ai/repo/releases/v1.5.0.yaml
echo '{"n":2}' > docs/generated/x.json
git add -A && git commit -qm 'chore(release): the record and its projections'
land release/record-and-refresh 'chore(release): advance'
step 1.5.0 "the record with its projections"

git checkout -q -B feature/more origin/master
echo 'fn more() {}' > more.rs && git add more.rs && git commit -qm 'fix: more'
land feature/more 'chore(release): advance'
step 1.6.0 "the next piece of work"

echo "    work → 1.5.0; refresh, record, record+refresh → still 1.5.0; next work → 1.6.0"
