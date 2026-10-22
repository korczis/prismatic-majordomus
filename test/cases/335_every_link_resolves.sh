# majordomus-covers: none
# Every link the site publishes resolves, decided offline: scripts/ci/link-check, exercised against
# a site and a git history this case builds, so the answer for each link is known before the check
# runs. Rule: project.every-link-and-control-is-tested.
#
# What is proved: a clean site passes and says how many links of each class it decided. Then each
# class fails with a finding that names the page and the link: an internal path with no file, a
# fragment its page does not carry, a blob link to an untracked file, a blob link to a directory, a
# commit outside the published history, a tag that does not exist, a host nothing can decide offline,
# javascript:, and a bare "#". And a shallow clone refuses to report clean rather than skip what it
# cannot see.
. "$ROOT/test/lib.sh"
command -v python3 >/dev/null 2>&1 || { echo "    skip: no python3"; exit 0; }
CHECK="$ROOT/scripts/ci/link-check"
REPO=https://github.com/owner/fixture

fresh() {
  F="$PWD/site-$1"; rm -rf "$F"; mkdir -p "$F/site/public/docs/guide" "$F/site/data/generated" "$F/docs"
  printf 'base_url = "https://example.test"\n' > "$F/site/config.toml"
  printf '{"repository_url": "%s"}\n' "$REPO" > "$F/site/data/generated/project.json"
  printf '# guide\n' > "$F/docs/GUIDE.md"
  # the branch is named, not left to init.defaultBranch: the links below point at master
  git -C "$F" init -q -b master && git -C "$F" add -A && git -C "$F" -c user.name=t -c user.email=t@t commit -qm first
  FIRST="$(git -C "$F" rev-parse HEAD)"
  git -C "$F" tag v1.0.0
  cat > "$F/site/public/index.html" <<HTML
<!doctype html><html><head><link rel="canonical" href="https://example.test/"><link rel="stylesheet" href="https://example.test/app.css"></head>
<body><a href="#top" id="top">top</a><a href="https://example.test/docs/guide/#install">guide</a><a href="/docs/guide/">relative</a>
<a href="$REPO/blob/master/docs/GUIDE.md">source</a><a href="$REPO/tree/master/docs">docs</a><a href="$REPO/commit/$FIRST">commit</a>
<a href="$REPO/releases/tag/v1.0.0">release</a><a href="$REPO/compare/v1.0.0...master">compare</a></body></html>
HTML
  printf '<!doctype html><html><body><h2 id="install">install</h2><a href="https://example.test/">home</a></body></html>' > "$F/site/public/docs/guide/index.html"
  printf 'body{}' > "$F/site/public/app.css"
  printf '<?xml version="1.0"?><urlset><url><loc>https://example.test/</loc></url><url><loc>https://example.test/docs/guide/</loc></url></urlset>' > "$F/site/public/sitemap.xml"
}
# the suite runs a case under `bash -e`, so an exit the case expects is captured, never left to abort it
run() { rc=0; MJ_ROOT="$F" "$CHECK" > out.txt 2>&1 || rc=$?; echo "$rc" > rc.txt; }
# expect_finding <exit> <pattern> <what>: the check exits so, and one FAIL line names the defect
expect_finding() {
  run
  [ "$(cat rc.txt)" = "$1" ] || { echo "    $3: expected exit $1, got $(cat rc.txt)"; cat out.txt; exit 1; }
  grep -E "$2" out.txt >/dev/null || { echo "    $3: no finding matching /$2/"; cat out.txt; exit 1; }
}
# break <name> <html>: a fresh fixture whose homepage carries one more link
# (python, not sed: the links carry every delimiter sed could use)
break_with() {
  fresh "$1"
  python3 - "$F/site/public/index.html" "$2" <<'PY2'
import sys
p, link = sys.argv[1], sys.argv[2]
s = open(p).read(); open(p, 'w').write(s.replace('</body>', link + '</body>', 1))
PY2
}

# ---------------------------------------------------------------- clean
fresh clean; run
[ "$(cat rc.txt)" = 0 ] || { echo "    a clean site did not pass"; cat out.txt; exit 1; }
expect_grep '^OK   links .* resolve: [0-9]+ internal, [0-9]+ fragment, 5 repository' out.txt

# ---------------------------------------------------------------- each class, broken
break_with internal '<a href="https://example.test/docs/missing/">x</a>'
expect_finding 10 'FAIL internal .*/docs/missing/ resolves to no file' 'an internal link with no page'
break_with relative '<a href="/nowhere/">x</a>'
expect_finding 10 'FAIL internal .*/nowhere/ resolves to no file' 'a root-relative link with no page'
break_with fragment '<a href="https://example.test/docs/guide/#uninstall">x</a>'
expect_finding 10 'FAIL fragment .*#uninstall, which /docs/guide/ does not carry' 'a fragment its page lacks'
break_with selffrag '<a href="#bottom">x</a>'
expect_finding 10 'FAIL fragment .*#bottom, which / does not carry' 'an in-page fragment that is missing'
break_with untracked "<a href=\"$REPO/blob/master/.ai/local/state.yaml\">x</a>"
expect_finding 10 'FAIL repository .*\.ai/local/state\.yaml is not a tracked file' 'a blob link to an untracked path'
break_with blobdir "<a href=\"$REPO/blob/master/docs\">x</a>"
expect_finding 10 'FAIL repository .*docs is a directory; the link is tree/, not blob/' 'a blob link to a directory'
break_with commit "<a href=\"$REPO/commit/0123456789abcdef0123456789abcdef01234567\">x</a>"
expect_finding 10 'FAIL repository .*is not in the history being published' 'a commit outside the history'
break_with tag "<a href=\"$REPO/releases/tag/v9.9.9\">x</a>"
expect_finding 10 'FAIL repository .*tag v9\.9\.9 does not exist' 'a release tag that does not exist'
break_with foreign '<a href="https://elsewhere.example/page">x</a>'
expect_finding 10 'FAIL refused .*elsewhere\.example cannot be decided offline' 'a host nothing decides offline'
break_with script '<a href="javascript:void(0)">x</a>'
expect_finding 10 'FAIL refused .*is not a link' 'a javascript: href'
break_with bare '<a href="#">x</a>'
expect_finding 10 'FAIL refused .*a bare "#" goes nowhere' 'a bare fragment'
fresh sitemap; sed -i.bak 's|</urlset>|<url><loc>https://example.test/gone/</loc></url></urlset>|' "$F/site/public/sitemap.xml"; rm -f "$F/site/public/sitemap.xml.bak"
expect_finding 10 'FAIL internal +/sitemap\.xml: https://example\.test/gone/ resolves to no file' 'a sitemap entry with no page'

# ---------------------------------------------------------------- a shallow clone cannot decide
fresh deep; git -C "$F" -c user.name=t -c user.email=t@t commit -q --allow-empty -m second
S="$PWD/shallow"; rm -rf "$S"; git clone -q --depth 1 "file://$F" "$S"
cp -R "$F/site/public" "$S/site/"; cp "$F/site/data/generated/project.json" "$S/site/data/generated/"
rc=0; MJ_ROOT="$S" "$CHECK" > out.txt 2>&1 || rc=$?
[ "$rc" = 12 ] || { echo "    a shallow clone reported exit $rc instead of refusing (12)"; cat out.txt; exit 1; }
expect_grep 'cannot be decided in a shallow clone' out.txt
