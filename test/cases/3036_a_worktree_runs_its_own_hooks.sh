# majordomus-covers: doctor
# A linked worktree runs the hooks of its own tree (I2718). core.hooksPath lives in the
# configuration every worktree shares; holding an absolute <primary>/.githooks, it sent every
# linked worktree to the primary checkout's hooks, and a primary three days behind refused
# every commit of current master in every worktree. `worktree create`, `migrate` and `repair`
# bind a linked worktree to its own .githooks with git's per-worktree configuration, and only
# when git would run another checkout's; doctor, in a worktree made by hand without that,
# fails `wiring hooks-path` with the command that fixes it. The primary checkout's hooks path
# is never changed, and a repository with a relative or no core.hooksPath gets nothing set.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

# Nested one level down so the container (<repo>-wt) stays under $T; resolved, because git
# reports the resolved path and on macOS that is not the $TMPDIR spelling.
mkdir -p "$T/wt"; W="$(cd "$T/wt" && pwd -P)"; R="$W/repo"
fixture_repo "$R" >/dev/null
mkdir -p "$R/.githooks"
# the hook says which file git ran, so a commit proves whose hooks judged it
printf '#!/bin/sh\necho "RAN=$0"\n' > "$R/.githooks/pre-commit"
chmod +x "$R/.githooks/pre-commit"
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
git -C "$R" add -A >/dev/null
git -C "$R" -c core.hooksPath=/dev/null commit -qm fixture >/dev/null
[ -x "$R/.githooks/pre-commit" ] || { echo "    the fixture does not track its hooks"; exit 1; }

mj() { local cwd="$1"; shift; ( cd "$cwd" && MAJORDOMUS_SHARE="$R/share" "$RB" "$@" ); }
hooks_of() { git -C "$1" rev-parse --path-format=absolute --git-path hooks; }
doctor_in() { env -i PATH="$PATH" HOME="$HOME" MAJORDOMUS_BIN="$RB" sh -c "cd '$1' && '$1/bin/majordomus' doctor" 2>&1 || true; }

# ---------------------------------------------------------------- 1. relative: already its own
#      git resolves a relative hooksPath against each worktree's own top level, so nothing is
#      written: no per-worktree value, no extension switched on.
git -C "$R" config core.hooksPath .githooks
expect_exit 0 mj "$R" worktree create feature/relative
expect_grep "hooks   $R-wt/feature/relative/.githooks, its own"
[ "$(hooks_of "$R-wt/feature/relative")" = "$R-wt/feature/relative/.githooks" ] \
  || { echo "    a relative hooksPath resolves to $(hooks_of "$R-wt/feature/relative")"; exit 1; }
[ -z "$(git -C "$R" config --get extensions.worktreeConfig)" ] \
  || { echo "    extensions.worktreeConfig was switched on where nothing needed it"; exit 1; }

# ---------------------------------------------------------------- 2. absolute: the defect, bound
git -C "$R" config core.hooksPath "$R/.githooks"
[ "$(hooks_of "$R-wt/feature/relative")" = "$R/.githooks" ] \
  || { echo "    the fixture does not reproduce the defect: $(hooks_of "$R-wt/feature/relative")"; exit 1; }
expect_exit 0 mj "$R" worktree create feature/own
expect_grep "hooks   $R-wt/feature/own/.githooks, its own [(]git config --worktree core.hooksPath; enabled"
expect_grep "extensions.worktreeConfig=true"
[ "$(hooks_of "$R-wt/feature/own")" = "$R-wt/feature/own/.githooks" ] \
  || { echo "    the new worktree runs $(hooks_of "$R-wt/feature/own")"; exit 1; }
[ "$(git -C "$R" config --get extensions.worktreeConfig)" = true ] || { echo "    the extension is off"; exit 1; }
# the primary checkout and the shared value are what they were
[ "$(hooks_of "$R")" = "$R/.githooks" ] || { echo "    the primary checkout now runs $(hooks_of "$R")"; exit 1; }
[ "$(git -C "$R" config --local --get core.hooksPath)" = "$R/.githooks" ] \
  || { echo "    the shared core.hooksPath was changed"; exit 1; }
# and the hook git runs on a commit there is the worktree's own, measured by running it
echo own > "$R-wt/feature/own/own.txt"; git -C "$R-wt/feature/own" add own.txt
expect_exit 0 git -C "$R-wt/feature/own" commit -qm own
expect_grep "RAN=$R-wt/feature/own/.githooks/pre-commit"
# the JSON names the outcome for whoever reads the report
expect_exit 0 mj "$R" worktree ensure feature/own --format json
expect_grep '"outcome": *"own"'

# ---------------------------------------------------------------- 3. by hand: doctor's finding
git -C "$R" worktree add -q -b fix/by-hand "$R-wt/fix/by-hand" >/dev/null 2>&1
H="$R-wt/fix/by-hand"
[ "$(hooks_of "$H")" = "$R/.githooks" ] || { echo "    a worktree made by hand runs $(hooks_of "$H")"; exit 1; }
out="$(doctor_in "$H")"
printf '%s\n' "$out" | grep -q "FAIL wiring      hooks-path — this linked worktree runs $R/.githooks, not its own $H/.githooks" \
  || { echo "    doctor does not name the borrowed hooks:"; printf '%s\n' "$out" | grep -i 'wiring\|hooks'; exit 1; }
printf '%s\n' "$out" | grep -qF "fix: git -C $H config --worktree core.hooksPath $H/.githooks" \
  || { echo "    doctor does not print the fixing command:"; printf '%s\n' "$out" | grep -i 'hooks-path'; exit 1; }
# the primary checkout has no such finding
out="$(doctor_in "$R")"
printf '%s\n' "$out" | grep -q "hooks-path" && { echo "    doctor reports a hooks-path finding in the primary checkout:"; printf '%s\n' "$out" | grep hooks-path; exit 1; }
# the printed command fixes it, and doctor then has nothing to say about it
git -C "$H" config --worktree core.hooksPath "$H/.githooks"
[ "$(hooks_of "$H")" = "$H/.githooks" ] || { echo "    the printed command did not fix it"; exit 1; }
out="$(doctor_in "$H")"
printf '%s\n' "$out" | grep -q "hooks-path" && { echo "    doctor still reports hooks-path after the fix:"; printf '%s\n' "$out" | grep hooks-path; exit 1; }

# ---------------------------------------------------------------- 4. repair binds a hand-made one
git -C "$R" worktree add -q -b fix/repaired "$R-wt/fix/repaired" >/dev/null 2>&1
expect_exit 0 mj "$R" worktree repair
expect_grep "$R-wt/fix/repaired"
expect_grep "hooks   $R-wt/fix/repaired/.githooks, its own [(]git config --worktree core.hooksPath[)]"
[ "$(hooks_of "$R-wt/fix/repaired")" = "$R-wt/fix/repaired/.githooks" ] \
  || { echo "    repair left it running $(hooks_of "$R-wt/fix/repaired")"; exit 1; }

# ---------------------------------------------------------------- 5. migrate binds it again
#      A per-worktree value is absolute, so a worktree bound at one path and moved names a
#      directory that is gone — and git runs no hooks at all from a missing directory.
#      Bringing it home binds it to the directory at its new path.
git -C "$R" worktree add -q -b fix/strayed "$W/strayed" >/dev/null 2>&1
git -C "$W/strayed" config --worktree core.hooksPath "$W/strayed/.githooks"
expect_exit 0 mj "$R" worktree migrate
expect_grep "moved and verified"
expect_grep "  hooks   $R-wt/fix/strayed/.githooks, its own [(]git config --worktree core.hooksPath[)]"
[ "$(hooks_of "$R-wt/fix/strayed")" = "$R-wt/fix/strayed/.githooks" ] \
  || { echo "    the migrated worktree runs $(hooks_of "$R-wt/fix/strayed")"; exit 1; }
[ "$(hooks_of "$R")" = "$R/.githooks" ] || { echo "    the primary checkout now runs $(hooks_of "$R")"; exit 1; }
exit 0
