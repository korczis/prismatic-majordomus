# majordomus-covers: doctor
# majordomus-negative: doctor
. "$ROOT/test/lib.sh"
# Runtime limit enforcement is chosen, never delivered by an update (I0203). Policy
# profiles.enforce_limits is false in the skeleton, absent and false both mean off, only true
# is on, and anything else is refused by doctor. A repository that never sets the key behaves
# as one that declines it: the lifecycle below runs under both, and the outputs are compared
# with each other rather than with a constant.
W="$(mktemp -d "${TMPDIR:-/tmp}/mj-le.XXXXXX")"; trap 'rm -rf "$W"' EXIT
"$MJ" init >/dev/null
POL=.ai/repo/policy.yaml
# the skeleton declines in writing: an opt-in that defaults to on is not an opt-in
grep -Eq '^  enforce_limits: false( |$)' "$ROOT/share/skeleton/policy.yaml"
grep -Eq '^  enforce_limits: false( |$)' "$POL"
cp "$POL" "$W/policy.false"
grep -v 'enforce_limits:' "$W/policy.false" > "$W/policy.absent"

expect_exit 0 "$MJ" start "limits" --scope lib --profile debugging
# what a worker meets, with what differs between two runs of the same thing left out
observe() {
  { "$MJ" check 2>&1 || true
    "$MJ" context 2>&1 || true
    "$MJ" doctrine limits --json 2>&1 | jq -c 'del(.enforcement_reason)' || true
  } | grep -v 'enforcement' \
    | sed -E 's/[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:.]+Z//g; s/[0-9]+m ago/Nm ago/g; s/[0-9]+ (second|minute)s? ago/N ago/g'
}

# declined in writing
doctor_out() { LAST_OUT="$("$MJ" doctor 2>&1)" || true; }
doctor_out
expect_grep 'OK +profile-limits +enforcement — off — policy profiles.enforce_limits: false$'
expect_exit 0 "$MJ" doctrine limits
expect_grep '^enforcement off — policy profiles.enforce_limits: false$'
observe > "$W/seen.false"

# never set: the same answers, and said to be off because it is absent
cp "$W/policy.absent" "$POL"
doctor_out
expect_grep 'OK +profile-limits +enforcement — off — policy profiles.enforce_limits is absent, which is off$'
expect_no_grep 'FAIL +profile-limits'
"$MJ" doctrine limits --json | jq -e '.enforcement == "off"' >/dev/null
observe > "$W/seen.absent"
diff -u "$W/seen.false" "$W/seen.absent" || { printf '    a repository that never set the key behaved differently\n'; exit 1; }
[ -s "$W/seen.absent" ]
# the projection a worker reads says off, from the policy
expect_exit 0 "$MJ" update
expect_grep 'Runtime limit enforcement is `off`' AGENTS.md

# only true is on; doctor reports it and does not turn anything on by itself
sed 's/^  enforce_limits: false/  enforce_limits: true/' "$W/policy.false" > "$POL"
doctor_out
expect_grep 'OK +profile-limits +enforcement — on — policy profiles.enforce_limits: true$'
"$MJ" doctrine limits --json | jq -e '.enforcement == "on"' >/dev/null
expect_exit 0 "$MJ" update
expect_grep 'Runtime limit enforcement is `on`' AGENTS.md

# anything but a boolean is refused by name, never read as off
sed 's/^  enforce_limits: false/  enforce_limits: sometimes/' "$W/policy.false" > "$POL"
doctor_out
expect_grep 'FAIL +profile-limits +enforcement — policy profiles.enforce_limits: sometimes is not true or false'
expect_exit 0 "$MJ" doctrine limits
expect_grep '^enforcement invalid — '
cp "$W/policy.false" "$POL"
