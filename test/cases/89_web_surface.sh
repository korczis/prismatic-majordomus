# The web surface, proven from outside the executable: one resolved topology, and every
# projection of it agreeing.
#
# The invariant this case exists for is a semantic one that a compiler cannot hold. `/docs`
# once served the Swagger UI, because the word was free and the viewer wanted a home; the
# day real documentation arrived the name meant two things. So the assertions here are not
# "some route answers" but "this name belongs to this producer": the viewer is at /swagger,
# the documentation is under /docs/, and neither has quietly taken the other's mount.
#
# The second half is the declare-once property. The page a person reads and the JSON a
# program reads are both projections of the resolved topology, so they are asked
# independently and required to describe the same set. A surface that appears in one and
# not the other is the drift the rule project.web-surface-declared-once forbids.
#
# The process resolves what it serves once, at startup, rather than stating the filesystem
# per request — so "documentation is missing" and "documentation is built" are two servers
# here, not one server and a mkdir. That is the behaviour being asserted, not a workaround.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
command -v curl >/dev/null 2>&1 || { echo "    skip: no curl"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj89.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT

# Start a server on an ephemeral port against this fixture and set U to its base URL.
serve_up() {
  "$RB" serve --repo "$PWD" --port 0 > "$S/out.txt" 2> "$S/err.txt" & SRV=$!
  local i=0
  until grep -q 'listening on http://' "$S/err.txt" 2>/dev/null; do
    i=$((i+1)); [ "$i" -lt 300 ] || { echo "    the server never listened"; cat "$S/err.txt"; return 1; }
    kill -0 "$SRV" 2>/dev/null || { echo "    the server exited before listening"; cat "$S/err.txt"; return 1; }
    sleep 0.1
  done
  U="$(sed -n 's#.*listening on \(http://127\.0\.0\.1:[0-9]*\).*#\1#p' "$S/err.txt" | head -n 1)"
  [ -n "$U" ] || { echo "    no URL on the listening line"; cat "$S/err.txt"; return 1; }
}
# `wait` on a signalled child reports its signal, which is the expected outcome here and
# not a failure of the case, so neither it nor the kill is allowed to trip `set -e`.
serve_down() {
  [ -n "$SRV" ] || return 0
  kill "$SRV" 2>/dev/null || true
  wait "$SRV" 2>/dev/null || true
  SRV=""
}
code() { curl -s -o "$S/body" -w '%{http_code}' "$U$1"; }

# --- a repository with a site, so the documentation surface is discovered at all
"$MJ" init >/dev/null
mkdir -p site
printf 'base_url = "/"\n' > site/config.toml
git add -A >/dev/null && git commit -qm site

# ---------------------------------------------------------------- the resolved topology
# The mounts are data before they are routes: whatever the router later does, the topology
# is what every projection reads, so the reserved names are asserted here first.
"$RB" web list --format json > "$S/topology.json" 2> "$S/list.err" \
  || { echo "    web list --format json failed"; cat "$S/list.err"; exit 1; }
jq -e '.surfaces | length > 0' "$S/topology.json" >/dev/null || { echo "    the topology resolved no surfaces"; exit 1; }

mount_of() { jq -r --arg id "$1" '.surfaces[] | select(.id == $id) | .mount' "$S/topology.json"; }
[ "$(mount_of swagger)" = /swagger ] \
  || { echo "    the Swagger UI is mounted at '$(mount_of swagger)', not /swagger"; exit 1; }
[ "$(mount_of docs)" = /docs ] \
  || { echo "    the documentation is mounted at '$(mount_of docs)', not /docs"; exit 1; }
[ "$(mount_of openapi)" = /openapi.json ] \
  || { echo "    the OpenAPI document moved to '$(mount_of openapi)'"; exit 1; }
# the collision the repository actually had: the viewer owning the documentation's name
jq -e '[.surfaces[] | select(.mount == "/docs")] | length == 1 and (.[0].id == "docs")' "$S/topology.json" >/dev/null \
  || { echo "    /docs is claimed by something other than the documentation:"; jq -c '[.surfaces[]|select(.mount=="/docs")]' "$S/topology.json"; exit 1; }
jq -e '[.surfaces[] | select(.producer | test("swagger"; "i")) | .mount] | index("/docs") | not' "$S/topology.json" >/dev/null \
  || { echo "    a Swagger producer still claims /docs"; exit 1; }
# every surface carries the typed metadata the projections group and filter by
jq -e '[.surfaces[] | select((.category | not) or (.visibility | not) or (.kind | not) or (.mount | not))] | length == 0' "$S/topology.json" >/dev/null \
  || { echo "    a surface is missing category, visibility, kind or mount"; exit 1; }

# the validator accepts the repository's own topology; a mount claimed twice within one
# world is what it exists to refuse
expect_exit 0 "$RB" web validate

# ================================================================ documentation missing
# Nothing is built yet. The documentation must say so and name the repair, and must never
# answer with the viewer that used to live at this path.
serve_up || exit 1

[ "$(code /swagger)" = 200 ] || { echo "    GET /swagger did not answer 200"; exit 1; }
grep -qi 'swagger' "$S/body" || { echo "    /swagger is not the Swagger UI"; head -c 300 "$S/body"; exit 1; }
grep -q '/openapi.json' "$S/body" || { echo "    the Swagger UI does not read /openapi.json"; exit 1; }
[ "$(code /swagger/)" = 200 ] || { echo "    GET /swagger/ did not answer 200"; exit 1; }

[ "$(code /openapi.json)" = 200 ] || { echo "    GET /openapi.json did not answer 200"; exit 1; }
jq -e '.openapi and .paths' "$S/body" >/dev/null || { echo "    /openapi.json is not an OpenAPI document"; exit 1; }

c="$(code /docs/)"
grep -qi 'swagger-ui\|swagger ui' "$S/body" && { echo "    /docs served the Swagger UI"; exit 1; }
[ "$c" = 503 ] || { echo "    unbuilt documentation answered $c, not 503"; head -c 300 "$S/body"; exit 1; }
grep -q 'site-build' "$S/body" || { echo "    the unavailable page does not name the repair"; cat "$S/body"; exit 1; }

# an unavailable surface stays in introspection — hiding it would only hide it from the
# people maintaining it — but the page must not offer it as a link to a certain failure
curl -s -H 'Accept: application/json' "$U/" > "$S/home_missing.json" || { echo "    GET / as JSON failed"; exit 1; }
jq -e '[.surfaces[] | select(.id == "docs")] | length == 1' "$S/home_missing.json" >/dev/null \
  || { echo "    the unbuilt surface vanished from introspection instead of reporting its state"; exit 1; }
curl -s -H 'Accept: text/html' "$U/" > "$S/home_missing.html" || { echo "    GET / failed"; exit 1; }
grep -o 'href="/docs[^"]*"' "$S/home_missing.html" \
  && { echo "    the page links documentation that is not built"; exit 1; }
grep -q 'site-build' "$S/home_missing.html" \
  || { echo "    the page does not say how to build the documentation it is missing"; exit 1; }

serve_down

# ================================================================ documentation built
mkdir -p target/web/docs/cli
printf '<!doctype html><title>Docs</title><h1>Majordomus documentation</h1>\n' > target/web/docs/index.html
printf '<!doctype html><title>CLI</title><h1>CLI reference</h1>\n' > target/web/docs/cli/index.html
printf 'body{color:#222}\n' > target/web/docs/style.css
serve_up || exit 1

[ "$(code /docs/)" = 200 ] || { echo "    built documentation did not answer 200"; exit 1; }
grep -q 'Majordomus documentation' "$S/body" || { echo "    /docs/ did not serve the documentation index"; exit 1; }
[ "$(code /docs/cli/)" = 200 ] || { echo "    a nested documentation path did not answer"; exit 1; }
grep -q 'CLI reference' "$S/body" || { echo "    /docs/cli/ served the wrong page"; exit 1; }
[ "$(code /docs/style.css)" = 200 ] || { echo "    a documentation asset did not answer"; exit 1; }

# --- the documentation mount does not swallow its neighbours
[ "$(code /swagger)" = 200 ] || { echo "    /docs swallowed /swagger once it was built"; exit 1; }
grep -qi 'swagger' "$S/body" || { echo "    /swagger stopped being the Swagger UI"; exit 1; }
[ "$(code /openapi.json)" = 200 ] || { echo "    /docs swallowed /openapi.json"; exit 1; }

# --- a static mount is not a file server: nothing escapes the documentation root
for esc in '/docs/../../../../etc/passwd' '/docs/%2e%2e/%2e%2e/etc/passwd' '/docs/..%2f..%2fetc%2fpasswd'; do
  c="$(curl -s -o "$S/body" -w '%{http_code}' --path-as-is "$U$esc")"
  [ "$c" = 200 ] && { echo "    path traversal answered 200: $esc"; exit 1; }
  grep -q 'root:' "$S/body" 2>/dev/null && { echo "    path traversal escaped the root: $esc"; exit 1; }
done

# ---------------------------------------------------------------- the projections agree
# The page and the index are asked independently; both are projections of the topology, so
# the set of available public mounts they describe must be identical. This is the assertion
# that fails the day somebody writes a link into the page by hand.
curl -s -H 'Accept: text/html' "$U/" > "$S/home.html" || { echo "    GET / failed"; exit 1; }
grep -qi '<html\|<h1' "$S/home.html" || { echo "    / did not render a page"; head -c 200 "$S/home.html"; exit 1; }
curl -s -H 'Accept: application/json' "$U/" > "$S/home.json" || { echo "    GET / as JSON failed"; exit 1; }
jq -e '.surfaces | length > 0' "$S/home.json" >/dev/null || { echo "    the machine-readable index carries no surfaces"; exit 1; }

jq -r '.surfaces[] | select(.visibility == "public" and .ready) | .path' "$S/home.json" | LC_ALL=C sort -u > "$S/index_mounts.txt"
grep -o 'href="[^"]*"' "$S/home.html" | sed 's/href="//; s/"$//' \
  | grep '^/' | sed 's#\(..*\)/$#\1#' | LC_ALL=C sort -u > "$S/page_links.txt"
while read -r m; do
  [ -n "$m" ] || continue
  grep -qx "$m" "$S/page_links.txt" || { echo "    the index offers $m and the page does not link it"; cat "$S/page_links.txt"; exit 1; }
done < "$S/index_mounts.txt"
while read -r l; do
  [ -n "$l" ] || continue
  grep -qx "$l" "$S/index_mounts.txt" || { echo "    the page links $l, which is in no public surface: a hand-written link"; exit 1; }
done < "$S/page_links.txt"

# the page carries no absolute self-link: it must survive a port, a host and a proxy prefix
if grep -o 'href="http[^"]*"' "$S/home.html" | grep -qv 'github.com'; then
  echo "    the page hard-codes an absolute URL to itself"; grep -o 'href="http[^"]*"' "$S/home.html"; exit 1
fi

# --- neither projection leaks where the repository sits on this host
# The page is served to whoever can reach the socket. Where the checkout lives is of no use
# to them and of some use to somebody else, so it belongs in neither rendering of `/`.
if grep -q "$PWD" "$S/home.html"; then
  echo "    the page prints the repository's filesystem path"; exit 1
fi
if grep -q "$PWD" "$S/home.json"; then
  echo "    the machine-readable index prints the repository's filesystem path:"
  jq -c 'with_entries(select(.value | tostring | test("/")))' "$S/home.json"; exit 1
fi

serve_down

# ---------------------------------------------------------------- the rule exists
R="$ROOT/.ai/repo/rules/project/web-surface-declared-once.v1.md"
[ -f "$R" ] || { echo "    the rule that owns this invariant is missing: $R"; exit 1; }
grep -q '^id: project.web-surface-declared-once$' "$R" || { echo "    the rule's id is not project.web-surface-declared-once"; exit 1; }
grep -q '^class: blocking$' "$R" || { echo "    the rule is not blocking"; exit 1; }
# resolved against this checkout's own layer, not the fixture's: the rule is this
# repository's, and a rule that parses but does not resolve is enforced by nobody
"$MJ" --repo "$ROOT" rules list | grep -q 'project.web-surface-declared-once' \
  || { echo "    the rule does not resolve in this repository's effective set"; exit 1; }
