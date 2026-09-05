# majordomus-covers: start finish
# majordomus-negative: start finish
. "$ROOT/test/lib.sh"
# Every profile a fresh install ships is runnable, its axes reach the worker's view of
# the task, and the verification it declares is actually enforced by finish. A loop
# rather than a case per profile, so adding a profile keeps it covered.
# covers-all: profiles
"$MJ" init >/dev/null
# init adds the local-state ignore line; commit it so the task that follows starts from a clean tree
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
"$MJ" update >/dev/null
mkdir -p lib test

PROFILES="$(for f in .ai/repo/profiles/*.yaml; do basename "$f" .yaml; done)"
[ -n "$PROFILES" ] || { echo "    a fresh install shipped no profiles"; exit 1; }

field() { sed -n "s/^  *$2: //p" ".ai/repo/profiles/$1.yaml" | head -n1; }

for p in $PROFILES; do
  expect_exit 0 "$MJ" start "work under $p" --scope lib,test --profile "$p"
  expect_grep "profile=$p"
  id="$(sed -n 's/^id: //p' .ai/local/state/current.yaml)"

  # the axes come back out of the task record, not out of a template
  expect_exit 0 "$MJ" check --explain
  expect_grep "^# profile $p$"
  for axis in capability effort verbosity checkpoint_interval; do
    expect_grep "^  $axis=" || { echo "    profile $p exposes no $axis"; exit 1; }
  done

  # a profile that requires a regression test refuses an outcome without one
  if [ "$(field "$p" regression_test_required)" = true ]; then
    printf 'x\n' > lib/touched.txt
    printf '# Objective\no\n\n# Current State\ns\n\n# Next Action\nn\n' | "$MJ" handover >/dev/null
    expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
    expect_grep "profile $p requires a regression test"
    printf 'x\n' > test/regression.txt
  fi

  # a profile that requires a decision record refuses an outcome without one
  if [ "$(field "$p" decision_record_required)" = true ]; then
    printf 'x\n' > lib/touched.txt
    printf '# Objective\no\n\n# Current State\ns\n\n# Next Action\nn\n' | "$MJ" handover >/dev/null
    expect_exit 10 "$MJ" finish --outcome completed --verify-command "true"
    expect_grep "profile $p requires an entry 'Task: $id'"
    printf '\n## a decision\nTask: %s\n' "$id" >> .ai/local/state/decisions.md
  fi

  printf '# Objective\no\n\n# Current State\ns\n\n# Next Action\nn\n' | "$MJ" handover >/dev/null
  expect_exit 0 "$MJ" finish --outcome completed --verify-command "true"
  expect_grep "finish: $id completed"
  rm -f lib/touched.txt test/regression.txt
done

# an unknown profile is refused rather than silently defaulted
expect_exit 12 "$MJ" start "work" --scope lib --profile no-such-profile
expect_grep "no profile 'no-such-profile'"

# the default named by the policy is one of the shipped profiles
def="$(sed -n 's/^  default: //p' .ai/repo/policy.yaml)"
[ -f ".ai/repo/profiles/$def.yaml" ] || { echo "    policy default '$def' has no profile file"; exit 1; }
