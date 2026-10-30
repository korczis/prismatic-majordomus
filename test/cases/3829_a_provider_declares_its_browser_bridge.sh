# majordomus-covers: none
# A provider declares whether its client drives a browser beside this repository's server,
# and preflight says when this machine has no bridge for it (I3829,
# project.a-provider-capability-cites-its-evidence@2).
#
# On 2026-10-21 the one path that drives a page and the live mesh in one session — Claude
# Code with the Claude in Chrome bridge — was declared nowhere, and a session learned the
# bridge was unconnected only by calling a browser tool and reading the error. Asserted here:
#
#   1. the evidence gate passes the distribution's declaration, and refuses a copy whose bridge
#      names no host, and a copy whose browser word is outside the vocabulary
#   2. preflight in a checkout configured for Claude Code (.mcp.json) reports the bridge
#      unavailable on a machine without its native messaging host, with the person's step,
#      and active once the host is there; a checkout configured for nothing hears nothing
#
# The in-process half — the declaration, the refusal of an unknown word, every verdict and
# the providers document's column — is apps/majordomus-cli/tests/provider_browser.rs.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    jq is required by this case"; exit 1; }
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3829.XXXXXX")"
trap 'rm -rf "$S"' EXIT
HOST=com.anthropic.claude_code_browser_extension

# ---------------------------------------------------------------- 1. the gate and its negatives
expect_exit 0 "$ROOT/scripts/provider-evidence-check" || exit 1
# Over a COPY of the distribution, never the repository's own declaration (case 200's reason).
cp -R "$ROOT/share" "$S/share"
grep -v '^      browser_host: ' "$ROOT/share/providers.yaml" > "$S/share/providers.yaml"
rc=0; MAJORDOMUS_SHARE="$S/share" "$ROOT/scripts/provider-evidence-check" >"$S/gate.out" 2>&1 || rc=$?
must "the gate passed a bridge with no host (exit $rc)" [ "$rc" = 10 ]
grep -qF 'no offers.browser_host' "$S/gate.out" \
  || { echo "    the gate failed for another reason than the missing host:"; sed 's/^/    | /' "$S/gate.out"; exit 1; }
sed 's/^      browser: "bridge"$/      browser: "extension"/' "$ROOT/share/providers.yaml" > "$S/share/providers.yaml"
rc=0; MAJORDOMUS_SHARE="$S/share" "$ROOT/scripts/provider-evidence-check" >"$S/gate.out" 2>&1 || rc=$?
must "the gate passed a browser word outside the vocabulary (exit $rc)" [ "$rc" = 10 ]
grep -qF "offers.browser is 'extension', which is outside the vocabulary" "$S/gate.out" \
  || { echo "    the gate failed for another reason than the word:"; sed 's/^/    | /' "$S/gate.out"; exit 1; }

# ---------------------------------------------------------------- 2. preflight on a machine
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null 2>&1; git commit -qm base >/dev/null 2>&1 || true
T="$PWD"
mkdir -p "$S/home"
browser() {   # the integration.browser check, or "absent"
  HOME="$S/home" MAJORDOMUS_SHARE="$ROOT/share" "$RB" env preflight --repo "$T" --format json \
    | jq -c '[.sections[].checks[] | select(.id == "integration.browser")] | .[0] // "absent"'
}
rm -f "$T/.mcp.json"
must "a checkout configured for no provider was told about a browser: $(browser)" \
  [ "$(browser)" = '"absent"' ]

printf '{"mcpServers":{}}\n' > "$T/.mcp.json"
got="$(browser)"
must "no bridge host on this machine, and the check is not unavailable: $got" \
  [ "$(jq -r .verdict <<<"$got")" = unavailable ]
must "the check does not name the person's step: $got" \
  grep -qF '/chrome' <<<"$(jq -r .next <<<"$got")"

mkdir -p "$S/home/.config/google-chrome/NativeMessagingHosts"
printf '{}\n' > "$S/home/.config/google-chrome/NativeMessagingHosts/$HOST.json"
got="$(browser)"
must "the bridge host is installed, and the check is not active: $got" \
  [ "$(jq -r .verdict <<<"$got")" = active ]
must "the active check does not cite the host's manifest: $got" \
  grep -qF "$HOST.json" <<<"$(jq -r '.evidence[].source' <<<"$got")"
