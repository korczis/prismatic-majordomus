# majordomus-covers: capability:mesh.build_cache.provision
# majordomus-timeout: 300
# A node is provisioned as the mesh build cache's S3 provider, idempotently, with no secret
# in the repository or in any answer (I2674, ADR 0157).
#
# Never run against a real host: every step below reads or writes a temporary root, and the
# one step on `/` is the refusal a non-root caller gets before anything is touched.
#
#   1. the plan over an empty root owes every file, names every node's identity, marks the
#      credentials as generated and carries no secret;
#   2. --apply into the staging root writes the files, generates one credential per node
#      with mode 0600, and the plan after it owes nothing;
#   3. a second apply writes nothing, generates nothing and keeps every credential byte
#      for byte;
#   4. a drifted file is put back, and a cargo rustc-wrapper line is rewritten once;
#   5. no answer holds a credential's secret, and no tracked file of the repository does;
#   6. --apply on the real root without root is refused with nothing written.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3956.XXXXXX")"
trap 'rm -rf "$S"' EXIT
prov() { ( cd "$ROOT" && "$RB" mesh build-cache provision --format json "$@" ); }
fail() { echo "    $1"; shift; for f in "$@"; do [ -f "$f" ] && cat "$f"; done; exit 1; }

# ---------------------------------------------------------------- 1. the plan
prov --dry-run --root "$S/host" > "$S/plan.json" 2> "$S/err" || fail "the plan failed" "$S/err"
jq -e '.mode == "plan" and .apply == null and .ok == true' "$S/plan.json" >/dev/null \
  || fail "a dry run is not a plan that changed nothing" "$S/plan.json"
jq -e '.plan.changes > 0 and (.plan.files | all(.state == "missing"))' "$S/plan.json" >/dev/null \
  || fail "an empty root does not owe every file" "$S/plan.json"
[ ! -e "$S/host" ] || fail "a plan created its root"
for node in $(jq -r '.policy.nodes[]' "$S/plan.json"); do
  jq -e --arg n "$node" '.plan.files | any(.path == "/etc/majordomus/mesh-build-cache/" + $n + "/credentials" and .source == "generated" and .mode == "0600")' \
    "$S/plan.json" >/dev/null || fail "no generated 0600 credential for $node"
  jq -e --arg n "$node" '.plan.steps | any(.id == "identity-" + $n and (.line | contains("<secret of " + $n + ">")))' \
    "$S/plan.json" >/dev/null || fail "no identity step for $node"
done
jq -e '.plan.steps | map(.id) | index("bucket") < index("quota")' "$S/plan.json" >/dev/null \
  || fail "the bucket is not made before its quota"
jq -e '.plan.steps | any(.id == "expiry") and any(.id == "ci-cache-quota") and any(.id == "guard-timer")' \
  "$S/plan.json" >/dev/null || fail "the store's bounds or the disk guard are missing from the plan"
! grep -q 'aws_secret_access_key' "$S/plan.json" || fail "a plan names a secret"

# ---------------------------------------------------------------- 2. apply into a staging root
prov --apply --root "$S/host" > "$S/apply1.json" 2> "$S/err" || fail "the staged apply failed" "$S/err" "$S/apply1.json"
jq -e '.ok == true and (.apply.written | length) > 0 and (.apply.generated | length) == (.policy.nodes | length) and .apply.after.changes == 0' \
  "$S/apply1.json" >/dev/null || fail "the staged apply did not bring the root to the policy" "$S/apply1.json"
cred="$S/host/etc/majordomus/mesh-build-cache/mbp/credentials"
[ -f "$cred" ] || fail "no credential for mbp"
mode="$(ls -l "$cred" | cut -c1-10)"
[ "$mode" = "-rw-------" ] || fail "a credential is not mode 0600: $mode"
cat "$S"/host/etc/majordomus/mesh-build-cache/*/credentials > "$S/creds.cat"
sum1="$(sha256_of_file "$S/creds.cat")"

# ---------------------------------------------------------------- 3. the second apply changes nothing
prov --apply --root "$S/host" > "$S/apply2.json" 2> "$S/err" || fail "the second apply failed" "$S/err" "$S/apply2.json"
jq -e '.ok == true and (.apply.written | length) == 0 and (.apply.generated | length) == 0 and .plan.changes == 0' \
  "$S/apply2.json" >/dev/null || fail "a second apply changed something" "$S/apply2.json"
cat "$S"/host/etc/majordomus/mesh-build-cache/*/credentials > "$S/creds.cat"
[ "$sum1" = "$(sha256_of_file "$S/creds.cat")" ] \
  || fail "a second apply rewrote a credential"

# ---------------------------------------------------------------- 4. drift and the cargo wrapper
printf '{}' > "$S/host/etc/docker/daemon.json"
mkdir -p "$S/host/home/korczis/.cargo"
printf '[build]\nrustc-wrapper = "/home/korczis/.cargo/bin/sccache"\n' > "$S/host/home/korczis/.cargo/config.toml"
prov --dry-run --root "$S/host" > "$S/drift.json" 2> "$S/err" || fail "the drift plan failed" "$S/err"
jq -e '.plan.changes == 2' "$S/drift.json" >/dev/null || fail "drift and the wrapper edit are not both owed" "$S/drift.json"
prov --apply --root "$S/host" > "$S/apply3.json" 2> "$S/err" || fail "the repair failed" "$S/err" "$S/apply3.json"
grep -q 'sccache-mesh' "$S/host/home/korczis/.cargo/config.toml" || fail "the wrapper line was not rewritten"
! grep -q '\.cargo/bin/sccache"' "$S/host/home/korczis/.cargo/config.toml" || fail "the old wrapper line remains"
cmp -s "$S/host/etc/docker/daemon.json" "$ROOT/share/mesh-cache-provider/daemon.json" || fail "daemon.json was not put back"
prov --apply --root "$S/host" > "$S/apply4.json" 2> "$S/err" || fail "a fourth apply failed" "$S/err"
jq -e '(.apply.written | length) == 0' "$S/apply4.json" >/dev/null || fail "the repair is not idempotent" "$S/apply4.json"

# ---------------------------------------------------------------- 5. no secret anywhere
for f in "$S"/*.json; do
  for secret in $(sed -n 's/^aws_secret_access_key = //p' "$S"/host/etc/majordomus/mesh-build-cache/*/credentials); do
    ! grep -q "$secret" "$f" || fail "an answer holds a credential's secret: $f"
    ! ( cd "$ROOT" && git grep -q "$secret" -- . ) || fail "a tracked file holds a credential's secret"
  done
done
! ( cd "$ROOT" && git grep -nE 'aws_secret_access_key *= *[0-9a-f]{20,}' -- . ':!test/cases/3956*' ) \
  || fail "a tracked file holds a credential"

# ---------------------------------------------------------------- 6. the real root refuses without root
if [ "$(id -u)" -ne 0 ]; then
  rc=0; prov --apply > "$S/refused.json" 2> "$S/err" || rc=$?
  [ "$rc" -eq 10 ] || fail "applying on / without root did not exit 10 (exit $rc)" "$S/err" "$S/refused.json"
  jq -e '.ok == false and (.apply.refused | contains("needs root")) and (.apply.written | length) == 0 and (.apply.ran | length) == 0' \
    "$S/refused.json" >/dev/null || fail "the refusal is not the whole answer" "$S/refused.json"
fi
