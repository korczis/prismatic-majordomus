# majordomus-covers: none
# claims: observatory-draws-nothing-it-did-not-read
# The Observatory of majordomus ui (ADR 0158), through a real server over a fixture
# `majordomus init` installs, and through the snapshot the public site publishes:
#
#   /ui/observatory   the one screen: it answers, and carries its whole projection as
#                     `observatory/v1` in data-observatory: the sources it asked with the
#                     state each answered in, and eight scenes
#   /ui/kiosk         the same screen and the same projection as a preset, not a second
#                     application
#   /ui/screensaver   a permanent redirect to the kiosk; /ui/obsrvatory one to the Observatory
#   a scene           is available with a model, or unavailable with no model and the
#                     reason its source gave; a scene whose source failed is never
#                     available, so nothing is drawn that was not read
#   no mesh           the fixture runs none, so the three scenes that need one are
#                     unavailable and say so, on the server and in the snapshot alike
#   without script    one row per scene, with its state in words
#   the snapshot      holds the page and the script it loads
#   every name        the two names the server redirects are pages of the snapshot that lead on
#   a recording       where no mesh runs, a mesh scene replays the recording the repository
#                     carries and says so; a broken recording is a failed source, never drawn
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || skip "jq absent"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

SCENES="mesh activity intents tokens heartbeat delivery coop ambient"
MESH_SCENES="mesh activity coop"

# projection <page.html> <out.json>: the value of data-observatory, unescaped
projection() {
  grep -oE 'data-observatory="[^"]*"' "$1" | head -1 \
    | sed -e 's/^data-observatory="//' -e 's/"$//' \
          -e 's/&quot;/"/g' -e 's/&#34;/"/g' -e "s/&#39;/'/g" -e "s/&#x27;/'/g" \
          -e 's/&lt;/</g' -e 's/&gt;/>/g' -e 's/&amp;/\&/g' > "$2"
  [ -s "$2" ] || { echo "    $1 carries no data-observatory"; return 1; }
  jq -e . "$2" >/dev/null 2>&1 || { echo "    data-observatory of $1 is not JSON:"; head -c 300 "$2"; echo; return 1; }
}

# held <page.html> <projection.json> <what>: the contract, on one page
held() {
  page="$1"; p="$2"; what="$3"
  jq -e '.schema == "observatory/v1"' "$p" >/dev/null \
    || { echo "    $what: the projection's schema is $(jq -c .schema "$p"), expected observatory/v1"; return 1; }
  jq -e '(.observed_at | type == "string" and length > 0)' "$p" >/dev/null \
    || { echo "    $what: the projection does not say when it was observed"; return 1; }
  got="$(jq -r '[.scenes[].id] | join(" ")' "$p")"
  [ "$got" = "$SCENES" ] || { echo "    $what: the scenes are '$got', expected '$SCENES'"; return 1; }
  jq -e '(.sources | length > 0) and all(.sources[]; (.capability | type == "string" and length > 0)
      and (.state == "answered" or .state == "failed"))' "$p" >/dev/null \
    || { echo "    $what: a source is unnamed or in a state that is neither answered nor failed:"; jq -c '.sources' "$p"; return 1; }
  jq -e 'all(.sources[] | select(.state == "failed"); (.reason | type == "string" and length > 0))' "$p" >/dev/null \
    || { echo "    $what: a source failed without a reason:"; jq -c '.sources[] | select(.state == "failed")' "$p"; return 1; }
  # a scene says what it means and where its facts are rows
  jq -e 'all(.scenes[]; (.meaning | type == "string" and length > 0) and (.href | type == "string" and length > 0)
      and (.character == "semantic" or .character == "ambient"))' "$p" >/dev/null \
    || { echo "    $what: a scene lacks its meaning, its link to the rows, or its character"; return 1; }
  # available with a model, or unavailable with none and a reason: never an empty picture
  jq -e 'all(.scenes[]; if .available then .model != null
      else (.model == null and (.reason | type == "string" and length > 0)) end)' "$p" >/dev/null \
    || { echo "    $what: a scene is available without a model, or unavailable without a reason:"
         jq -c '.scenes[] | {id, available, reason, model: (.model != null)}' "$p"; return 1; }
  # nothing is drawn that was not read: a semantic scene one of whose sources failed is not available
  bad="$(jq -r '([.sources[] | select(.state == "failed") | .capability]) as $f
      | [.scenes[] | select(.available and .character == "semantic")
         | select(any(.requires[]; . as $r | $f | index($r) != null)) | .id] | join(" ")' "$p")"
  [ -z "$bad" ] || { echo "    $what: available although a source it requires failed: $bad"; return 1; }
  # this fixture runs no mesh
  for id in $MESH_SCENES; do
    jq -e --arg id "$id" '.scenes[] | select(.id == $id) | (.available == false) and (.model == null)
        and (.reason | length > 0)' "$p" >/dev/null \
      || { echo "    $what: scene $id is drawn in a repository that runs no mesh:"; jq -c --arg id "$id" '.scenes[] | select(.id == $id) | del(.model)' "$p"; return 1; }
  done
  # without script: one row per scene
  for id in $SCENES; do
    grep -qF "data-scene=\"$id\"" "$page" || { echo "    $what: no row for scene $id without script"; return 1; }
  done
  n="$(grep -oE 'data-scene="[a-z]+"' "$page" | LC_ALL=C sort -u | wc -l | tr -d ' ')"
  [ "$n" = 8 ] || { echo "    $what: $n distinct scene rows, expected 8"; return 1; }
}

# ---------------------------------------------------------------- the server
SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
code() { curl -s --max-time 120 -o "$2" -w '%{http_code}' "$U$1"; }

[ "$(code /ui/observatory obs.html)" = 200 ] || { echo "    /ui/observatory did not answer 200"; exit 1; }
projection obs.html obs.json || exit 1
held obs.html obs.json "/ui/observatory" || exit 1
grep -qE 'observatory\.js' obs.html || { echo "    /ui/observatory loads no observatory.js"; exit 1; }

[ "$(code /ui/kiosk kiosk.html)" = 200 ] || { echo "    /ui/kiosk did not answer 200"; exit 1; }
projection kiosk.html kiosk.json || exit 1
held kiosk.html kiosk.json "/ui/kiosk" || exit 1
grep -qF 'data-observatory-mode="kiosk"' kiosk.html || { echo "    /ui/kiosk is not the kiosk preset of the screen"; exit 1; }
# one screen, one inventory: the preset shows the scenes the screen shows, in its order
[ "$(jq -c '[.scenes[] | [.id, .available]]' kiosk.json)" = "$(jq -c '[.scenes[] | [.id, .available]]' obs.json)" ] \
  || { echo "    /ui/kiosk and /ui/observatory disagree about the scenes"; exit 1; }

out="$(curl -s --max-time 120 -o /dev/null -w '%{http_code} %{redirect_url}' "$U/ui/screensaver")"
case "$out" in
  "308 "*/ui/kiosk) ;;
  *) echo "    /ui/screensaver answered '$out', expected 308 to /ui/kiosk"; exit 1 ;;
esac
out="$(curl -s --max-time 120 -o /dev/null -w '%{http_code} %{redirect_url}' "$U/ui/obsrvatory")"
case "$out" in
  "308 "*/ui/observatory) ;;
  *) echo "    /ui/obsrvatory answered '$out', expected 308 to /ui/observatory"; exit 1 ;;
esac

# the navigation of another screen leads to it
[ "$(code /ui/knowledge nav.html)" = 200 ] || { echo "    /ui/knowledge did not answer 200"; exit 1; }
grep -qE 'href="[^"]*observatory/?"' nav.html || { echo "    the navigation does not lead to the Observatory"; exit 1; }
serve_down; SRV=""

# ---------------------------------------------------------------- the snapshot
S="$PWD/target/web/ui-snapshot"
"$RB" web report ui-snapshot --limit 40 >/dev/null 2> err.txt \
  || { echo "    the snapshot was not written:"; tail -5 err.txt | sed 's/^/    | /'; exit 1; }
# the page is found by its path inside the snapshot: a temporary directory whose own name
# holds the word would otherwise make every page of the snapshot match
SP="$(cd "$S" && find . -path '*observatory*' -name '*.html' | LC_ALL=C sort | head -1)"
[ -n "$SP" ] || { echo "    the snapshot holds no observatory page"; exit 1; }
SP="$S/${SP#./}"
projection "$SP" snap.json || exit 1
held "$SP" snap.json "the snapshot's observatory" || exit 1
[ -n "$(find "$S" -name 'observatory.js' | head -1)" ] || { echo "    the snapshot does not hold observatory.js"; exit 1; }
# a file cannot redirect: each name the server redirects is a page of the publication that
# sends its reader to the page the server would, without a script
for pair in "screensaver kiosk" "obsrvatory observatory"; do
  old="${pair% *}"; to="${pair#* }"
  [ -f "$S/$to/index.html" ] || { echo "    the snapshot holds no $to page for $old to lead to"; exit 1; }
  [ -f "$S/$old/index.html" ] || { echo "    the snapshot holds no page for /ui/$old, which the server redirects to /ui/$to"; exit 1; }
  grep -qF "<meta http-equiv=\"refresh\" content=\"0; url=../$to/index.html\">" "$S/$old/index.html" \
    || { echo "    $old/index.html does not send its reader to ../$to/index.html:"; head -c 400 "$S/$old/index.html"; echo; exit 1; }
  grep -qF "href=\"../$to/index.html\"" "$S/$old/index.html" || { echo "    $old/index.html carries no link for a reader whose browser does not follow the refresh"; exit 1; }
  grep -q '<script' "$S/$old/index.html" && { echo "    $old/index.html carries a script"; exit 1; }
  jq -e --arg n "/ui/$old" --arg f "$old/index.html" '.redirects[$n] == $f' "$S/snapshot.json" >/dev/null \
    || { echo "    snapshot.json does not record the page for /ui/$old"; exit 1; }
done

# ---------------------------------------------------------------- a recording
# Where no mesh runs, a scene of the mesh draws the recording the repository carries, says it
# is a replay and when it was recorded, and a scene the recording does not carry stays
# unavailable. The recording here is a test fixture; a repository's real one is captured by
# observatory.capture from a server that runs the mesh.
mkdir -p .ai/repo/observatory
cat > .ai/repo/observatory/recording.json <<'JSON'
{"schema": "observatory-recording/v1", "recorded_at": "2026-10-14T05:00:00Z", "revision": "abc123",
 "scenes": {"mesh": {"known": 1, "refused": 0,
   "machines": [{"id": "m1", "label": "machine 1", "local": true,
                 "runtimes": [{"id": "m1-r1", "liveness": "live", "this": true, "sessions": 1}]}]}},
 "manifest": {"sources": ["mesh.peers"], "removed": ["identities, replaced by ordinals"]}}
JSON
"$RB" web report ui-snapshot --limit 40 >/dev/null 2> err.txt \
  || { echo "    the snapshot with a recording was not written:"; tail -5 err.txt | sed 's/^/    | /'; exit 1; }
projection "$SP" rec.json || exit 1
jq -e '.scenes[] | select(.id == "mesh") | .available and .model.machines[0].label == "machine 1"
    and .replay.recorded_at == "2026-10-14T05:00:00Z" and .replay.revision == "abc123"
    and .replay.sources == ["mesh.peers"]' rec.json >/dev/null \
  || { echo "    the mesh scene does not replay the recording, labelled with when it was recorded:"; jq -c '.scenes[] | select(.id == "mesh")' rec.json; exit 1; }
for id in activity coop; do
  jq -e --arg id "$id" '.scenes[] | select(.id == $id) | (.available | not) and (.replay == null)' rec.json >/dev/null \
    || { echo "    scene $id is drawn although the recording does not carry it"; exit 1; }
done
jq -e '[.scenes[] | select(.id != "mesh") | .replay] | all(. == null)' rec.json >/dev/null \
  || { echo "    a scene other than the recorded one says it is a replay"; exit 1; }
# a file that is not a recording is an error the page states, never a scene drawn from it
printf 'not a recording\n' > .ai/repo/observatory/recording.json
"$RB" web report ui-snapshot --limit 40 >/dev/null 2> err.txt || { echo "    the snapshot failed on a broken recording"; exit 1; }
projection "$SP" broken.json || exit 1
jq -e '(.sources[] | select(.capability == "observatory.replay") | .state == "failed" and (.reason | length > 0))
    and (.scenes[] | select(.id == "mesh") | .available | not)' broken.json >/dev/null \
  || { echo "    a broken recording was not reported as a failed source, or was drawn"; jq -c '.sources[] | select(.capability == "observatory.replay")' broken.json; exit 1; }
