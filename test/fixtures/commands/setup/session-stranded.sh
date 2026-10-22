# Installed, with two episodes open: one whose client is gone and one that is live.
#
# Both halves of the stranded episode's trail are aged, not only its open record. The last
# sign of life is the later of started_at and the newest ledger line the episode stamped, so
# an episode whose ledger is fresh is young however old its file claims to be — a fixture
# that moved one half would demonstrate the command passing for the wrong reason.
. "$FIXTURE_SETUP/installed.sh"
"$MJ" session start --provider-session gone-client >/dev/null
"$MJ" session start --provider-session this-client >/dev/null
_S=.ai/local/state
_dead="$(sed -n 's/^session_id: //p' "$_S/sessions-open/gone-client.yaml" | head -n 1)"
sed -i.bak 's/^started_at: .*/started_at: 2020-01-01T00:00:00Z/' "$_S/sessions-open/gone-client.yaml"
rm -f "$_S/sessions-open/gone-client.yaml.bak"
awk -v s="$_dead" '{ if (index($0, "\"session\":\"" s "\"") > 0) sub(/"ts":"[^"]*"/, "\"ts\":\"2020-01-01T00:00:00Z\""); print }' \
  "$_S/ledger.jsonl" > "$_S/ledger.mj-tmp" && mv "$_S/ledger.mj-tmp" "$_S/ledger.jsonl"
