# The web surfaces of a repository: discovered from their producers, validated as one
# topology, composed into one publishable tree, and served from the running executable.
#
# The case this exists for is the last one: a surface nobody has written yet. A directory
# under the generated web root with a declaration beside it becomes a surface — listed,
# validated, composed and served — without a line changing in the router, the command line,
# the publication or this case's own fixtures. If that ever stops being true, the
# architecture has leaked and this case says so.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
git add -A >/dev/null; git commit -qm base
BIN="$(rust_bin)" || { echo "    no toolchain; skipping"; exit 0; }
# the executable reads its kinds and schemas from the distribution it was built beside
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

# ---------------------------------------------------------------- what a bare repository has
"$BIN" web list > list.txt
expect_grep '^ID +KIND +MOUNT +CATEGORY +WHERE +SOURCE$' list.txt
expect_grep '^api +native +/api/v1' list.txt
expect_grep '^swagger +native +/swagger ' list.txt
# and never at /docs: that prefix is the documentation's, and a native route there would take
# every page under it (project.web-surface-declared-once)
expect_no_grep '^swagger +native +/docs' list.txt
# a repository with no site and nothing generated has no static surface at all
expect_no_grep '^app +static' list.txt
expect_exit 0 "$BIN" web validate

# ---------------------------------------------------------------- a surface nobody wrote code for
mkdir -p target/web/example-report
cat > target/web/example-report/surface.json <<'JSON'
{
  "schema": "web-surface/v1",
  "id": "example-report",
  "mount": "/example-report",
  "title": "An example report",
  "producer": "the fixture of test/cases/84_web_surfaces.sh"
}
JSON
printf '<!DOCTYPE html><title>example</title><h1>example report</h1>\n' > target/web/example-report/index.html

# it is discovered, described and valid, with nothing registered anywhere
"$BIN" web list > list.txt
expect_grep '^example-report +static +/example-report +.*target/web/example-report$' list.txt
expect_exit 0 "$BIN" web validate --artifacts
"$BIN" web explain example-report > explain.txt
expect_grep 'came from producer declaration target/web/example-report/surface.json' explain.txt
expect_grep 'producer   the fixture of' explain.txt

# it is selectable by the id it declared, and an id nothing declared is refused
expect_exit 0 "$BIN" web list --only example-report
expect_grep 'example-report'
expect_exit 10 "$BIN" web list --only nothing-declares-this

# it is composed into the publication under its mount, and nothing else is
expect_exit 0 "$BIN" web compose --destination target/site
[ -f target/site/example-report/index.html ] || { echo "    the surface was not composed under its mount"; exit 1; }
grep -q 'example report' target/site/example-report/index.html

# and it is served, by the same server, from the same resolution
"$BIN" serve --port 0 > serve.log 2>&1 &
serve_pid=$!
url=""
for _ in $(seq 1 50); do
  url="$(sed -n 's/.*listening on \(http:[^ ]*\).*/\1/p' serve.log | head -1)"
  [ -n "$url" ] && break
  sleep 0.2
done
[ -n "$url" ] || { echo "    the server never logged its address:"; cat serve.log; kill $serve_pid 2>/dev/null; exit 1; }
status() { curl -s -o /dev/null -w '%{http_code}' "$url$1"; }
[ "$(status /example-report/)" = 200 ] || { echo "    /example-report/ answered $(status /example-report/)"; kill $serve_pid 2>/dev/null; exit 1; }
[ "$(status /example-report/index.html)" = 200 ] || { echo "    the file answered $(status /example-report/index.html)"; kill $serve_pid 2>/dev/null; exit 1; }
# the routes the executable answers itself are not taken by a static surface
[ "$(status /openapi.json)" = 200 ] || { echo "    /openapi.json answered $(status /openapi.json)"; kill $serve_pid 2>/dev/null; exit 1; }
# and nothing escapes the surface's own directory
[ "$(status /example-report/../../../etc/passwd)" = 404 ] || { echo "    a traversal was answered"; kill $serve_pid 2>/dev/null; exit 1; }
kill $serve_pid 2>/dev/null; wait $serve_pid 2>/dev/null || true

# ---------------------------------------------------------------- and every way it can be wrong
# two surfaces claiming one mount
mkdir -p target/web/second-claim
sed 's/example-report/second-claim/; s|"mount": "/second-claim"|"mount": "/example-report"|' \
  target/web/example-report/surface.json > target/web/second-claim/surface.json
printf '<!DOCTYPE html><title>x</title>\n' > target/web/second-claim/index.html
expect_exit 10 "$BIN" web validate
expect_grep 'surface.mount-collision'
rm -rf target/web/second-claim

# a surface mounted inside another's subtree
mkdir -p target/web/nested
printf '{"schema":"web-surface/v1","id":"nested","mount":"/example-report/nested"}\n' > target/web/nested/surface.json
printf '<!DOCTYPE html><title>x</title>\n' > target/web/nested/index.html
expect_exit 10 "$BIN" web validate
expect_grep 'surface.nested-mount'
rm -rf target/web/nested

# a declaration with a key the contract does not have
printf '{"schema":"web-surface/v1","id":"typo","mount":"/typo","moutn":"/oops"}\n' > target/web/example-report/surface.json
expect_exit 10 "$BIN" web list
rm -rf target/web/example-report

# ---------------------------------------------------------------- and it leaves no trace
"$BIN" web list > list.txt
expect_no_grep 'example-report' list.txt
expect_exit 0 "$BIN" web validate
expect_exit 0 "$BIN" web compose --destination target/site
[ -e target/site/example-report ] && { echo "    a removed surface left files in the publication"; exit 1; }
exit 0
