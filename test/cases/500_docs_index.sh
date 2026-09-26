# majordomus-covers: none
# The documentation index is read off the built pages, and the documentation hub counts
# nothing of its own.
#
#   1. scripts/lib/docs-index.mjs takes a page's title from its <head> (not from an inline
#      SVG's <title>), strips the " — <site name>" suffix the page itself declares, decodes
#      entities, leaves out a redirect, joins an entity page to its object, sorts by route and
#      writes the same bytes twice;
#   2. `scripts/pages verify-docs` passes over the served site on its commit and refuses another
#      commit or a missing page;
#   3. docs-index.mjs refuses a build in which a route the catalogue publishes was not built
#      (exit 10), and an entities.json without a catalogue (exit 12);
#   4. the catalogue scripts/generate-site-data writes names every kind site/data/publication.toml
#      declares, and its count for each kind is the count `majordomus entity kinds` reports —
#      the hub's numbers are the index's numbers.
. "$ROOT/test/lib.sh"

command -v node >/dev/null 2>&1 || { echo "    node is required by the site build"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
GEN="$ROOT/scripts/lib/docs-index.mjs"
[ -f "$GEN" ] || { echo "    $GEN is missing"; exit 1; }

fail=0
note() { echo "    $1"; fail=1; }

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-500.XXXXXX")"
trap 'rm -rf "$S"' EXIT

page() { # route-dir title description [extra-head]
  mkdir -p "$S/public/$1"
  printf '<!doctype html><html><head><title>%s</title><meta name="description" content="%s">%s<meta property="og:site_name" content="Site &amp; Co"></head><body><svg><title>Not the title</title></svg></body></html>\n' \
    "$2" "$3" "${4:-}" > "$S/public/$1/index.html"
}
page "" "Site &amp; Co — the home page" "Home."
page "docs" "Documentation — Site &amp; Co" "Every document &amp; more."
page "adrs" "Decisions — Site &amp; Co" "Decisions."
page "adrs/adr-0001" "One decision — Site &amp; Co" "The first."
page "rules" "Rules — Site &amp; Co" "Rules."
page "old" "Moved — Site &amp; Co" "Moved." '<meta http-equiv="refresh" content="0; url=/docs/">'
printf '{"commit":"0123456789abcdef0123456789abcdef01234567","source_version":"9.9.9"}\n' > "$S/public/build.json"
cat > "$S/entities.json" <<'JSON'
{"schema":1,
 "kinds":[{"kind":"adr","route":"/adrs/"}],
 "catalogue":[{"kind":"adr","projection":"entity","route":"/adrs/","title":"Decisions","reason":null,"count":1},
              {"kind":"rule","projection":"section","route":"/rules/","title":"Rules","reason":null,"count":0},
              {"kind":"knowledge","projection":"none","route":null,"title":null,"reason":"not for visitors","count":4}],
 "entities":[{"kind":"adr","id":"majordomus://adr/adr-0001","slug":"adr-0001","source":".ai/repo/adrs/0001-x.md"}]}
JSON
printf '{"registry":{"summary":{"total":3,"cli_commands":2,"http_routes":1,"mcp_tools":1,"mcp_resources":2,"modules":1}}}\n' > "$S/registry.json"

# ---------------------------------------------------------------- 1. what it reads, and determinism
if ! node "$GEN" "$S/public" "$S/entities.json" "$S/registry.json" >/dev/null 2>"$S/err"; then
  note "docs-index refused a complete fixture:"; sed 's/^/      /' "$S/err"
else
  IX="$S/public/docs/index.json"
  cp "$IX" "$S/first.json"
  node "$GEN" "$S/public" "$S/entities.json" "$S/registry.json" >/dev/null 2>&1 || note "a second run over one tree failed"
  # the second run also sees the first run's docs/index.json, which is not an index.html
  cmp -s "$IX" "$S/first.json" || note "two runs over one tree wrote different bytes"
  [ "$(jq -r '[.pages[].route] | join(" ")' "$IX")" = "/ /adrs/ /adrs/adr-0001/ /docs/ /rules/" ] \
    || note "pages are not the built documents sorted by route: $(jq -c '[.pages[].route]' "$IX")"
  [ "$(jq -r '.pages[] | select(.route == "/docs/") | .title' "$IX")" = "Documentation" ] \
    || note "the site-name suffix was not stripped, or the SVG title was read: $(jq -c '.pages[] | select(.route == "/docs/")' "$IX")"
  [ "$(jq -r '.pages[] | select(.route == "/") | .title' "$IX")" = "Site & Co — the home page" ] \
    || note "a title that is not '<page> — <site>' was cut: $(jq -r '.pages[] | select(.route == "/") | .title' "$IX")"
  [ "$(jq -r '.pages[] | select(.route == "/docs/") | .description' "$IX")" = "Every document & more." ] \
    || note "the description was not decoded"
  jq -e '.pages[] | select(.route == "/adrs/adr-0001/") | .kind == "adr" and .id == "majordomus://adr/adr-0001" and .source == ".ai/repo/adrs/0001-x.md"' "$IX" >/dev/null \
    || note "an entity page is not joined to its object"
  jq -e 'any(.pages[]; .route == "/old/") | not' "$IX" >/dev/null || note "a redirect page was indexed as a document"
  jq -e '.build.commit == "0123456789abcdef0123456789abcdef01234567" and .build.source_version == "9.9.9"' "$IX" >/dev/null \
    || note "the index does not carry the identity of the build it was written by"
  jq -e '.counts.capabilities == 3 and .counts.mcp_resources == 2 and .counts.pages == 5' "$IX" >/dev/null \
    || note "the counts are not the registry summary's: $(jq -c .counts "$IX")"
  jq -e '[.kinds[].kind] == ["adr","rule","knowledge"]' "$IX" >/dev/null || note "the kinds are not the catalogue"
fi

# ---------------------------------------------------------------- 2. verify-docs over the served fixture
# the same public probe the Pages workflow runs after deploying, against the fixture served
# over HTTP: it passes on the commit the index names, and refuses another commit and a
# published entity page that answers 404
if command -v python3 >/dev/null 2>&1; then
  sed -i.bak 's|<body>|<body><div data-docs-search></div>|' "$S/public/docs/index.html" && rm -f "$S/public/docs/index.html.bak"
  printf '{}\n' > "$S/public/openapi.json"
  PORT=$(( 20000 + $$ % 20000 ))
  ( cd "$S/public" && exec python3 -m http.server "$PORT" --bind 127.0.0.1 ) >/dev/null 2>&1 &
  SRV=$!
  trap 'kill "$SRV" 2>/dev/null || true; rm -rf "$S"' EXIT
  for _ in 1 2 3 4 5 6 7 8 9 10; do curl -fs "http://127.0.0.1:$PORT/build.json" >/dev/null 2>&1 && break; sleep 0.3; done
  V="http://127.0.0.1:$PORT"
  bash "$ROOT/scripts/pages" verify-docs --url "$V" --commit 0123456789ab --quiet >"$S/v.out" 2>&1 \
    || { note "verify-docs refused a complete fixture:"; sed 's/^/      /' "$S/v.out"; }
  rc=0; bash "$ROOT/scripts/pages" verify-docs --url "$V" --commit fedcba9876543 --quiet >/dev/null 2>&1 || rc=$?
  [ "$rc" = 10 ] || note "verify-docs over an index of another commit exited $rc, not 10"
  mv "$S/public/adrs/adr-0001" "$S/moved"
  rc=0; bash "$ROOT/scripts/pages" verify-docs --url "$V" --commit 0123456789ab --quiet >"$S/v.out" 2>&1 || rc=$?
  [ "$rc" = 10 ] || note "verify-docs with an entity page answering 404 exited $rc, not 10"
  grep -q '/adrs/adr-0001/ answered 404' "$S/v.out" || note "verify-docs does not name the route that failed: $(cat "$S/v.out")"
  mv "$S/moved" "$S/public/adrs/adr-0001"
  kill "$SRV" 2>/dev/null || true
else
  note "python3 is required to serve the fixture"
fi

# ---------------------------------------------------------------- 3. what it refuses
rm -rf "$S/public/adrs/adr-0001"
rc=0; node "$GEN" "$S/public" "$S/entities.json" "$S/registry.json" >/dev/null 2>"$S/err" || rc=$?
[ "$rc" = 10 ] || note "a published entity page that was not built exited $rc, not 10"
grep -q '/adrs/adr-0001/' "$S/err" || note "the refusal does not name the missing route"
jq 'del(.catalogue)' "$S/entities.json" > "$S/nocat.json"
rc=0; node "$GEN" "$S/public" "$S/nocat.json" "$S/registry.json" >/dev/null 2>&1 || rc=$?
[ "$rc" = 12 ] || note "an entities.json without a catalogue exited $rc, not 12"

# ---------------------------------------------------------------- 4. the catalogue is the index's
ENT="$ROOT/site/data/generated/entities.json"
jq -e '.catalogue | length > 0' "$ENT" >/dev/null 2>&1 || { note "$ENT carries no catalogue"; exit 1; }
for k in $(sed -n 's/^kind = "\(.*\)"$/\1/p' "$ROOT/site/data/publication.toml"); do
  jq -e --arg k "$k" 'any(.catalogue[]; .kind == $k)' "$ENT" >/dev/null || note "the catalogue does not name declared kind '$k'"
done
MJ="$(rust_bin)" || rust_bin_exit $?
KINDS="$(env -u MAJORDOMUS_SHARE "$MJ" entity kinds --repo "$ROOT" --format json 2>/dev/null || true)"
[ -n "$KINDS" ] || { note "majordomus entity kinds did not answer"; exit 1; }
printf '%s' "$KINDS" | jq -r '.kinds[] | "\(.kind) \(.count)"' > "$S/index-counts"
jq -r '.catalogue[] | select(.count > 0) | "\(.kind) \(.count)"' "$ENT" | LC_ALL=C sort > "$S/cat-counts"
LC_ALL=C sort "$S/index-counts" | grep -v ' 0$' > "$S/idx-sorted" || true
cmp -s "$S/idx-sorted" "$S/cat-counts" || {
  note "the catalogue's counts are not the index's:"
  diff "$S/idx-sorted" "$S/cat-counts" | sed 's/^/      /' | head -10; }

exit "$fail"
