# majordomus-covers: none
# A committed page's link into its own source names the commit the site was built from, not master.
#
# scripts/lib/project-links.awk rewrites a projected document's relative links into forge links,
# and what it writes is committed (site/content, site/data/generated). A committed file cannot
# name the commit it is in, so it writes a placeholder ref, and scripts/site-build
# pins it in the built site to the ref site/templates/source-links.html gives template-made links
# (case 533): the commit data/build.json records, master when the build is dirty or unknown.
#
#   1. the projection writes the placeholder, never master, for a tracked file and a directory;
#   2. rendered by zola in a fixture site, the placeholder reaches the HTML intact, and the pass
#      (scripts/site-build --pin-source-refs) makes it the commit of a clean build, master for a
#      dirty build, an unknown commit and no build.json; the full build runs that same pass over
#      its output after zola;
#   3. a placeholder the pass cannot resolve (outside a blob/ or tree/ link, or percent-encoded)
#      refuses the build (exit 10), and scripts/ci/link-check refuses a placeholder link by name;
#   4. the pin is load-bearing: a copy of site-build whose pass names master whatever the build
#      is, and a projection that writes master again, each fail step 1-2's checks, and the real
#      files are left byte-identical.
. "$ROOT/test/lib.sh"
command -v python3 >/dev/null 2>&1 || skip "no python3 for scripts/ci/link-check"

AWK="$ROOT/scripts/lib/project-links.awk"
BUILD="$ROOT/scripts/site-build"
CHECK="$ROOT/scripts/ci/link-check"
REPO=https://github.com/owner/fixture
COMMIT=0123456789abcdef0123456789abcdef01234567
[ -f "$AWK" ] && [ -x "$BUILD" ] && [ -x "$CHECK" ] || { echo "    a unit of the mechanism is missing"; exit 1; }

# ---------------------------------------------------------------- 1. the projection
printf 'docs/X.md\ndocs/sub/B.txt\ndocs/sub/C.txt\n' > tracked.txt
: > docs.tsv
cat > in.md <<'MD'
See [the file](sub/B.txt) and [the directory](sub) and [a fragment](sub/C.txt#top).
MD
# project <awk file>: the fixture document as the projection writes it
project() {
  awk -v src=docs/X.md -v repo="$REPO" -v tracked=tracked.txt -v docs=docs.tsv -v mode=zola \
      -v base=https://fixture.test -f "$1" < in.md
}
# projected <awk file>: 0 when every forge link carries the placeholder and none names master
projected() {
  local md; md="$(project "$1")"
  for want in "$REPO/blob/@source-ref@/docs/sub/B.txt" "$REPO/tree/@source-ref@/docs/sub" \
              "$REPO/blob/@source-ref@/docs/sub/C.txt#top"; do
    printf '%s' "$md" | grep -qF "($want)" || { echo "    the projection did not write $want: $md"; return 1; }
  done
  ! printf '%s' "$md" | grep -qE '/(blob|tree)/master/' || { echo "    the projection named master: $md"; return 1; }
}
projected "$AWK" || exit 1

# the full build pins what zola wrote, before any later pass reads the pages
grep -qE '^pin_source_refs "\$PUBLIC" "\$SITE/data/build\.json"$' "$BUILD" \
  || { echo "    site-build does not pin the source refs of the pages it built"; exit 1; }
awk '/^  zola "\$@"$/ { z = NR } /^pin_source_refs "\$PUBLIC"/ { p = NR } END { exit !(z && p > z) }' "$BUILD" \
  || { echo "    site-build pins the source refs before zola has written the pages"; exit 1; }

# ---------------------------------------------------------------- 2. rendered and pinned
# render <awk file> -> the directory of a fixture site built from the projected document
render() {
  local site; site="$(mktemp -d "$T/site.XXXXXX")"
  mkdir -p "$site/templates" "$site/content"
  printf 'base_url = "https://fixture.test"\n' > "$site/config.toml"
  printf '{{ page.content | safe }}\n' > "$site/templates/page.html"
  { printf '+++\ntitle = "x"\n+++\n{%% raw %%}\n'; project "$1"; printf '{%% endraw %%}\n'; } > "$site/content/x.md"
  ( cd "$site" && zola build >/dev/null 2>&1 ) || { echo "    zola could not build the fixture site" >&2; return 1; }
  printf '%s' "$site/public"
}
# hrefs <public>: the forge links of the page, one per line
hrefs() { grep -oE "href=\"$REPO/[^\"]*\"" "$1/x/index.html" | sed 's/^href="//; s/"$//' | LC_ALL=C sort; }
# pinned_as <site-build> <public> <build.json or ""> <ref>: the pass makes every link name <ref>
pinned_as() {
  local pub="$T/pin.$RANDOM"; rm -rf "$pub"; cp -R "$2" "$pub"
  local bj="$T/build.$RANDOM.json"; [ -z "$3" ] || printf '%s\n' "$3" > "$bj"
  rc=0; "$1" --pin-source-refs "$pub" "$bj" > pin.out 2>&1 || rc=$?
  [ "$rc" = 0 ] || { echo "    the pass failed ($rc): $(cat pin.out)"; return 1; }
  local want; want="$(printf '%s\n' "$REPO/blob/$4/docs/sub/B.txt" "$REPO/blob/$4/docs/sub/C.txt#top" \
    "$REPO/tree/$4/docs/sub" | LC_ALL=C sort)"
  local got; got="$(hrefs "$pub")"
  [ "$got" = "$want" ] || { echo "    build $3: got [$got], want the ref $4"; return 1; }
}
# pins <site-build> <public>: 0 when every kind of build names what it must
pins() {
  pinned_as "$1" "$2" "{\"commit\": \"$COMMIT\", \"dirty\": false}" "$COMMIT" || return 1
  pinned_as "$1" "$2" "{\"commit\": \"$COMMIT\", \"dirty\": true}" master || return 1
  pinned_as "$1" "$2" '{"commit": "unknown", "dirty": false}' master || return 1
  pinned_as "$1" "$2" "" master || return 1
}

HAVE_ZOLA=1
if ! command -v zola >/dev/null 2>&1; then
  [ "${CI:-}" != true ] || { echo "    zola is absent on CI, where the site cases must run rather than skip"; exit 1; }
  HAVE_ZOLA=0; echo "    zola absent; the rendering is skipped (projection, pass and refusals still run)"
fi
if [ "$HAVE_ZOLA" = 1 ]; then
  PUB="$(render "$AWK")" || exit 1
  [ "$(hrefs "$PUB" | grep -c '/@source-ref@/')" = 3 ] \
    || { echo "    the placeholder did not reach the rendered page intact: $(hrefs "$PUB")"; exit 1; }
else
  # the same three links as the rendered page carries them
  PUB="$T/pub.fixture"; mkdir -p "$PUB/x"
  printf '<p><a href="%s">a</a><a href="%s">b</a><a href="%s">c</a></p>\n' \
    "$REPO/blob/@source-ref@/docs/sub/B.txt" "$REPO/tree/@source-ref@/docs/sub" \
    "$REPO/blob/@source-ref@/docs/sub/C.txt#top" > "$PUB/x/index.html"
fi
pins "$BUILD" "$PUB" || { echo "    a content link does not name the ref of the build"; exit 1; }

# ---------------------------------------------------------------- 3. a leftover is refused
for leftover in 'the ref @source-ref@ in prose' "$REPO/raw/%40source-ref%40/docs/X.md"; do
  L="$T/left.$RANDOM"; rm -rf "$L"; cp -R "$PUB" "$L"
  printf '<p>%s</p>\n' "$leftover" >> "$L/x/index.html"
  rc=0; "$BUILD" --pin-source-refs "$L" /nonexistent/build.json > left.out 2>&1 || rc=$?
  [ "$rc" = 10 ] || { echo "    the pass accepted a leftover placeholder '$leftover' (exit $rc)"; cat left.out; exit 1; }
  grep -q 'x/index.html' left.out || { echo "    the refusal does not name the page: $(cat left.out)"; exit 1; }
done

F="$T/linkfix"; rm -rf "$F"; mkdir -p "$F/site/public" "$F/site/data/generated" "$F/docs/generated"
printf 'base_url = "https://example.test"\n' > "$F/site/config.toml"
printf '{"repository_url": "%s"}\n' "$REPO" > "$F/site/data/generated/project.json"
printf '# guide\n' > "$F/docs/GUIDE.md"
printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' \
  > "$F/docs/generated/web.json"
git -C "$F" init -q -b master && git -C "$F" add -A && git -C "$F" -c user.name=t -c user.email=t@t commit -qm first
page() { printf '<!doctype html><html><body><a href="%s">source</a></body></html>' "$1" > "$F/site/public/index.html"; }
page "$REPO/blob/master/docs/GUIDE.md"
rc=0; MJ_ROOT="$F" "$CHECK" > lc.out 2>&1 || rc=$?
[ "$rc" = 0 ] || { echo "    link-check refused the clean fixture ($rc)"; cat lc.out; exit 1; }
for l in "$REPO/blob/@source-ref@/docs/GUIDE.md" "$REPO/tree/@source-ref@/docs"; do
  page "$l"
  rc=0; MJ_ROOT="$F" "$CHECK" > lc.out 2>&1 || rc=$?
  [ "$rc" = 10 ] || { echo "    link-check accepted the placeholder link $l (exit $rc)"; cat lc.out; exit 1; }
  grep -q 'placeholder ref @source-ref@ was never resolved' lc.out \
    || { echo "    link-check did not name the placeholder: $(cat lc.out)"; exit 1; }
done

# ---------------------------------------------------------------- 4. the mutations
before_awk="$(cksum < "$AWK")"; before_build="$(cksum < "$BUILD")"
MA="$T/project-links.mutant.awk"
sed 's/SOURCE_REF = "@source-ref@"/SOURCE_REF = "master"/' "$AWK" > "$MA"
cmp -s "$AWK" "$MA" && { echo "    the projection mutation changed nothing"; exit 1; }
rc=0; projected "$MA" >/dev/null || rc=$?
[ "$rc" != 0 ] || { echo "    the projection check passed with master written back; it proves nothing"; exit 1; }

# a copy of site-build beside the library it sources, whose pass names master whatever the build is
M="$T/mutant"; mkdir -p "$M/scripts"; ln -s "$ROOT/lib" "$M/lib"
sed 's/then \.commit else "master" end/then "master" else "master" end/' "$BUILD" > "$M/scripts/site-build"
chmod +x "$M/scripts/site-build"
cmp -s "$BUILD" "$M/scripts/site-build" && { echo "    the pass mutation changed nothing"; exit 1; }
rc=0; pins "$M/scripts/site-build" "$PUB" >/dev/null || rc=$?
[ "$rc" != 0 ] || { echo "    the pin check passed with the pass naming master; it proves nothing"; exit 1; }

[ "$(cksum < "$AWK")" = "$before_awk" ] || { echo "    a mutation touched the real project-links.awk"; exit 1; }
[ "$(cksum < "$BUILD")" = "$before_build" ] || { echo "    a mutation touched the real site-build"; exit 1; }
exit 0
