# The wiring verifier is the reason this layer exists, so it is itself mutation-tested:
# each link of the chain is broken in a throwaway copy and doctor must go red. A verifier
# that survives broken wiring proves nothing.
. "$ROOT/test/lib.sh"
# The mutations below rewrite the tool itself, so the case runs against its own copy.
mkdir -p "$T/tool" "$T/tool/.github/workflows"
fixture_repo "$T/tool" docs
cp -R "$ROOT/test" "$T/tool/"
cp "$ROOT/.github/workflows/validate.yml" "$T/tool/.github/workflows/"
MJ="$T/tool/bin/majordomus"
REG="$T/tool/share/doctrines.yaml"; LIBD="$T/tool/lib"
CI="$T/tool/.github/workflows/validate.yml"; RUN="$T/tool/test/run.sh"
cp "$REG" "$REG.orig"
# The two declared enforcement hooks: a fresh checkout has none, and their absence is a
# real doctor failure that would mask the one this case is about.
wire() {
  mkdir -p .git/hooks
  printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
  printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
  chmod +x .git/hooks/pre-commit .git/hooks/pre-push
  PATH="$(dirname "$MJ"):$PATH"; export PATH
}
# A mutation must be shown to have taken before anything is asserted about what it
# caused. sed is silent when its pattern matches nothing: it exits 0 having changed
# nothing, and the case then asserts that a no-op produced no failure, which is
# trivially true. That is how mutation 2 here went green for a while after
# mj_run_task_checks stopped being a one-liner. One grep per probe closes it.
took() { # file, text the mutation must have introduced
  grep -qF -- "$2" "$1" || { printf '    the probe did not take: %s is absent from %s\n' "$2" "$1"; exit 1; }
}
# ...and the inverse, for a mutation that removes rather than adds.
gone() { # file, text the mutation must have removed
  grep -qF -- "$2" "$1" && { printf '    the probe did not take: %s is still present in %s\n' "$2" "$1"; exit 1; }
  return 0
}
restore() {
  cp "$REG.orig" "$REG"
  cp "$ROOT/lib/check.sh" "$LIBD/check.sh"
  cp "$ROOT/.github/workflows/validate.yml" "$CI"
  cp "$ROOT/test/run.sh" "$RUN"
}
"$MJ" init >/dev/null; "$MJ" update >/dev/null
wire

# a healthy tree passes, and says so in one line naming a derived count
expect_exit 0 "$MJ" doctor
expect_grep 'OK +doctrine +[0-9]+ doctrines .* validator, dispatch, propagation, test and CI resolve'

# `doctrine status` derives its counts; nothing is written down
expect_exit 0 "$MJ" doctrine status
expect_grep 'missing validators:   0'
expect_grep 'without a test file:  0'
expect_exit 0 "$MJ" doctrine list
expect_grep 'scope_integrity +blocking +mj_validate_scope'
expect_exit 0 "$MJ" doctrine show scope_integrity
expect_grep 'wired       yes'
expect_exit 12 "$MJ" doctrine show no_such_doctrine


# 1. declared doctrine, no validator function
sed 's/^    validator: scope$/    validator: nothing_implements_this/' "$REG.orig" > "$REG"
took "$REG" "validator: nothing_implements_this"
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL doctrine .* validator function mj_validate_nothing_implements_this is defined nowhere'
restore

# 2. validator exists but its command never dispatches
sed '/mj_doctrine_dispatch check/d' "$ROOT/lib/check.sh" > "$LIBD/check.sh"
gone "$LIBD/check.sh" "mj_doctrine_dispatch check"
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL doctrine .* lib/check.sh never calls mj_doctrine_dispatch'
restore

# 3. a blocking doctrine whose command cannot turn a finding into a non-zero exit
sed -e 's/exit "\$MJ_EX_CONTRACT"/exit 0/g' -e 's/\[ "\$MJ_FAILS" = 0 \] \&\& exit "\$MJ_EX_OK" || exit 0/exit 0/' "$ROOT/lib/check.sh" \
  | sed 's/MJ_FAILS/MJ_NOTHING/g' > "$LIBD/check.sh"
gone "$LIBD/check.sh" "MJ_FAILS"
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL doctrine .* never turns a failing finding into a non-zero exit'
restore

# 4. the test that proves a doctrine does not exist
sed 's#^    test: test/cases/04_start_check.sh$#    test: test/cases/99_absent.sh#' "$REG.orig" > "$REG"
took "$REG" "test: test/cases/99_absent.sh"
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL doctrine .* test test/cases/99_absent.sh does not exist'
restore

# 5. a doctrine naming a claim that is not in the claims file
sed 's/^    claims: \[scope-enforcement, scoped-task\]$/    claims: [scope-enforcement, invented-claim]/' "$REG.orig" > "$REG"
took "$REG" "invented-claim"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL doctrine .* names claim 'invented-claim'"
restore

# 6. a validator no doctrine declares — enforcement that runs under no rule
printf '\nmj_validate_orphan_rule() { return 0; }\n' >> "$LIBD/check.sh"
took "$LIBD/check.sh" "mj_validate_orphan_rule"
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL doctrine +mj_validate_orphan_rule .* no doctrine declares it'
restore

# 7. CI stops running the suite
sed 's#run: bash test/run.sh#run: echo skipped#' "$ROOT/.github/workflows/validate.yml" > "$CI"
gone "$CI" "run: bash test/run.sh"
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL doctrine +ci .* does not run test/run.sh'
restore

# 8. CI runs the suite but swallows its exit code
sed 's#run: bash test/run.sh#run: bash test/run.sh || true#' "$ROOT/.github/workflows/validate.yml" > "$CI"
took "$CI" "bash test/run.sh || true"
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL doctrine +ci .* does not let it fail the job'
restore

# 9. the runner stops globbing, so a new case would silently not run
sed 's#cases/\*\.sh#cases/01_init.sh#' "$ROOT/test/run.sh" > "$RUN"
gone "$RUN" "cases/*.sh"
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL doctrine +runner .* does not glob'
restore

# healthy again after every mutation is reverted
expect_exit 0 "$MJ" doctor
