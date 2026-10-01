# claims: advisor-transports-share-one-contract
# Every advisor transport is held to one contract (ADR 0098), and the contract suite runs
# without a network, a model or a credential: each adapter against a fake fetch or a fake
# process. It covers availability through the catalogue (every adapter the catalogue names
# exists, and no adapter module exists that no advisor names), request normalisation,
# timeouts, cancellation, rate limits, authentication failures, malformed and empty
# answers, structured answers, usage reporting, the order of concurrent results, and the
# redaction of secrets from questions and diagnostics.
. "$ROOT/test/lib.sh"
command -v node >/dev/null 2>&1 || {
  [ -z "${CI:-}" ] || { echo "    node is required on CI: the contract suite cannot run"; exit 1; }
  skip "no node"; }
out="$(cd "$ROOT" && env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY node --test scripts/lib/advisors/contract.test.mjs 2>&1)" \
  || { printf '%s\n' "$out" | grep -E '^not ok|# (pass|fail)|Error|expected|actual' | head -40; exit 1; }
printf '%s\n' "$out" | grep -qE '^# fail 0$' || { printf '%s\n' "$out" | tail -20; exit 1; }
pass="$(printf '%s\n' "$out" | sed -n 's/^# pass \([0-9]*\)$/\1/p')"
# a suite that ran nothing passes nothing: the count is a measured floor
[ "${pass:-0}" -ge 17 ] || { echo "    the contract suite ran ${pass:-0} test(s), fewer than the 17 it holds"; exit 1; }
