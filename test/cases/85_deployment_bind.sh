# majordomus-covers: none
# The listen address is one fact with two projections.
#
# A server on a person's machine binds loopback, and that default does not move: a
# repository's AI layer, its diagnostics and its peers are not for every host on the café
# network. A server inside a machine somewhere binds every interface, because a hosted
# process that bound loopback would be unreachable inside its own container — and the way
# it says so is a canonical object, not a flag that turns the warning off.
#
# What this case proves is that the two are the same code path reading different data, and
# that the port exists once: it is changed in the object alone and the process follows.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj85.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
# the section is seeded: a repository that later deploys something adds one file, not a
# directory, a context document, a manifest entry and a source class
[ -f .ai/repo/deployments/README.md ] || { echo "    init did not seed the deployments section"; exit 1; }

write_deployment() {   # port
  cat > .ai/repo/deployments/hosted.yaml <<MD
schema: deployment/v1
kind: deployment
id: hosted
title: The hosted deployment
application: hosted
build:
  package: majordomus-cli
  binary: majordomus
  profile: release
  inputs:
    - .ai
listen:
  port: $1
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
  name: fly
MD
}

write_deployment 8087
git add -A >/dev/null; git commit -qm "the deployment object" >/dev/null

# ---------------------------------------------------------------- the local default
"$RB" serve --port 0 </dev/null >"$S/local.log" 2>&1 &
pid=$!; n=0
while [ $n -lt 150 ] && ! grep -q 'shared server listening' "$S/local.log" 2>/dev/null; do n=$((n+1)); sleep 0.1; done
kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true
grep -q 'listening on http://127\.0\.0\.1:' "$S/local.log" \
  || { echo "    the local default did not bind loopback:"; cat "$S/local.log"; exit 1; }
grep -q '0\.0\.0\.0' "$S/local.log" \
  && { echo "    the local default bound a reachable interface"; exit 1; }

# ---------------------------------------------------------------- the declared address
"$RB" serve --deployment hosted </dev/null >"$S/hosted.log" 2>&1 &
pid=$!; n=0
while [ $n -lt 150 ] && ! grep -q 'shared server listening' "$S/hosted.log" 2>/dev/null; do n=$((n+1)); sleep 0.1; done
# it bound every interface, on the port the object states, and said which object said so
grep -q 'listening on http://0\.0\.0\.0:8087' "$S/hosted.log" \
  || { echo "    the hosted server did not bind the declared address:"; cat "$S/hosted.log"; kill "$pid" 2>/dev/null || true; exit 1; }
grep -q 'deployments/hosted.yaml' "$S/hosted.log" \
  || { echo "    the hosted bind did not name the object that declared it"; kill "$pid" 2>/dev/null || true; exit 1; }
# and it is not the accidental-bind warning being suppressed: the warning is for a bind
# nobody declared, and this one was declared
grep -q 'bind 127.0.0.1 unless that is intended' "$S/hosted.log" \
  && { echo "    the declared bind produced the accidental-bind warning"; kill "$pid" 2>/dev/null || true; exit 1; }
kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true

# ---------------------------------------------------------------- the port lives once
# Change it in the object and nowhere else; the process follows.
write_deployment 8092
git add -A >/dev/null
"$RB" serve --deployment hosted </dev/null >"$S/moved.log" 2>&1 &
pid=$!; n=0
while [ $n -lt 150 ] && ! grep -q 'shared server listening' "$S/moved.log" 2>/dev/null; do n=$((n+1)); sleep 0.1; done
kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true
grep -q 'listening on http://0\.0\.0\.0:8092' "$S/moved.log" \
  || { echo "    the port was changed in the object and the process did not follow:"; cat "$S/moved.log"; exit 1; }

# ---------------------------------------------------------------- a deployment that is not there
expect_exit 12 "$RB" serve --deployment absent
expect_grep "no deployment 'absent'"

# ---------------------------------------------------------------- the two are exclusive
# The address is the object's or the command line's; asking for both is a question with
# two answers.
expect_exit 2 "$RB" serve --deployment hosted --port 9000
