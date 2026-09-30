# Unknown is never pass (ADR 0071). A delivery dimension that could not be measured is
# `unknown`, and a feature with any unknown dimension does not exist — whatever the reason
# the measurement failed. This case removes, one at a time, the things a measurement needs.
#
# What it proves, all of it negative:
#   1  a public site that does not answer: deployed and publicly verified are unknown, not
#      fail and never pass, and the feature does not exist
#   2  `delivery report --check` exits 10 when a feature does not exist
#   3  a clone without origin/master: on master is unknown, and the stage is unknown
#   4  a feature id the layer does not declare is not found (exit 12), never "not delivered"
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
W="$(mktemp -d "${TMPDIR:-/tmp}/mj374.XXXXXX")"; trap 'rm -rf "$W"' EXIT

jqe() {          # jqe <file> <filter> <what broke>
  jq -e "$2" "$W/$1.json" >/dev/null 2>&1 || { printf '    %s\n' "$3"; jq -c . "$W/$1.json" | head -c 2000; echo; return 1; }
}

"$MJ" init >/dev/null
git checkout -q -B master
mkdir -p .ai/repo/features .ai/repo/ci apps/majordomus-cli/src/capability/builtin
printf 'version: 1\ndeploy:\n  branch: gh-pages\n  identity: build.json\n' > .ai/repo/ci/pages.yaml
cat > .ai/repo/features/shipped.md <<'MD'
---
schema: feature/v1
id: shipped
kind: feature
title: 'The feature this case cannot measure'
headline: 'It is merged, and nobody can see the site.'
summary: 'A feature made of one capability module, whose source file this fixture commits.'
status: draft
modules: [delivery]
---

## What it does

Ships, as far as anybody can tell.
MD
echo '// the module' > apps/majordomus-cli/src/capability/builtin/delivery.rs
git add -A >/dev/null && git commit -qm install
git update-ref refs/remotes/origin/master HEAD

# ---------------------------------------------------------------- 1  no site answers
export MAJORDOMUS_DELIVERY_SITE_URL="file://$W/no-such-site"
run_quiet "$W/offline.err" "$RB" delivery show shipped --format json > "$W/offline.json"
jqe offline '.dimensions[0].verdict == "pass"' "the fixture is not on master, so the case proves nothing"
jqe offline '.dimensions[1].verdict == "unknown"' "an unreachable identity is not unknown for deployed"
jqe offline '.dimensions[2].verdict == "unknown"' "an unreachable identity is not unknown for verified"
jqe offline '.exists == false and .delivery.state == "not_delivered"' "an unmeasured feature exists"
jqe offline '[.dimensions[] | select(.verdict != "pass") | .remediation] | all(. != null)' \
  "a dimension that does not pass carries no remediation"

# ---------------------------------------------------------------- 2  the check
expect_exit 10 "$RB" delivery report --check
expect_grep 'unknown'
expect_exit 0 "$RB" delivery report

# ---------------------------------------------------------------- 3  no trunk in the clone
git update-ref -d refs/remotes/origin/master
run_quiet "$W/notrunk.err" "$RB" delivery show shipped --format json > "$W/notrunk.json"
jqe notrunk '.dimensions[0].verdict == "unknown" and .delivery.stage == "unknown"' \
  "a clone without origin/master is not unknown for on master"
jqe notrunk '.dimensions[1].verdict == "unknown"' "deployment is decided without knowing the trunk"

# ---------------------------------------------------------------- 4  an absent feature
expect_exit 12 "$RB" delivery show no-such-feature
