# majordomus-covers: doctor
# The branch-to-worktree topology, end to end, against real git and through the wiring the
# shell tool verifies: the container and every path derive from git identity alone, a
# misplaced dirty worktree migrates with its work and its fingerprint equal, the pre-commit
# hook asks the guard and refuses a feature branch committed from the wrong place, and
# doctor proves the hook is wired once the policy declares it.
#
# The crate's own suite (apps/majordomus-cli/tests/worktree.rs) covers the standings, the
# conflicts and the refusals in depth. What is here is the part that crosses the two
# programs and git's own hook mechanism.
#
# Proves claim worktree-topology-derived (docs/CLAIMS.yaml), whose `test:` names this case.
# Proves claim worktree-migration-lossless (docs/CLAIMS.yaml), whose `test:` names this case.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

# The repository is nested one level down on purpose: its container is its *sibling*, so a
# repository rooted at the case's temporary directory would put the container beside that
# directory and leave it behind. Everything this case creates stays under $T.
# Paths are resolved (`pwd -P`): the executable reports what git holds, and git holds the
# resolved path of the primary checkout, which on macOS is not the $TMPDIR spelling.
mkdir -p "$T/wt"; W="$(cd "$T/wt" && pwd -P)"; R="$W/repo"
fixture_repo "$R" >/dev/null
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
git -C "$R" add -A >/dev/null
git -C "$R" commit -qm fixture >/dev/null
[ -f "$R/.ai/manifest.yaml" ] || { echo "    the fixture repository has no layer"; exit 1; }
trunk="$(git -C "$R" symbolic-ref --short HEAD)"

# every invocation of the executable, from a directory the caller names
mj() { local cwd="$1"; shift; ( cd "$cwd" && MAJORDOMUS_SHARE="$R/share" "$RB" "$@" ); }

# ---------------------------------------------------------------- 1. the derived container and path
expect_exit 0 mj "$R" worktree root
[ "$LAST_OUT" = "$R-wt" ] || { echo "    root is '$LAST_OUT', expected '$R-wt'"; exit 1; }
expect_exit 0 mj "$R" worktree path feature/providers/streaming
[ "$LAST_OUT" = "$R-wt/feature/providers/streaming" ] || { echo "    path is '$LAST_OUT'; the hierarchy must be kept"; exit 1; }
expect_exit 0 mj "$R" worktree
expect_grep "worktree    ok  primary"

# ---------------------------------------------------------------- 2. create derives the path
expect_exit 0 mj "$R" worktree create feature/x/y
[ -d "$R-wt/feature/x/y" ] || { echo "    $R-wt/feature/x/y was not created"; exit 1; }
[ -e "$R/feature" ] && { echo "    a directory was created inside the repository"; exit 1; }
git -C "$R" worktree list --porcelain | grep -qx "worktree $R-wt/feature/x/y" \
  || { echo "    git did not register $R-wt/feature/x/y"; exit 1; }
[ "$(git -C "$R-wt/feature/x/y" symbolic-ref --short HEAD)" = feature/x/y ] || { echo "    the worktree is not on its branch"; exit 1; }

# ---------------------------------------------------------------- 3. the same answer from within
mkdir -p "$R-wt/feature/x/y/apps/foo/src"
expect_exit 0 mj "$R-wt/feature/x/y/apps/foo/src" worktree root
[ "$LAST_OUT" = "$R-wt" ] || { echo "    from inside a linked worktree the root is '$LAST_OUT'"; exit 1; }
expect_no_grep "y-wt"
expect_exit 0 mj "$R-wt/feature/x/y" worktree
expect_grep "worktree    ok  canonical"

# ---------------------------------------------------------------- 4. a misplaced dirty worktree
git -C "$R" worktree add -q -b fix/legacy "$W/outside"
echo "modified" > "$W/outside/README.md"
echo "staged" > "$W/outside/staged.txt"; git -C "$W/outside" add staged.txt
echo "untracked" > "$W/outside/untracked.txt"
head_before="$(git -C "$W/outside" rev-parse HEAD)"
status_before="$(git -C "$W/outside" status --porcelain | LC_ALL=C sort)"
expect_exit 10 mj "$R" worktree validate
expect_grep "worktree.path_mismatch"
expect_grep "$R-wt/fix/legacy"

# the guard refuses a commit from there, and passes at the primary checkout on the trunk
expect_exit 10 mj "$W/outside" worktree guard
expect_grep "REFUSED"
expect_exit 0 mj "$R" worktree guard

# ---------------------------------------------------------------- 5. the plan changes nothing
before="$(git -C "$R" worktree list --porcelain)"
expect_exit 0 mj "$R" worktree migrate --plan
expect_grep "$W/outside"
expect_grep "1 staged, 1 unstaged, 1 untracked"
[ "$(git -C "$R" worktree list --porcelain)" = "$before" ] || { echo "    --plan changed the topology"; exit 1; }
[ -d "$R-wt/fix/legacy" ] && { echo "    --plan created the destination"; exit 1; }

# ---------------------------------------------------------------- 6. migrate moves and verifies
expect_exit 0 mj "$R" worktree migrate
expect_grep "moved and verified"
[ -d "$R-wt/fix/legacy" ] || { echo "    the worktree was not moved"; exit 1; }
[ -e "$W/outside" ] && { echo "    the old path survived"; exit 1; }
[ "$(git -C "$R-wt/fix/legacy" rev-parse HEAD)" = "$head_before" ] || { echo "    HEAD changed"; exit 1; }
[ "$(git -C "$R-wt/fix/legacy" status --porcelain | LC_ALL=C sort)" = "$status_before" ] || { echo "    the dirty state changed"; git -C "$R-wt/fix/legacy" status --porcelain; exit 1; }
[ "$(cat "$R-wt/fix/legacy/untracked.txt")" = untracked ] || { echo "    untracked work was lost"; exit 1; }
[ "$(cat "$R-wt/fix/legacy/README.md")" = modified ] || { echo "    the modification was lost"; exit 1; }
expect_exit 0 mj "$R" worktree validate

# ---------------------------------------------------------------- 7. the hook asks the guard
mkdir -p "$R/.githooks"
cat > "$R/.githooks/pre-commit" <<H
#!/bin/sh
MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$R/share" "$R/bin/majordomus-cli" worktree guard --quiet || exit \$?
H
chmod +x "$R/.githooks/pre-commit"
# absolute: a relative hooksPath resolves against the worktree the hook runs in, and the
# linked worktrees of this fixture do not carry the hooks directory (the repository's own
# .githooks/ is tracked, so every worktree of it does)
git -C "$R" config core.hooksPath "$R/.githooks"
# a feature branch in its canonical worktree commits
echo work > "$R-wt/fix/legacy/work.txt"; git -C "$R-wt/fix/legacy" add work.txt
expect_exit 0 git -C "$R-wt/fix/legacy" commit -qm "canonical"
# the same branch, if it were somewhere else, would not: move it back out by hand and try
git -C "$R" worktree move "$R-wt/fix/legacy" "$W/stray"
echo more > "$W/stray/more.txt"; git -C "$W/stray" add more.txt
# git exits 1 when a pre-commit hook fails, whatever the hook's own code; the refusal is in the output
expect_exit 1 git -C "$W/stray" commit -qm "stray"
expect_grep "worktree.path_mismatch"
[ -z "$(git -C "$W/stray" log --oneline -1 --grep=stray)" ] || { echo "    the commit went through"; exit 1; }
# the primary checkout on a feature branch is refused too
git -C "$R" switch -q -c feature/oops
echo x > "$R/oops.txt"; git -C "$R" add oops.txt
expect_exit 1 git -C "$R" commit -qm "oops"
expect_grep "worktree.primary_on_non_trunk"
git -C "$R" reset -q oops.txt; rm -f "$R/oops.txt"; git -C "$R" switch -q "$trunk"

# ---------------------------------------------------------------- 8. doctor sees the enforcement entry
#      The fixture carries this repository's policy, whose enforcement list declares the
#      guard; doctor's wiring check must find the hook line and the launcher it names. Other
#      findings of doctor in a fixture (the derived merge driver of this clone, the local
#      layout) are not this case's subject, so only the wiring line is asserted.
cat > "$R/.githooks/pre-commit" <<H
#!/bin/sh
$R/bin/majordomus doctor || exit \$?
MAJORDOMUS_BIN="$RB" $R/bin/majordomus-cli worktree guard --quiet || exit \$?
H
cat > "$R/.githooks/pre-push" <<H
#!/bin/sh
$R/bin/majordomus finish --check || exit \$?
H
chmod +x "$R/.githooks/pre-commit" "$R/.githooks/pre-push"
[ -x "$R/bin/majordomus-cli" ] || { echo "    the fixture carries no bin/majordomus-cli launcher"; exit 1; }
grep -q 'worktree-guard-on-commit' "$R/.ai/repo/policy.yaml" || { echo "    the fixture policy declares no worktree guard"; exit 1; }
doctor_out="$(env -i PATH="$PATH" HOME="$HOME" MAJORDOMUS_BIN="$RB" sh -c "cd '$R' && '$R/bin/majordomus' doctor" 2>&1 || true)"
printf '%s\n' "$doctor_out" | grep -q 'OK   wiring      worktree-guard-on-commit' \
  || { echo "    doctor does not report the guard as wired:"; printf '%s\n' "$doctor_out" | grep -i 'worktree\|wiring' ; exit 1; }
# and an unwired hook is a finding: remove the line and doctor says so
sed -i.bak '/worktree guard/d' "$R/.githooks/pre-commit"; rm -f "$R/.githooks/pre-commit.bak"
doctor_out="$(env -i PATH="$PATH" HOME="$HOME" MAJORDOMUS_BIN="$RB" sh -c "cd '$R' && '$R/bin/majordomus' doctor" 2>&1 || true)"
printf '%s\n' "$doctor_out" | grep -q 'FAIL wiring      worktree-guard-on-commit' \
  || { echo "    doctor does not report the missing guard line:"; printf '%s\n' "$doctor_out" | grep -i 'worktree\|wiring'; exit 1; }

# ---------------------------------------------------------------- 9. a new path starts approved for direnv
#      direnv approves an .envrc by path and content, so the repository's own .envrc at a
#      path that did not exist a moment ago is blocked until `direnv allow` runs there:
#      36 of 37 worktrees on 2026-09-09. create, ensure and a moved migrate step carry the
#      primary checkout's approval to the new path — and only that: an .envrc that is not
#      the primary checkout's stays as direnv left it, and when the primary checkout's is
#      not approved nothing is. A direnv of the case's own says whether the primary is
#      approved and records every allow it is asked for, so the decision is what is proved.
mkdir -p "$T/fakebin"
cat > "$T/fakebin/direnv" <<H
#!/bin/sh
case "\$1" in
  status) printf 'Loaded RC allowed 0\nFound RC allowed %s\n' "\${FAKE_DIRENV_ALLOWED:-0}" ;;
  allow) printf '%s\n' "\$2" >> "$T/allowed.log" ;;
  *) exit 2 ;;
esac
H
chmod +x "$T/fakebin/direnv"
# the .envrc must be tracked for a new worktree to carry it; section 8 left the fixture's
# pre-commit hook running doctor, which has findings of its own in a fixture, so the hook
# goes back to the guard alone (section 7's form) — the guard is what a commit here answers to
cat > "$R/.githooks/pre-commit" <<H
#!/bin/sh
MAJORDOMUS_BIN="$RB" MAJORDOMUS_SHARE="$R/share" "$R/bin/majordomus-cli" worktree guard --quiet || exit \$?
H
printf 'PATH_add bin\n' > "$R/.envrc"; git -C "$R" add .envrc
expect_exit 0 git -C "$R" commit -qm envrc
mjd() { local cwd="$1"; shift; ( cd "$cwd" && PATH="$T/fakebin:$PATH" MAJORDOMUS_SHARE="$R/share" "$RB" "$@" ); }
expect_exit 0 mjd "$R" worktree create feature/approved
expect_grep "envrc   approved for direnv"
grep -qx "$R-wt/feature/approved/.envrc" "$T/allowed.log" 2>/dev/null \
  || { echo "    direnv allow was not asked for the new path:"; cat "$T/allowed.log" 2>/dev/null; exit 1; }
# ensure carries it too, and the JSON says so for whoever reads the report
expect_exit 0 mjd "$R" worktree ensure feature/approved --format json
expect_grep '"outcome": *"approved"'
# an .envrc that is not the primary checkout's is moved with its work and left blocked;
# direnv is not asked, because approving what the person has not read is the one thing
# `direnv allow` exists to prevent
git -C "$R" worktree add -q -b fix/foreign "$W/foreign" >/dev/null
printf 'eval "$(something else)"\n' > "$W/foreign/.envrc"
expect_exit 0 mjd "$R" worktree migrate
expect_grep "moved and verified"
expect_grep "envrc   differs from the primary checkout's"
grep -q "fix/foreign" "$T/allowed.log" && { echo "    direnv allow was asked for an .envrc that is not the primary checkout's"; exit 1; }
# and when the primary checkout's own .envrc is not approved, nothing is approved anywhere
rm -f "$T/allowed.log"
export FAKE_DIRENV_ALLOWED=1
expect_exit 0 mjd "$R" worktree create feature/unapproved
unset FAKE_DIRENV_ALLOWED
expect_grep "envrc   the primary checkout's is not approved"
[ -e "$T/allowed.log" ] && { echo "    direnv allow was asked although the primary checkout is not approved:"; cat "$T/allowed.log"; exit 1; }
# without direnv at all, nothing is blocked and the report says why nothing was done: a
# PATH with git on it and nothing else
mkdir -p "$T/gitonly"; ln -s "$(command -v git)" "$T/gitonly/git"
expect_exit 0 env -i PATH="$T/gitonly" HOME="$HOME" MAJORDOMUS_SHARE="$R/share" /bin/sh -c "cd '$R' && '$RB' worktree ensure feature/approved"
expect_grep "envrc   direnv is not installed"

echo "    worktree topology: derived from git, guarded on commit, migrated with its work, approved for direnv at its new path, wired and verified"
