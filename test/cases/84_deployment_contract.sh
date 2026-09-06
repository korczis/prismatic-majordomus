# majordomus-covers: doctor
# A deployment is described once, in the layer, against a closed contract.
#
# `init` seeds the section with its contract and nothing in it, so a repository that later
# deploys something adds one file rather than a directory, a context document, a manifest
# entry and a source class. An object in it is read, closed and refused by name with no
# registration anywhere: the source class and the kind are data, and the index, the
# listing and the graph follow them.
#
# What this case exists to prove is the refusals. A deployment that cannot work must cost a
# test failure rather than a failed rollout, and a credential must never reach the layer.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null

# ---------------------------------------------------------------- an empty section
# `init` seeds the section with its contract and no objects, so a repository that later
# deploys something adds one file. An empty section is not a finding: there is nothing
# that could be wrong.
git add -A >/dev/null; git commit -qm base
[ -f .ai/repo/deployments/README.md ] || { echo "    init did not seed the section's contract"; exit 1; }
"$MJ" doctor 2>&1 | grep -qE '^FAIL +deployment ' \
  && { echo "    an empty deployments section was reported as a failure"; exit 1; }
"$MJ" doctor 2>&1 | grep -q 'deployment(s)' \
  && { echo "    an empty section counted something"; exit 1; }

# ---------------------------------------------------------------- one deployment
cat > .ai/repo/deployments/example.yaml <<'MD'
schema: deployment/v1
kind: deployment
id: example
title: The example deployment
description: One machine that stops when idle.
application: example
build:
  package: example-cli
  binary: example
  profile: release
  inputs:
    - src
listen:
  port: 8080
  interface: all
health:
  liveness: /healthz
  readiness: /readyz
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
  name: fly
MD
git add -A >/dev/null

# read through the object surface, counted by doctor, refused by nothing
"$MJ" doctor 2>&1 | grep -q 'OK   deployment' || { echo "    doctor did not accept a valid deployment"; exit 1; }
# exactly one: the section's own contract is not an instance of the kind beside it
"$MJ" doctor 2>&1 | grep -q '1 deployment(s)' || { echo "    doctor did not count exactly the object"; exit 1; }

# ---------------------------------------------------------------- every refusal, by name
d=.ai/repo/deployments/example.yaml
cp "$d" keep.yaml
probe() {   # reason mutation...
  local want="$1"; shift
  "$@"
  git add -A >/dev/null
  "$MJ" doctor 2>&1 | grep -qE "^FAIL +deployment .*$want" || { echo "    doctor did not refuse: $want"; exit 1; }
  cp keep.yaml "$d"; git add -A >/dev/null
}
probe 'key\(s\) the contract does not have: fly_api_token' \
  sed -i.bak 's|^region: fra$|region: fra\nfly_api_token: "x"|' "$d"
probe 'this executable reads deployment/v1' \
  sed -i.bak 's|^schema: deployment/v1$|schema: deployment/v2|' "$d"
probe 'lacks region' \
  sed -i.bak 's|^region: fra$|region_was: fra|' "$d"
probe 'is neither loopback nor all' \
  sed -i.bak 's|^  interface: all$|  interface: public|' "$d"
probe 'is outside 1024-65535; the process runs unprivileged' \
  sed -i.bak 's|^  port: 8080$|  port: 80|' "$d"
probe 'machines.min_running 2 exceeds machines.count 1' \
  sed -i.bak 's|^  min_running: 0$|  min_running: 2|' "$d"
probe 'is not a path beginning with /' \
  sed -i.bak 's|^  liveness: /healthz$|  liveness: http://example.invalid/healthz|' "$d"
probe 'carries what reads as a credential' \
  sed -i.bak 's|^  org: .*$||; s|^provider:$|provider:\n  fly:\n    org: "FlyV1 fm2_notarealtoken"|' "$d"
rm -f "$d".bak

# two objects, one identity
cp "$d" .ai/repo/deployments/a-second-claim.yaml
git add -A >/dev/null
"$MJ" doctor 2>&1 | grep -q 'two objects claim this identity' || { echo "    a duplicate identity was not refused"; exit 1; }
rm -f .ai/repo/deployments/a-second-claim.yaml; git add -A >/dev/null

# ---------------------------------------------------------------- the contract is generated
# The allow-list is a projection of the schema, never written by hand: remove it and the
# validator says which command puts it back rather than passing silently.
allow="$(dirname "$(dirname "$MJ")")/share/allow/deployment.txt"
[ -f "$allow" ] || { echo "    share/allow/deployment.txt was not projected from the schema"; exit 1; }
grep -qx '\^listen\\.port\$' "$allow" || { echo "    the allow-list does not close listen.port"; exit 1; }
grep -qx '\^provider\\.fly\\.org\$' "$allow" || { echo "    the allow-list does not reach into the provider block"; exit 1; }

# and the restored object is accepted again: every refusal above was the mutation, not the shape
"$MJ" doctor 2>&1 | grep -q 'OK   deployment' || { echo "    the restored object is no longer accepted"; exit 1; }
