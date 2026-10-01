# majordomus-covers: none
# majordomus-timeout: 900
# The Cockpit says only what it knows (UI-DERIVED-STATE): over a real socket, every fact a
# page shows is what its capability answered, and a capability that failed or answered
# nothing is rendered unknown — never ok, never healthy, never a count of 0.
#
# The repository and the installation are built so that each page has something it cannot
# know or something bad to count, because a page that renders the happy path proves only
# that the happy path renders:
#   * an installation with no distribution model, so `distribution.status` does not answer;
#   * a generated tree whose one file carries no recorded hash, so `artifacts.list` cannot
#     say it is current;
#   * a registered worktree whose directory is gone, so the topology has a missing row;
#   * a model catalogue that declares no status for its models;
#   * an MCP client that attaches and announces nothing, on the server's own board.
# The failing `repository.info` of the navigation and the unknown upstream distance of a
# worktree cannot be forced from outside a process; their unit tests hold them
# (`cockpit::nav`, `cockpit::pages`).
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || skip "no curl"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj540.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT

# ---------------------------------------------------------------- the installation
cp -R "$ROOT/share" "$S/share"
rm -f "$S/share/distribution.yaml"
# a catalogue whose models declare no status: stripped here rather than assumed of the
# shipped one, so the case holds whatever that one comes to declare
sed -i.bak '/^    status:/d' "$S/share/models.yaml"
grep -q '^  - id:' "$S/share/models.yaml" || skip "the shipped model catalogue declares no model"
MAJORDOMUS_SHARE="$S/share"; export MAJORDOMUS_SHARE

# ---------------------------------------------------------------- the repository
"$MJ" init >/dev/null
# a manifest whose only file is the manifest itself, which cannot hash itself
mkdir -p docs/generated
jq '(.artifacts | map(select(.path == "docs/generated/artifacts.json"))) as $a
    | {schema, documents: [.documents[] | select(.id == $a[0].document)], artifacts: $a}' \
  "$ROOT/docs/generated/artifacts.json" > "$S/manifest.json"
mv "$S/manifest.json" docs/generated/artifacts.json
jq -e '.artifacts | length == 1 and (.[0].sha256 == null)' docs/generated/artifacts.json >/dev/null \
  || { echo "    the fixture manifest is not one unhashed file"; cat docs/generated/artifacts.json; exit 1; }
git add -A >/dev/null && git commit -qm layer
# a worktree registered and then removed behind git's back
git worktree add -q "$S/gone" -b gone
rm -rf "$S/gone"

serve_up "$S/out.txt" "$S/err.txt" || exit 1
# Every assertion below is counted rather than fatal, so that one run names every page that
# says more than it knows instead of only the first.
fails=0
claim() { "$@" || fails=$((fails + 1)); }
page() { curl -s -m 60 "$U$1" > "$S/page.html"; LAST_OUT="$(cat "$S/page.html")"; }
# the value of the statistic labelled $1 on the last page
stat_of() {
  grep -oE "mj-stat-value\">[^<]*</span><span class=\"mj-stat-label\">$1<" "$S/page.html" \
    | head -1 | sed -E 's/mj-stat-value">([^<]*)<.*/\1/'
}

# ---------------------------------------------------------------- Overview: Distribution
# the card is there and says the capability did not answer, rather than vanishing
page /cockpit
claim expect_grep 'distribution.status did not answer'

# ---------------------------------------------------------------- Artifacts
page /cockpit/artifacts
claim expect_grep 'unverified: 1 file\(s\) carry no recorded hash'
# every tally is on the strip, the unhashed one included
[ "$(stat_of present)" = 1 ] || { echo "    no 'present' statistic of 1 on the Artifacts page"; fails=$((fails + 1)); }
# and the verdict comes from the capability, which says the same
curl -s -m 30 "$U/api/v1/artifacts" > "$S/artifacts.json"
jq -e '.verdict == "unverified"' "$S/artifacts.json" >/dev/null \
  || { echo "    artifacts.list does not decide 'unverified'"; jq -c '{verdict, tallies}' "$S/artifacts.json"; fails=$((fails + 1)); }

# ---------------------------------------------------------------- Worktrees
page /cockpit/worktrees
[ "$(stat_of missing)" = 1 ] || { echo "    the missing worktree is not counted: '$(stat_of missing)'"; fails=$((fails + 1)); }
for f in locked errors warnings; do
  [ -n "$(stat_of "$f")" ] || { echo "    the Worktrees strip leaves out '$f'"; fails=$((fails + 1)); }
done

# ---------------------------------------------------------------- Commands
page /cockpit/commands
chips="$(grep -oE 'mj-chip-count">[0-9]+<' "$S/page.html" | grep -oE '[0-9]+')"
[ "$(printf '%s\n' "$chips" | grep -c .)" -ge 4 ] \
  || { echo "    fewer chips than two facets of two: $chips"; fails=$((fails + 1)); }
if printf '%s\n' "$chips" | grep -qx 0; then
  echo "    a command chip counts 0, and a chip exists only for what is present: $chips"
  fails=$((fails + 1))
fi
# and every chip is a link the page answers: a chip built from a word the filter does not
# take is a link to a 500
for href in $(grep -oE 'class="mj-chip[^"]*" href="[^"]*"' "$S/page.html" | sed -E 's/.*href="([^"]*)"/\1/' | sed 's/&amp;/\&/g'); do
  code="$(curl -s -m 60 -o /dev/null -w '%{http_code}' "$U$href")"
  [ "$code" = 200 ] || { echo "    the chip $href answers $code"; fails=$((fails + 1)); }
done

# ---------------------------------------------------------------- Models
page /cockpit/models
claim expect_grep '>undeclared<'
claim expect_no_grep '>available</span>'

# ---------------------------------------------------------------- Health: attached clients
# an MCP client attaches over HTTP and announces nothing: the board now understates who is
# here, and the check says so rather than "ok"
curl -s -m 30 -X POST -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case540","version":"1"}}}' \
  "$U/mcp" > /dev/null
# the report is cached for five seconds; ask past that
sleep 6
curl -s -m 30 "$U/api/v1/health" > "$S/health.json"
jq -e '.checks[] | select(.id == "peers") | .status != "ok"' "$S/health.json" >/dev/null \
  || { echo "    an unannounced peer leaves Attached clients ok"; jq -c '.checks[] | select(.id == "peers")' "$S/health.json"; fails=$((fails + 1)); }
jq -e '.checks[] | select(.id == "peers") | .detail | test("^1 attached \\(0 announced\\)")' "$S/health.json" >/dev/null \
  || { echo "    the attached count is not the live peer"; jq -c '.checks[] | select(.id == "peers")' "$S/health.json"; fails=$((fails + 1)); }
page /cockpit/health
claim expect_grep '1 attached \(0 announced\)'
serve_down
[ "$fails" = 0 ] || { echo "    $fails assertion(s) failed"; exit 1; }
