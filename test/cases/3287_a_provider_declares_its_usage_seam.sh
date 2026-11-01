# majordomus-covers: none
# A provider declares the official seams through which it reports what a session consumed,
# with the citation they were read from (I3287, ADR 0213,
# project.a-provider-capability-cites-its-evidence@3).
#
# Usage is observed only where the provider itself reports it, so before anything can receive
# a provider's usage the distribution has to say which seam that provider offers: its own
# OpenTelemetry export (otel), its app server's notification (app-server), the usage a
# non-interactive run prints (stream-json), or none. Asserted here:
#
#   1. the evidence gate passes the distribution's declaration, and refuses a copy with a
#      usage word outside the vocabulary, a copy whose row declares none beside a seam, and a
#      copy whose usage citation is gone
#   2. product validate refuses a row that declares usage and cites nothing
#   3. lifecycle providers and the generated providers document carry each row's seams, as
#      declared: Claude Code otel and stream-json, Codex app-server, Gemini otel, and none for
#      agents, bb and generic
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    jq is required by this case"; exit 1; }
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3287.XXXXXX")"
trap 'rm -rf "$S"' EXIT

# ---------------------------------------------------------------- 1. the gate and its negatives
expect_exit 0 "$ROOT/scripts/provider-evidence-check" || exit 1
# Over a COPY of the distribution, never the repository's own declaration (case 200's reason).
cp -R "$ROOT/share" "$S/share"
refused() {   # refused <what the finding says> — the gate over the copy exits 10 saying it
  local rc=0
  MAJORDOMUS_SHARE="$S/share" "$ROOT/scripts/provider-evidence-check" >"$S/gate.out" 2>&1 || rc=$?
  must "the gate passed a copy it should refuse (exit $rc), wanting: $1" [ "$rc" = 10 ]
  grep -qF "$1" "$S/gate.out" \
    || { echo "    the gate failed for another reason than: $1"; sed 's/^/    | /' "$S/gate.out"; exit 1; }
}
sed 's/^      usage: \["app-server"\]$/      usage: ["transcript"]/' "$ROOT/share/providers.yaml" > "$S/share/providers.yaml"
refused "offers.usage names 'transcript', which is outside the vocabulary"
sed 's/^      usage: \["otel"\]$/      usage: ["otel", "none"]/' "$ROOT/share/providers.yaml" > "$S/share/providers.yaml"
refused "offers.usage names none beside a seam"
grep -v '^      usage_evidence: "https://github.com/google-gemini' "$ROOT/share/providers.yaml" > "$S/share/providers.yaml"
refused "offers.usage_evidence is empty"
echo "    ok: the gate passes the declaration and refuses an unknown seam, none beside a seam, and an uncited list"

# ---------------------------------------------------------------- 2. product validate
rc=0; MAJORDOMUS_SHARE="$S/share" "$RB" product validate --repo "$ROOT" --format json > "$S/validate.json" 2>/dev/null || rc=$?
must "product validate passed a usage row with no citation (exit $rc)" [ "$rc" != 0 ]
jq -e '[.. | objects | select(.code? == "provider_usage_uncited" and .id == "gemini")] | length == 1' "$S/validate.json" >/dev/null \
  || { echo "    product validate did not name gemini's uncited usage"; jq -c '[.. | objects | select(.code? != null) | {code, id}]' "$S/validate.json" | head -c 600; exit 1; }
echo "    ok: product validate refuses a usage row with no citation, naming the provider"

# ---------------------------------------------------------------- 3. the surfaces
MAJORDOMUS_SHARE="$ROOT/share" "$RB" run lifecycle.providers --repo "$ROOT" --format json 2>/dev/null \
  | jq -c '.output.providers | map({key: .id, value: .usage}) | from_entries' > "$S/usage.json"
want='{"agents":["none"],"bb":["none"],"claude-code":["otel","stream-json"],"codex":["app-server"],"gemini":["otel"],"generic":["none"]}'
must "lifecycle providers does not carry each row's usage: $(cat "$S/usage.json")" \
  [ "$(jq -cS . "$S/usage.json")" = "$(jq -cS . <<<"$want")" ]
grep -qF '| `claude-code` |' "$ROOT/docs/generated/providers.md" \
  && grep -F '| `claude-code` |' "$ROOT/docs/generated/providers.md" | grep -qF '`otel`, `stream-json`' \
  || { echo "    docs/generated/providers.md does not show Claude Code's usage seams"; exit 1; }
echo "    ok: lifecycle providers and the providers document carry each provider's usage seams as declared"
