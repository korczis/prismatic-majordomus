# majordomus-covers: none
# Work held where only one disk can see it is a finding, not a footnote.
#
# `project.a-worker-that-stops-leaves-its-work-behind` was written as advisory, and said in
# its own Failure behaviour why: "nothing can see the uncommitted half except somebody
# looking, which is exactly why the responsibility is written down rather than gated."
#
# That premise was true when it was written and is not true now. The worktree topology
# already counts the uncommitted work of every registered work tree, and one `git rev-list
# --branches --not --remotes` names every commit that has never left this disk. On
# 2026-09-16 this repository held nine branch tips whose commits reached no remote — the
# oldest four days old — and sixty-four work trees with uncommitted files, and nothing in
# the tool said so.
#
# Asserted here, against a fixture repository built for it: a branch that reaches no remote
# and a work tree with uncommitted files are each reported as at risk, with the command
# that would move them out of danger; pushing and committing makes the same repository
# converge; and the completion invariant's `no-stale-topology` question reads this verdict
# rather than answering "nothing in this report reaches that fact".
. "$ROOT/test/lib.sh"

RB="$(rust_bin)" || rust_bin_exit $?

WORK="$(mktemp -d "${TMPDIR:-/tmp}/mj-convergence.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

REMOTE="$WORK/remote.git"
REPO="$WORK/repo"

git init -q --bare "$REMOTE"
git init -q -b master "$REPO"
git -C "$REPO" config user.email case@example.com
git -C "$REPO" config user.name "case 383"
git -C "$REPO" remote add origin "$REMOTE"
echo base > "$REPO/README.md"
git -C "$REPO" add -A
git -C "$REPO" commit -qm "base"
git -C "$REPO" push -q origin master
git -C "$REPO" fetch -q origin

# The tool answers about the repository it is run in — it takes the git toplevel of the
# working directory and has no --repo of its own — so every call below runs inside the
# fixture. The layer is what makes the repository indexable, and the shell tool creates it.
"$ROOT/bin/majordomus" --repo "$REPO" init >/dev/null 2>&1 || true
git -C "$REPO" add -A >/dev/null 2>&1
git -C "$REPO" commit -qm "the .ai layer" >/dev/null 2>&1 || true
git -C "$REPO" push -q origin master

# The convergence of the fixture, never of the repository this case runs in. The command
# exits 10 when the repository does not converge — a verdict, like every other contract of
# this executable — so the status is captured rather than left to abort the case under
# `set -e`.
# Sets OUT and STATUS in this shell: a command substitution would run the assignment in a
# subshell and the status would be lost, which is how a verdict silently becomes a pass.
STATUS=0
OUT=""
convergence() { STATUS=0; OUT="$(cd "$REPO" && "$RB" convergence --share "$ROOT/share" --format json 2>&1)" || STATUS=$?; }

# --- 1. a converged repository says so
convergence
[ "$STATUS" = 0 ] || { echo "    a clean fixture did not exit 0 (got $STATUS):"
  echo "$OUT" | sed 's/^/      /'; exit 1; }
printf '%s' "$OUT" | grep -q '"converged": *true' || {
  echo "    a repository whose every branch is on the remote and whose tree is clean is not"
  echo "    reported as converged:"; echo "$OUT" | sed 's/^/      /'; exit 1; }
echo "    a clean fixture converges"

# --- 2. a branch that reaches no remote is at risk, and named
git -C "$REPO" checkout -q -b feature/never-pushed
echo unique > "$REPO/unique.txt"
git -C "$REPO" add -A
git -C "$REPO" commit -qm "work that exists on one disk"
git -C "$REPO" checkout -q master

convergence
printf '%s' "$OUT" | grep -q '"converged": *false' || {
  echo "    a branch whose commits reach no remote did not make the repository unconverged:"
  echo "$OUT" | sed 's/^/      /'; exit 1; }
printf '%s' "$OUT" | grep -q 'feature/never-pushed' || {
  echo "    the unconverged verdict does not name the branch holding the work"; exit 1; }
printf '%s' "$OUT" | grep -q '"local_only"' || {
  echo "    the branch is not given the local_only disposition"; exit 1; }
printf '%s' "$OUT" | grep -q 'git push -u origin feature/never-pushed' || {
  echo "    the finding carries no remedy; a finding a person cannot act on is a footnote"
  exit 1; }
[ "$STATUS" = 10 ] || {
  echo "    the command exited $STATUS, not 10: a verdict nothing can refuse on is a report"
  exit 1; }
echo "    a branch on no remote is local_only, named, and carries its remedy"

# --- 3. pushing it is what converges it
git -C "$REPO" push -q origin feature/never-pushed
git -C "$REPO" fetch -q origin
convergence
printf '%s' "$OUT" | grep -q '"converged": *true' || {
  echo "    pushing the branch did not converge the repository:"
  echo "$OUT" | sed 's/^/      /'; exit 1; }
echo "    pushing it converges the repository"

# --- 4. uncommitted files are the half nothing could see before
echo "half written" > "$REPO/draft.txt"
convergence
printf '%s' "$OUT" | grep -q '"converged": *false' || {
  echo "    an untracked file in the work tree left the repository converged — this is the"
  echo "    exact half the rule said nothing could see:"; echo "$OUT" | sed 's/^/      /'; exit 1; }
printf '%s' "$OUT" | grep -q '"uncommitted"' || {
  echo "    the work tree is not given the uncommitted disposition"; exit 1; }
rm -f "$REPO/draft.txt"
echo "    uncommitted files are reported, with the work tree that holds them"

# --- 5. the completion invariant reads this verdict, not a sentence about a command
# The question exists either way; what is asserted is where its answer comes from. Before
# this, `no-stale-topology` answered "nothing in this report reaches that fact".
grep -q 'Answers::Convergence' "$ROOT/apps/majordomus-cli/src/gates/done.rs" || {
  echo "    the done invariant does not answer no-stale-topology from the convergence verdict"
  exit 1; }
expect_no_grep 'majordomus worktree && majordomus doctor' \
  "$ROOT/apps/majordomus-cli/src/gates/done.rs" \
  "no-stale-topology still defers to a command instead of reading a verdict"
echo "    the completion invariant answers no-stale-topology from the verdict"
