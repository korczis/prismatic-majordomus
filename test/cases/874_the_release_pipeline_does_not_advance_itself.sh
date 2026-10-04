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
#   a record as #748 landed it           owes nothing            gate green, advance writes nothing
#   a published record edited by hand    owes 1.6.0              it is work, and is advanced
#   the next piece of work               owes 1.7.0              the next advance
#   trunk attributes nobody can read     unverified, exit 12     never a guess
#
# A pipeline that classified its own evidence as work would climb 1.5.0, 1.6.0, 1.7.0 ... with
# no work behind any of it; the version is read after every step, so one extra advance anywhere
# fails the case at the step that made it.
#
# The #748 step is the change set master's e08aec8651 merged, shape for shape: a new record,
# the public metadata the record generator writes from it (site/static/releases/v1.5.1.json,
# and the stable pointer), the .gitattributes line marking the new metadata merge=derived with
# the generated block's path count — and the derived refresh. The trunk's attributes cannot mark
# a file the trunk does not have, so the new metadata and its attribute line are recognised as
# the record's projections from what the generator writes, never from a commit message.
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
# the trunk declares the release metadata it already has, as master's generated block does
mkdir -p site/static/releases
printf '# 2 path(s) from docs/generated/artifacts.json\ndocs/generated/** merge=derived\nsite/static/releases/latest.json merge=derived\nsite/static/releases/v1.4.0.json merge=derived\n' >> .gitattributes
echo '{"tag":"v1.4.0"}' > site/static/releases/latest.json
echo '{"tag":"v1.4.0"}' > site/static/releases/v1.4.0.json
printf 'schema: release/v1\nversion: 1.4.0\n' > .ai/repo/releases/v1.4.0.yaml
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

# #748, mirrored: the record of a new release, its generated metadata and pointer, the
# attribute line that marks the new metadata derived (and the block's count), and the refresh
git checkout -q -B release/record-v1.5.1 origin/master
printf 'schema: release/v1\nversion: 1.5.1\n' > .ai/repo/releases/v1.5.1.yaml
echo '{"tag":"v1.5.1"}' > site/static/releases/v1.5.1.json
echo '{"tag":"v1.5.1"}' > site/static/releases/latest.json
awk '{ sub(/^# 2 path\(s\)/, "# 3 path(s)"); print }
     $0 == "site/static/releases/v1.4.0.json merge=derived" { print "site/static/releases/v1.5.1.json merge=derived" }' \
  .gitattributes > .gitattributes.new && mv .gitattributes.new .gitattributes
grep -qx 'site/static/releases/v1.5.1.json merge=derived' .gitattributes || { echo "    the fixture did not add the attribute line"; exit 1; }
echo '{"n":2}' > docs/generated/x.json
git add -A && git commit -qm 'chore(release): record v1.5.1'
expect_exit 0 "$RB" release obligation --base origin/master
expect_grep 'carries +release-evidence \(0 authored'
expect_grep 'state +not-owed'
land release/record-v1.5.1 'chore(release): advance'
step 1.5.0 "a record landed as #748 landed it"
# the gate, as CI runs it on the merge: against the first parent
expect_exit 0 "$RB" release obligation --base HEAD^1
expect_grep 'state +not-owed'

# a record already published, edited by hand: that is a change to what the repository says it
# published, so it is work, owes the cadence, and is advanced like any other work
git checkout -q -B fix/amend-a-record origin/master
printf 'schema: release/v1\nversion: 1.5.1\nnote: amended by hand\n' > .ai/repo/releases/v1.5.1.yaml
git add -A && git commit -qm 'chore(release): amend the record'
expect_exit 10 "$RB" release obligation --base origin/master
expect_grep 'carries +work \(1 authored'
expect_grep 'state +owed'
expect_grep '\.ai/repo/releases/v1\.5\.1\.yaml'
land fix/amend-a-record 'chore(release): advance'
step 1.6.0 "a published record edited by hand"

git checkout -q -B feature/more origin/master
echo 'fn more() {}' > more.rs && git add more.rs && git commit -qm 'fix: more'
land feature/more 'chore(release): advance'
step 1.7.0 "the next piece of work"

# trunk attributes nobody can read classify nothing: unverified, never "no path is derived"
# (which would call the projection refresh below work and owe a minor nobody owes)
git checkout -q -B chore/refresh-again origin/master
echo '{"n":3}' > docs/generated/x.json
expect_exit 0 "$RB" release obligation --base origin/master
expect_grep 'carries +generated-sync'
mkdir -p "$PWD/.shim"; echo '.shim/' >> .git/info/exclude
REAL_GIT="$(command -v git)"
printf '#!/usr/bin/env bash\nfor a in "$@"; do [ "$a" = check-attr ] && { echo "fatal: injected check-attr failure" >&2; exit 128; }; done\nexec "%s" "$@"\n' "$REAL_GIT" > .shim/git
chmod +x .shim/git
expect_exit 12 env PATH="$PWD/.shim:$PATH" "$RB" release obligation --base origin/master
expect_grep 'state +unverified'
expect_grep 'no changed path could be classified'

echo "    work → 1.5.0; refresh, record, #748's record → still 1.5.0; edited record → 1.6.0; next work → 1.7.0; unreadable attributes → unverified"
