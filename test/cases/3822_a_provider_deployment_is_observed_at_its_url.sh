# majordomus-covers: capability:served.observe capability:served.show
# A deployment that names a provider is observed at its own URL, and is active only while it
# serves the commit (I3822, ADR 0219).
#
# The deployment object says where the hosted application answers (`provider.url`) and which
# route reports the commit the running executable was built from (`health.liveness`). So
# `served observe --deployment <id>` needs neither --url nor --identity for such an object, and
# `served show` derives the object's status from the newest observation instead of trusting the
# hand-set word: `active` only while that observation, re-judged, serves the commit asked about.
#
# The history is A <- B. The fixture's liveness answer is served from a local directory (the
# provider is a prop; the address resolution and the judgement are the subject):
#   serves B, clean         -> served (0), status active
#   serves A                -> stale (10), status declared
#   `dirty: null`           -> malformed (12), naming the null, status declared
#   nothing                 -> unreachable (12), status declared
# and with no --url at all the probe goes to the object's provider URL and liveness route.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

repo="$PWD/repo"; site="$PWD/site"
mkdir -p "$repo" "$site/api/v1"
g() { git -C "$repo" -c user.name=t -c user.email=t@t "$@"; }
g init -q
mkdir -p "$repo/.ai/repo/knowledge" "$repo/.ai/repo/profiles" "$repo/.ai/repo/rules" "$repo/.ai/repo/deployments"
cp "$ROOT/.ai/manifest.yaml" "$ROOT/.ai/README.md" "$repo/.ai/"
cp "$ROOT/.ai/repo/policy.yaml" "$ROOT/.ai/repo/scope.yaml" "$repo/.ai/repo/"
cp "$ROOT/.ai/repo/knowledge/sources.yaml" "$repo/.ai/repo/knowledge/"
cp "$ROOT"/.ai/repo/profiles/*.yaml "$repo/.ai/repo/profiles/" 2>/dev/null || true
cp "$ROOT"/.ai/repo/rules/project/*.md "$repo/.ai/repo/rules/" 2>/dev/null || true
cat > "$repo/.ai/repo/deployments/hosted.yaml" <<'YAML'
schema: deployment/v1
kind: deployment
id: hosted
title: The hosted deployment
status: declared
application: hosted
build:
  package: majordomus-cli
  binary: majordomus
  profile: release
  inputs:
    - apps/majordomus-cli
listen:
  port: 8080
  interface: all
health:
  liveness: /api/v1/live
  readiness: /api/v1/ready
resources:
  cpu_kind: shared
  cpus: 1
  memory_mb: 256
machines:
  count: 1
  min_running: 0
  autostart: true
  autostop: true
region: fra
provider:
  name: gigalixir
  app: hosted-app
  stack: gigalixir-24
  url: https://hosted-app.invalid
YAML
g add -A .ai
for c in a b; do echo "$c" > "$repo/$c"; g add "$c"; g commit -q -m "$c"; done
A="$(g rev-parse HEAD~1)"; B="$(g rev-parse HEAD)"

live() { printf '{"alive":true,"version":"0.0.0","commit":"%s","dirty":%s}\n' "$1" "$2" > "$site/api/v1/live"; }
start_http "$site" || skip "no python3 or node to serve the fixture liveness answer"
trap stop_http EXIT

observe() { # <expected-exit> <verdict> <args...>
  local want="$1" verdict="$2"; shift 2
  local rc=0
  "$RB" served observe --repo "$repo" --deployment hosted --commit "$B" --timeout 3 \
    --format json "$@" > out.json 2> err.txt || rc=$?
  [ "$rc" = "$want" ] || { echo "    served observe $* exited $rc, expected $want"; cat out.json err.txt; exit 1; }
  jq -e --arg v "$verdict" '.observation.verdict == $v' out.json >/dev/null \
    || { echo "    served observe $* judged $(jq -r '.observation.verdict' out.json), expected $verdict"; cat out.json; exit 1; }
}
status() { # <expected-status>
  "$RB" served show --repo "$repo" --deployment hosted --commit "$B" --format json > show.json \
    || { echo "    served show failed"; cat show.json; exit 1; }
  jq -e --arg s "$1" '.deployments | length == 1 and .[0].status == $s' show.json >/dev/null \
    || { echo "    the hosted deployment's status is $(jq -c '[.deployments[].status]' show.json), expected $1"; cat show.json; exit 1; }
}

# ---------------------------------------------------------------- the identity comes from the object
live "$B" false
observe 0 served --url "$HTTP_BASE"
jq -e --arg u "$HTTP_BASE/api/v1/live" '.observation.url == $u' out.json >/dev/null \
  || { echo "    the probe did not go to the object's liveness route: $(jq -r '.observation.url' out.json)"; exit 1; }
status active

# ---------------------------------------------------------------- active only while it serves the commit
live "$A" false
observe 10 stale --url "$HTTP_BASE"
status declared

live "$B" false
observe 0 served --url "$HTTP_BASE"
status active

live "$B" null
observe 12 malformed --url "$HTTP_BASE"
jq -e '.observation.reason | contains("dirty: null")' out.json >/dev/null \
  || { echo "    a null dirtiness is not named as the reason: $(jq -r '.observation.reason' out.json)"; exit 1; }
status declared

stop_http
observe 12 unreachable --url "$HTTP_BASE"
status declared

# ---------------------------------------------------------------- with no --url, the provider's own address
observe 12 unreachable --dry-run
jq -e '.observation.url == "https://hosted-app.invalid/api/v1/live"' out.json >/dev/null \
  || { echo "    without --url the probe did not go to the provider's liveness route: $(jq -r '.observation.url' out.json)"; exit 1; }

[ -z "$(g status --porcelain --untracked-files=no)" ] || { echo "    observing changed a tracked file"; exit 1; }

echo "    observed at the object's provider URL and liveness route; active only while serving the commit"
