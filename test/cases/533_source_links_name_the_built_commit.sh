# majordomus-covers: none
# A page's link into its own source names the commit the site was built from, not master.
#
#   1. site/templates/source-links.html is the one place a source link is made, and a converted
#      template goes through it: rendered by zola in a fixture site, the real line of a converted
#      template (entity.html's canonical-file link) carries the commit data/build.json records,
#      and falls back to master when the build is dirty or there is no build.json at all;
#   2. the ratchet refuses one added blob/master under site/templates (exit 10) and passes the
#      tree as it is;
#   3. the pin is load-bearing: the same rendering over a copy of source-links.html with the
#      pin reverted to master is refused by the check of step 1, and the real file is left
#      byte-identical.
#
# The commit a built page names must be one scripts/ci/link-check can resolve against git; it is
# HEAD at build time, which `scripts/pages build && scripts/pages check` decide over the real site.
. "$ROOT/test/lib.sh"

SRC="$ROOT/site/templates/source-links.html"
CHECK="$ROOT/scripts/ci/source-pin-check"
BASE="$ROOT/.ai/repo/source-pin-baseline.txt"
[ -f "$SRC" ] || { echo "    $SRC is missing"; exit 1; }
[ -x "$CHECK" ] || { echo "    $CHECK is missing or not executable"; exit 1; }
[ -f "$BASE" ] || { echo "    $BASE is missing"; exit 1; }

# ---------------------------------------------------------------- 1. the one place, used
for c in source_ref source_blob source_tree; do
  grep -q "{% component $c(" "$SRC" || { echo "    source-links.html does not define the component $c"; exit 1; }
done
LINE="$(grep -E 'canonical file.*<source_blob path=\{e\.source\} />' "$ROOT/site/templates/entity.html" || true)"
[ -n "$LINE" ] || { echo "    entity.html's canonical-file link does not go through <source_blob>"; exit 1; }

REPO_URL="$(jq -r .repository_url "$ROOT/site/data/generated/project.json")"
COMMIT="$(git -C "$ROOT" rev-parse HEAD)"

# render <components-file> <build.json content or ""> -> the href of the canonical-file link
render() {
  local site; site="$(mktemp -d "$T/site.XXXXXX")"
  mkdir -p "$site/templates" "$site/content" "$site/data/generated"
  cp "$1" "$site/templates/source-links.html"
  printf 'base_url = "https://fixture.test"\n' > "$site/config.toml"
  printf '{"repository_url": "%s"}\n' "$REPO_URL" > "$site/data/generated/project.json"
  [ -z "$2" ] || printf '%s\n' "$2" > "$site/data/build.json"
  { printf '{%%- set e = {"source": "docs/ARCHITECTURE.md"} -%%}\n'; printf '%s\n' "$LINE"; } > "$site/templates/index.html"
  ( cd "$site" && zola build >/dev/null 2>&1 ) || { echo "zola-failed"; return 0; }
  grep -oE 'href="[^"]*docs/ARCHITECTURE\.md"' "$site/public/index.html" | head -1 | sed 's/^href="//; s/"$//'
}

# pinned <components-file>: 0 when the three builds name what they must
pinned() {
  local got
  got="$(render "$1" "{\"commit\": \"$COMMIT\", \"dirty\": false}")"
  [ "$got" = "$REPO_URL/blob/$COMMIT/docs/ARCHITECTURE.md" ] || { echo "    clean build: got '$got', want the commit $COMMIT"; return 1; }
  got="$(render "$1" "{\"commit\": \"$COMMIT\", \"dirty\": true}")"
  [ "$got" = "$REPO_URL/blob/master/docs/ARCHITECTURE.md" ] || { echo "    dirty build: got '$got', want master"; return 1; }
  got="$(render "$1" "")"
  [ "$got" = "$REPO_URL/blob/master/docs/ARCHITECTURE.md" ] || { echo "    no build.json: got '$got', want master"; return 1; }
  return 0
}

if command -v zola >/dev/null 2>&1; then
  pinned "$SRC" || { echo "    a converted page's source link does not name the built commit"; exit 1; }

  # ------------------------------------------------------------ 3. the mutation
  before="$(cksum < "$SRC")"
  MUT="$T/source-links.mutant.html"
  sed 's/{{ build\.commit }}/master/' "$SRC" > "$MUT"
  cmp -s "$SRC" "$MUT" && { echo "    the mutation did not change source-links.html"; exit 1; }
  rc=0; pinned "$MUT" >/dev/null || rc=$?
  [ "$rc" != 0 ] || { echo "    the check passed with the pin reverted to master; it proves nothing"; exit 1; }
  [ "$(cksum < "$SRC")" = "$before" ] || { echo "    the mutation touched the real source-links.html"; exit 1; }
elif [ "${CI:-}" = true ]; then
  echo "    zola is absent on CI, where the site cases must run rather than skip"; exit 1
else
  echo "    zola absent; the rendering and its mutation are skipped (the ratchet still runs)"
fi

# ---------------------------------------------------------------- 2. the ratchet
rc=0; out="$("$CHECK" 2>&1)" || rc=$?
[ "$rc" = 0 ] || { echo "    source-pin-check fails on this tree ($rc): $out"; exit 1; }

R="$T/ratchet"; mkdir -p "$R/.ai/repo" "$R/site"
cp -R "$ROOT/site/templates" "$R/site/templates"
cp "$BASE" "$R/.ai/repo/source-pin-baseline.txt"
rc=0; MJ_ROOT="$R" "$CHECK" >/dev/null 2>&1 || rc=$?
[ "$rc" = 0 ] || { echo "    source-pin-check fails on an unchanged copy ($rc)"; exit 1; }
printf '<a href="%s/blob/master/README.md">readme</a>\n' "$REPO_URL" >> "$R/site/templates/entity.html"
rc=0; out="$(MJ_ROOT="$R" "$CHECK" 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    source-pin-check accepted an added blob/master (exit $rc)"; exit 1; }
printf '%s\n' "$out" | grep -q 'NEW  site/templates/entity.html' \
  || { echo "    source-pin-check did not name the template that grew: $out"; exit 1; }
# one more in a file the baseline already allows some in is refused too
cp "$ROOT/site/templates/entity.html" "$R/site/templates/entity.html"
f="$(awk '!/^#/ && NF == 2 { print $1; exit }' "$BASE")"
if [ -n "$f" ]; then
  printf '<a href="%s/tree/master/docs">docs</a>\n' "$REPO_URL" >> "$R/$f"
  rc=0; MJ_ROOT="$R" "$CHECK" >/dev/null 2>&1 || rc=$?
  [ "$rc" = 10 ] || { echo "    source-pin-check accepted one more tree/master in $f (exit $rc)"; exit 1; }
fi
exit 0
