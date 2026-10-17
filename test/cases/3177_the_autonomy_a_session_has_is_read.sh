# majordomus-timeout: 300
# The autonomy a session has is read from the facts (I2790). `majordomus autonomy status`
# derives the profile from the client's settings layers, the person's trust-dialog acceptance
# in their own ~/.claude.json and the isolation evidence, failing closed: an untrusted
# checkout is balanced, a checked-in bypassPermissions default raises nothing, the person's
# acceptance raises it to trusted, isolated needs a container marker and no credential in
# reach, and an unreadable layer drops it to conservative. `autonomy explain` answers with its
# decision as the exit code (0 allow, 10 defer, 11 require_approval), never allows a compound
# command, and holds the boundaries inside isolation. `autonomy friction` counts the refusals
# of this checkout's transcripts and carries no payload.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_BIN="$RB"

R="$T/repo"
mkdir -p "$R"
cd "$R" || exit 1
git init -q .
git config user.email t@example.com
git config user.name t
"$MJ" init >/dev/null
"$MJ" update >/dev/null
git add -A && git commit -qm base

# a home of the case's own, and no credential variable from the runner's environment
H="$T/home"
mkdir -p "$H/.claude"
for v in ANTHROPIC_API_KEY AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY AWS_SESSION_TOKEN \
  AZURE_CLIENT_SECRET CARGO_REGISTRY_TOKEN DOCKER_PASSWORD FLY_API_TOKEN GH_TOKEN GITHUB_TOKEN \
  GITLAB_TOKEN GOOGLE_APPLICATION_CREDENTIALS NPM_TOKEN OPENAI_API_KEY SSH_AUTH_SOCK container; do
  unset "$v"
done
export HOME="$H"
ROOT_SEEN="$("$RB" autonomy status --format json | jq -r '.root')"
[ -n "$ROOT_SEEN" ] || { echo "    status names no checkout"; exit 1; }
REAL="$(pwd -P)"

status() { "$RB" autonomy status --format json; }
profile() { status | jq -r '.profile'; }
expect_profile() { # <want> <what>
  local got; got="$(profile)"
  [ "$got" = "$1" ] || { echo "    $2: profile $got, wanted $1"; status; exit 1; }
}
trust() { # <true|false>
  printf '{"projects":{"%s":{"hasTrustDialogAccepted":%s},"%s":{"hasTrustDialogAccepted":%s}}}' \
    "$R" "$1" "$REAL" "$1" > "$H/.claude.json"
}

# ---------------------------------------------------------------- 1. untrusted, no isolation: balanced, with the refusals named
expect_profile balanced "an untrusted checkout"
status | jq -e '.recommended_mode == "acceptEdits"' >/dev/null \
  || { echo "    balanced does not launch with acceptEdits"; exit 1; }
status | jq -e '[.refused[].profile] == ["isolated","trusted"]' >/dev/null \
  || { echo "    the refusals of isolated and trusted are not named"; status; exit 1; }

# ---------------------------------------------------------------- 2. the repository's word raises nothing; the person's does
mkdir -p .claude
printf '{"permissions":{"defaultMode":"bypassPermissions","allow":["Bash(private-host *)"]}}' > .claude/settings.json
expect_profile balanced "a checked-in bypassPermissions default"
status | grep -q private-host && { echo "    rule text left the settings file"; exit 1; }
trust true
expect_profile trusted "the person's acceptance"
status | jq -e '.recommended_mode == "auto" and .configured_mode == "bypassPermissions"' >/dev/null \
  || { echo "    trusted does not launch with auto, or the configured mode is not reported"; status; exit 1; }

# ---------------------------------------------------------------- 3. isolation: a container marker and no credential in reach
export container=podman
expect_profile isolated "a container with no credential"
status | jq -e '.recommended_mode == "bypassPermissions"' >/dev/null \
  || { echo "    isolated does not launch with bypassPermissions"; exit 1; }
GITHUB_TOKEN=case-secret-value "$RB" autonomy status --format json > "$T/s.json"
jq -e '.profile == "trusted" and .isolation.credential_variables == ["GITHUB_TOKEN"]' "$T/s.json" >/dev/null \
  || { echo "    a credential variable did not refuse isolation"; cat "$T/s.json"; exit 1; }
grep -q case-secret-value "$T/s.json" && { echo "    a credential value left the process"; exit 1; }
mkdir -p "$H/.ssh" && : > "$H/.ssh/id_ed25519"
expect_profile trusted "a key file in reach"
rm -f "$H/.ssh/id_ed25519"
unset container

# ---------------------------------------------------------------- 4. an unreadable layer fails closed
printf '{ not json' > .claude/settings.local.json
expect_profile conservative "an unreadable settings layer"
rm -f .claude/settings.local.json

# ---------------------------------------------------------------- 5. explain: the decision is the exit code
explain() { # <want> <profile> <command>
  local got=0; "$RB" autonomy explain "$3" --profile "$2" >"$T/e" 2>&1 || got=$?
  [ "$got" = "$1" ] || { echo "    '$3' under $2: exit $got, wanted $1"; cat "$T/e"; exit 1; }
}
explain 0 trusted "cargo test --lib"
explain 0 trusted "git commit -m done"
explain 10 balanced "git commit -m done"
explain 10 isolated "git status && rm -rf /"
explain 10 isolated "ls \$(cat list)"
explain 10 isolated "FOO=1 cargo test"
explain 11 isolated "git push --force origin master"
explain 11 isolated "git reset --hard"
explain 11 isolated "rm -rf target"
explain 11 conservative "sudo true"

# ---------------------------------------------------------------- 6. friction: this checkout's refusals, no payload
for p in "$R" "$REAL"; do
  d="$H/.claude/projects/$(printf '%s' "$p" | tr -c 'A-Za-z0-9-' '-')"
  mkdir -p "$d"
  {
    printf '{"timestamp":"2099-01-01T00:00:00Z","message":{"content":[{"type":"tool_use","id":"a","name":"Bash","input":{"command":"case-private-command"}}]}}\n'
    printf '{"timestamp":"2099-01-01T00:00:01Z","message":{"content":[{"type":"tool_result","tool_use_id":"a","is_error":true,"content":"Permission for this action was denied by the Claude Code auto mode classifier. Reason: [Self-Modification]."}]}}\n'
  } > "$d/s.jsonl"
done
"$RB" autonomy friction --since-days 1 --format json > "$T/f.json"
jq -e '.tool_calls == 1 and .by_kind.classifier_denial == 1 and .by_reason["Self-Modification"] == 1' "$T/f.json" >/dev/null \
  || { echo "    friction did not count the denial once"; cat "$T/f.json"; exit 1; }
grep -q case-private-command "$T/f.json" && { echo "    a command left a transcript"; exit 1; }
expect_exit 0 "$RB" autonomy friction --since-days 1
c=0; "$RB" autonomy friction --since-days 0 >/dev/null 2>&1 || c=$?
[ "$c" != 0 ] || { echo "    a zero-day window was not refused"; exit 1; }
echo "    profile, explain and friction answered from the facts"
