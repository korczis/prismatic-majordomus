# majordomus-covers: none
# The two commands that keep this repository's self-accumulating quantities bounded, and the
# gate that keeps them honest (rule project.accumulation-is-measured).
#
# Five things here grow without anyone deciding they should — open pull requests, linked
# worktrees, local branches, running servers and the disk their build output holds — and the
# day the first four were counted there were sixteen, sixty-four, a hundred and eleven and
# fourteen of them. The commands that reduce them are the dangerous kind: one ends processes,
# one deletes directories, one merges and pushes. So what this case proves is not that they
# run, but that they refuse. The fifth quantity's own behaviour is case 276.
#
# The reaper must never end a server a live session is attached to — sessions here are each
# other's instruments, and a name match would take them all out. The gate must actually fail
# when its subject is broken, because a gate that passes over a missing driver, a missing
# case or a document advertising a merge button that cannot work is how the backlog above was
# allowed to accumulate in silence.
. "$ROOT/test/lib.sh"

REAPER="$ROOT/scripts/reap-orphans"
GATE="$ROOT/scripts/ci/backlog-check"

# ---------------------------------------------------------------- the commands exist
expect_file "$REAPER"
expect_file "$GATE"
[ -x "$REAPER" ] || { echo "    scripts/reap-orphans is not executable"; exit 1; }
[ -x "$GATE" ]   || { echo "    scripts/ci/backlog-check is not executable"; exit 1; }
expect_file "$ROOT/.ai/repo/rules/project/accumulation-is-measured.v2.md"

# ---------------------------------------------------------------- the reaper spares the living
# A process whose parent is alive is not garbage however old it looks. The reaper's whole
# safety rests on that predicate, so the case starts one and insists it survives a dry run.
#
# `--servers`: the same reaper has a second subject since ADR 0055, and this case is about
# the first. What it does to build directories is test/cases/276_disk_is_bounded.sh, which
# builds a fixture repository for it rather than classifying this machine's real worktrees.
sleep 600 &
LIVE=$!
trap 'kill "$LIVE" 2>/dev/null || true' EXIT
out="$("$REAPER" --servers --min-age 0s 2>&1 || true)"
if grep -qE "^$LIVE " <<<"$out"; then
  echo "    the reaper listed pid $LIVE, whose parent is this test: it reaps by age, not by ownership"; exit 1
fi

# Dry run is the default and must change nothing: the process it was pointed at is still here.
"$REAPER" --servers --min-age 0s >/dev/null 2>&1 || true
ps -p "$LIVE" >/dev/null 2>&1 || { echo "    the default run ended a process; the reaper must be dry unless --kill"; exit 1; }

# It matches by predicate. A name match is the one implementation that is always wrong here.
grep -nE '^[^#]*\b(pkill|killall)\b' "$REAPER" >/dev/null 2>&1 && {
  echo "    scripts/reap-orphans matches processes by name; it would end the servers live peers use"; exit 1; }

# ---------------------------------------------------------------- the gate passes on the tree it guards
expect_exit 0 "$GATE"

# ---------------------------------------------------------------- and fails when its subject is broken
# A copy, so the findings are provoked without touching the repository. Only the paths the
# gate reads are needed; MJ_ROOT points it at them.
W="$(mktemp -d)"
trap 'kill "$LIVE" 2>/dev/null || true; rm -rf "$W"' EXIT
mkdir -p "$W/scripts/ci" "$W/test/cases" "$W/.ai/repo/rules/project" "$W/docs" "$W/.just" "$W/lib" "$W/bin"
cp "$ROOT/.gitattributes" "$W/.gitattributes"
cp "$ROOT/scripts/merge-derived" "$W/scripts/merge-derived"
cp "$REAPER" "$W/scripts/reap-orphans"
cp "$GATE" "$W/scripts/ci/backlog-check"
cp "$ROOT/.ai/repo/rules/project/accumulation-is-measured.v2.md" "$W/.ai/repo/rules/project/"
cp "$ROOT/test/cases/131_backlog_hygiene.sh" "$W/test/cases/"
cp "$ROOT/test/cases/276_disk_is_bounded.sh" "$W/test/cases/"
# the fifth quantity's half: the floor lives in the library, and the paths that start a
# build ask it. The gate reads all four, so the fixture carries all four.
cp "$ROOT/lib/rust_bin.sh" "$W/lib/rust_bin.sh"
cp "$ROOT/bin/majordomus-cli" "$W/bin/majordomus-cli"
cp "$ROOT/scripts/derive" "$W/scripts/derive"
cp "$ROOT/.just/build.just" "$W/.just/build.just"
cp "$ROOT/.just/site.just" "$W/.just/site.just" 2>/dev/null || true
# the eighth clause's half: the recorded ratchet of the landable backlog
cp "$ROOT/.ai/repo/backlog-baseline.txt" "$W/.ai/repo/backlog-baseline.txt"
touch "$W/justfile"
chmod +x "$W/scripts/merge-derived" "$W/scripts/reap-orphans" "$W/scripts/ci/backlog-check"

MJ_ROOT="$W" expect_exit 0 "$W/scripts/ci/backlog-check"

# the declaration without its driver — the one-way relation this gate exists to close
mv "$W/scripts/merge-derived" "$W/scripts/merge-derived.away"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep 'declares merge=derived but scripts/merge-derived is missing'
mv "$W/scripts/merge-derived.away" "$W/scripts/merge-derived"

# the fifth quantity, both halves. A reaper that has stopped holding one of its three
# conditions, and a build path that starts a build without asking whether there is room.
sed 's/status --porcelain/status/' "$REAPER" > "$W/scripts/reap-orphans"
chmod +x "$W/scripts/reap-orphans"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep 'no longer tests .status --porcelain.'
cp "$REAPER" "$W/scripts/reap-orphans"; chmod +x "$W/scripts/reap-orphans"

sed 's/mj_rust_space_check/mj_nothing_at_all/' "$ROOT/.just/build.just" > "$W/.just/build.just"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep 'start a build without asking whether there is room'
cp "$ROOT/.just/build.just" "$W/.just/build.just"

mv "$W/lib/rust_bin.sh" "$W/lib/rust_bin.sh.away"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep 'the free-space floor has no home'
mv "$W/lib/rust_bin.sh.away" "$W/lib/rust_bin.sh"

# a document that sends a reader to the merge button, which cannot work in this repository
printf 'To land this, click the Merge pull request button on GitHub.\n' > "$W/docs/HOWTO.md"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep 'instructs a reader to merge on the forge'
rm "$W/docs/HOWTO.md"

# a case that backgrounds a server without bounding its life — the shape that actually
# leaked here: `serve`'s --idle defaults to 0, so an interrupted run abandons the server
mkdir -p "$W/test/cases"
printf '#!/bin/sh\n"$BIN" serve --port 0 > serve.log 2>&1 &\n' > "$W/test/cases/99_probe.sh"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep 'backgrounds a server without --idle'
printf '#!/bin/sh\n"$BIN" serve --port 0 --idle 60 > serve.log 2>&1 &\n' > "$W/test/cases/99_probe.sh"
MJ_ROOT="$W" expect_exit 0 "$W/scripts/ci/backlog-check"
rm "$W/test/cases/99_probe.sh"

# ---------------------------------------------------------------- the quantity itself
# Clause 8 of the rule — growth is loud before it is a cliff — is the only one that is a
# number, and it was the only one nothing ever measured: the gate could, under --remote, and
# the CI model wired it without the flag. So the number is proved here, both ways, against a
# fixture forge: a fake `gh` that lists pull requests, and a real local remote carrying their
# heads under refs/pull/<n>/head, which is what the gate fetches and merge-trees.
#
# The bound is the ratchet, not the threshold: 59 pull requests were already landable the day
# this was wired, and a gate that refused them would have reddened the trunk for every
# session here over a backlog none of them created. The ratchet may fall and may never rise,
# which is the third thing proved below.
RATCHET="$W/.ai/repo/backlog-baseline.txt"
ratchet_of() { printf '# fixture ratchet\n'; for n in "$@"; do printf '2026-09-1%s %s debt: %s landable pull requests.\n' "$n" "$n" "$n"; done; }

# a ratchet raised above what is already recorded is refused, whatever the forge says
printf '# fixture\n2026-09-01 4 debt: four landable pull requests.\n2026-09-02 6 debt: six landable pull requests.\n' > "$RATCHET"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep 'ratchet in .* was raised from 4 to 6'

# and a ratchet that falls is the whole point
printf '# fixture\n2026-09-01 6 debt: six landable pull requests.\n2026-09-02 4 debt: four landable pull requests.\n' > "$RATCHET"
MJ_ROOT="$W" expect_exit 0 "$W/scripts/ci/backlog-check"
expect_grep 'ratchet is 4 over 2 recorded measurement'

# a fixture forge: two open pull requests, both merging cleanly into the fixture's master
FORGE="$W/forge.git"
git init --quiet --bare "$FORGE"
G="$W/repo"
mkdir -p "$G"
git -C "$G" init --quiet -b master
git -C "$G" config user.email t@example.com
git -C "$G" config user.name test
printf 'one\n' > "$G/a.txt"
git -C "$G" add a.txt
git -C "$G" commit --quiet -m base
git -C "$G" remote add origin "$FORGE"
git -C "$G" push --quiet origin master
for n in 1 2; do
  git -C "$G" checkout --quiet -b "pr$n" master
  printf 'pr%s\n' "$n" > "$G/pr$n.txt"
  git -C "$G" add "pr$n.txt"
  git -C "$G" commit --quiet -m "pr$n"
  git -C "$G" push --quiet origin "HEAD:refs/pull/$n/head"
  git -C "$G" checkout --quiet master
done
git -C "$G" fetch --quiet origin master:refs/remotes/origin/master
# the gate reads the fixture repository, so its structural subjects move in beside it
cp -R "$W/scripts" "$W/lib" "$W/bin" "$W/.just" "$W/test" "$W/.ai" "$G/"
cp "$W/.gitattributes" "$W/justfile" "$G/"
mkdir -p "$G/docs"
FAKEBIN="$W/fakebin"
mkdir -p "$FAKEBIN"
cat > "$FAKEBIN/gh" <<'SH'
#!/bin/sh
case "$1 $2" in
  "auth status") exit 0 ;;
  "pr list") printf '1\n2\n' ;;
  *) exit 1 ;;
esac
SH
chmod +x "$FAKEBIN/gh"

# at the ratchet: two landable, ratchet two — loud, and not a failure
ratchet_of 3 2 > "$G/.ai/repo/backlog-baseline.txt"
expect_exit 0 env MJ_ROOT="$G" MJ_BACKLOG_MAX=0 PATH="$FAKEBIN:$PATH" "$G/scripts/ci/backlog-check" --remote
expect_grep '2 of 2 open pull requests are landable'

# above it: the same two against a ratchet of one, which must name both numbers and fail
ratchet_of 3 1 > "$G/.ai/repo/backlog-baseline.txt"
expect_exit 10 env MJ_ROOT="$G" MJ_BACKLOG_MAX=0 PATH="$FAKEBIN:$PATH" "$G/scripts/ci/backlog-check" --remote
expect_grep '2 of 2 open pull requests merge cleanly here and are still open, above the ratchet of 1'

# and a forge it cannot reach is a finding, never a quiet pass
NOAUTH="$W/noauthbin"
mkdir -p "$NOAUTH"
printf '#!/bin/sh\nexit 1\n' > "$NOAUTH/gh"
chmod +x "$NOAUTH/gh"
expect_exit 10 env MJ_ROOT="$G" MJ_BACKLOG_MAX=0 PATH="$NOAUTH:$PATH" "$G/scripts/ci/backlog-check" --remote
expect_grep 'gh has no token: the backlog cannot be measured'

# the behavioural case itself going missing
rm "$W/test/cases/131_backlog_hygiene.sh"
MJ_ROOT="$W" expect_exit 10 "$W/scripts/ci/backlog-check"
expect_grep '131_backlog_hygiene.sh is missing'
