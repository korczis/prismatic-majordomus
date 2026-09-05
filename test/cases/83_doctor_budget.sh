# majordomus-covers: doctor watch
# majordomus-negative: doctor watch
#
# doctor and watch report their own wall time against the policy's budget for them: INFO
# under, WARN over, and the budget never becomes the exit code. The budget is a policy key
# like any other: declared, read with no reader-side default, and refused when missing.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
# doctor is healthy only with the hooks wired; the budget line is reported either way
printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push
PATH="$(dirname "$MJ"):$PATH"; export PATH

# --- under budget: the time and the budget on one INFO line; the budget is set so wide
#     that a loaded machine cannot cross it, because this asserts the mechanism, not the host
sed -i.bak 's/^    doctor_ms: 3000$/    doctor_ms: 600000/; s/^    watch_ms: 3000$/    watch_ms: 600000/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
"$MJ" update >/dev/null
expect_exit 0 "$MJ" doctor
expect_grep 'INFO +budget +doctor . [0-9]+ ms of 600000 ms'
expect_no_grep 'WARN +budget'
expect_exit 0 "$MJ" watch
expect_grep 'INFO +budget +watch . [0-9]+ ms of 600000 ms'

# --- over budget: WARN, naming the policy key, and still exit 0
sed -i.bak 's/^    doctor_ms: 600000$/    doctor_ms: 1/; s/^    watch_ms: 600000$/    watch_ms: 1/' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
"$MJ" update >/dev/null
expect_exit 0 "$MJ" doctor
expect_grep 'WARN +budget +doctor . [0-9]+ ms, over the budget of 1 ms \(policy benchmark.budget.doctor_ms\)'
expect_no_grep 'INFO +budget +doctor'
expect_exit 0 "$MJ" watch
expect_grep 'WARN +budget +watch . [0-9]+ ms, over the budget of 1 ms \(policy benchmark.budget.watch_ms\)'
expect_grep 'watch: 0 drift'

# --- the budget is declared or refused, never defaulted
sed -i.bak '/^    doctor_ms: 1$/d' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
"$MJ" update >/dev/null
expect_exit 10 "$MJ" doctor
expect_grep 'benchmark.budget.doctor_ms'
reset_policy; "$MJ" update >/dev/null
expect_exit 0 "$MJ" doctor
echo "    INFO under, WARN over naming the key, never the exit code, refused when undeclared"
