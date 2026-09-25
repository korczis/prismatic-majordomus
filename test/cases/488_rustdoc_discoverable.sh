# majordomus-covers: none
# claims: rustdoc-discoverable
# The crate's rustdoc is a web surface declared by discovery from the crate: in the topology,
# its committed projection and the home page whether or not the producer has run, and
# answered by a running server at /rustdoc/ from the tree the producer wrote (ADR 0086).
#
# Two regressions this case exists for. The first is a surface that exists only once a
# producer has run in one checkout: then the committed docs/generated/web.json, which the
# site's build reads to decide what it composes, depends on which clone generated it, and a
# clone that had not run the producer would publish a site without the reference. So the
# declaration is asked before the producer runs, and the committed projection is required to
# carry it with nothing of any checkout in it. The second is a reference that is declared and
# unreachable: the mount is asked of a real socket, unbuilt (503, naming the producer) and
# built (the pages, their assets and the redirect stub rustdoc writes for a macro, each with
# its type).
#
# Two more ways the reference stops being reachable are asked too. A second producer that
# declares a surface at /rustdoc is refused by the topology's own validation, since one path
# has one owner. And the site links the mount from its menu, and a link a site page makes
# into the reference is resolved by scripts/ci/link-check against the pages the producer
# wrote: the menu's link and a link to an item page resolve, and a link to a page the
# producer never wrote is refused.
#
# The fixture crate is documented by the real producer (scripts/rust-check --doc) under the
# repository's toolchain pin. Without cargo, jq, curl or python3 it says what it did not
# measure and ends; under CI, where the job installs all four, that absence fails the case.
. "$ROOT/test/lib.sh"
command -v cargo >/dev/null 2>&1 || skip_case "no cargo, so no tree was produced and nothing was served"
command -v jq >/dev/null 2>&1 || skip_case "no jq, so the topology was not read"
command -v curl >/dev/null 2>&1 || skip_case "no curl, so the mount was not asked of a server"
command -v python3 >/dev/null 2>&1 || skip_case "no python3, so scripts/ci/link-check was not run"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj488.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT
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
serve_down() {
  [ -n "$SRV" ] || return 0
  kill "$SRV" 2>/dev/null || true
  wait "$SRV" 2>/dev/null || true
  SRV=""
}
# status, content type and redirect target of one path, the body into $S/body
probe() { curl -s --path-as-is -o "$S/body" -w '%{http_code} %{content_type} %{redirect_url}' "$U$1"; }
# the home page as a browser asks for it (a program is answered JSON)
home() { curl -s -H 'Accept: text/html' -o "$S/body" "$U/"; }

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm layer

# ---------------------------------------------------------------- no crate, no surface
"$RB" web list > "$S/list.txt" 2>/dev/null
expect_no_grep '^rustdoc ' "$S/list.txt"

# ---------------------------------------------------------------- the crate, nothing built
rustdoc_fixture_crate
git add -A >/dev/null && git commit -qm "the fixture crate"
[ ! -e target/web/rustdoc ] || { echo "    the fixture produced a tree before the producer ran"; exit 1; }
"$RB" web list > "$S/list.txt" 2>/dev/null
expect_grep '^rustdoc +static +/rustdoc +documentation +served\+published +target/web/rustdoc' "$S/list.txt"
expect_exit 0 "$RB" web explain rustdoc
expect_grep 'producer +scripts/rust-check --doc'
expect_grep 'kind +came from filesystem apps/majordomus-cli'
expect_exit 0 "$RB" web validate
# A second producer at the reference's mount: a copy of it declared under another id. Two
# answers for one path, and whichever the router consulted first would be the reference, so
# the topology refuses it, as a collision naming both owners and as a claim on a reserved
# name, before anything serves either.
mkdir -p target/web/rust-api
printf '{"schema":"web-surface/v1","id":"rust-api","mount":"/rustdoc"}\n' > target/web/rust-api/surface.json
expect_exit 10 "$RB" web validate
expect_grep 'surface\.mount-collision'
expect_grep 'surface\.reserved-namespace'
expect_grep 'target/web/rust-api/surface\.json'
rm -rf target/web/rust-api
expect_exit 0 "$RB" web validate
# The projection the site's build reads is the repository's committed docs/generated/web.json,
# and it is the same declaration, with no value of the checkout that generated it: no
# built_from, which is the one value a producer's run changes. `generate` cannot run in this
# fixture — the executable refuses a tree whose crate is not the one it was built from — so
# the projection is asked of the repository itself, read-only: its committed file, and the
# executable's own verdict that the file is what it would write now, whether or not a tree is
# on disk there (the crate's generate::tests hold the with-and-without half).
jq -e '.reserved.rustdoc == "/rustdoc"
       and ([.surfaces[] | select(.id == "rustdoc")] | length) == 1
       and (.surfaces[] | select(.id == "rustdoc")
            | .mount == "/rustdoc" and .kind == "static-directory" and .availability == "both"
              and .artifact == "target/web/rustdoc" and (has("built_from") | not))' \
  "$ROOT/docs/generated/web.json" >/dev/null \
  || { echo "    the committed web.json does not carry the rustdoc surface and its reservation as declared:"
       jq -c '.reserved, (.surfaces[] | select(.id == "rustdoc"))' "$ROOT/docs/generated/web.json"; exit 1; }
expect_exit 0 "$RB" generate web --check --repo "$ROOT"

# unbuilt, the mount is answered and names what builds it, rather than 404
serve_up || exit 1
r="$(probe /rustdoc/)"
case "$r" in 503\ *) ;; *) echo "    unbuilt /rustdoc/ answered '$r', not 503"; serve_down; exit 1 ;; esac
grep -qF 'scripts/rust-check --doc' "$S/body" || { echo "    the 503 does not name the producer"; cat "$S/body"; serve_down; exit 1; }
# the home page lists it from the same topology, as not built and naming the producer, with no
# link that is certain to fail
home
grep -qF '<code>/rustdoc/</code>' "$S/body" || { echo "    the home page does not list /rustdoc/"; serve_down; exit 1; }
grep -qF 'not built</span> — run <code>scripts/rust-check --doc</code>' "$S/body" \
  || { echo "    the home page does not say the reference is not built and what builds it"; serve_down; exit 1; }
grep -qF 'href="/rustdoc/"' "$S/body" && { echo "    the home page links a reference that is not built"; serve_down; exit 1; }
serve_down

# ---------------------------------------------------------------- the producer ran
rustdoc_fixture_produce || exit 1
head="$(git rev-parse HEAD)"
[ "$(jq -r .built_from target/web/rustdoc/surface.json)" = "$head" ] \
  || { echo "    the producer did not declare the tree built from HEAD"; exit 1; }
# the crate's own COMMIT page names the same commit: the producer hands the build the value it
# records, so the two witnesses have one source
grep -qF "&quot;$head" target/web/rustdoc/majordomus_cli/constant.COMMIT.html \
  || { echo "    the COMMIT constant page does not name $head"; exit 1; }
# the running topology reads what the producer declared; the committed projection never does
expect_exit 0 "$RB" web explain rustdoc
expect_grep "built from +$head"

serve_up || exit 1
home
grep -qF 'href="/rustdoc/"' "$S/body" || { echo "    the home page does not link the built reference"; serve_down; exit 1; }
r="$(probe /rustdoc/)"
case "$r" in "200 text/html"*) ;; *) echo "    /rustdoc/ answered '$r'"; serve_down; exit 1 ;; esac
grep -qF "$head" "$S/body" || { echo "    the landing page does not name the commit it documents"; serve_down; exit 1; }
r="$(probe /rustdoc/majordomus_cli/index.html)"
case "$r" in "200 text/html"*) ;; *) echo "    the crate's index answered '$r'"; serve_down; exit 1 ;; esac
grep -qF '<title>majordomus_cli - Rust</title>' "$S/body" || { echo "    the crate's index is not rustdoc's"; serve_down; exit 1; }
r="$(probe /rustdoc/majordomus_cli/alpha/struct.Thing.html)"
case "$r" in "200 text/html"*) ;; *) echo "    an item page answered '$r'"; serve_down; exit 1 ;; esac
# the redirect stub rustdoc writes for a macro carries a '!' in its name, and is served as the
# page it is
[ -f 'target/web/rustdoc/majordomus_cli/macro.nothing!.html' ] \
  || { echo "    rustdoc wrote no macro.nothing!.html; the case no longer asks what it says"; serve_down; exit 1; }
r="$(probe '/rustdoc/majordomus_cli/macro.nothing!.html')"
case "$r" in "200 text/html"*) ;; *) echo "    the macro's redirect stub answered '$r'"; serve_down; exit 1 ;; esac
# every stylesheet, script and font rustdoc wrote, each with its type
for f in target/web/rustdoc/static.files/*; do
  n="${f#target/web/rustdoc/}"
  case "$n" in *.css) want=text/css ;; *.js) want=javascript ;; *.woff2) want=font/woff2 ;; *) continue ;; esac
  r="$(probe "/rustdoc/$n")"
  case "$r" in "200 "*"$want"*) ;; *) echo "    /rustdoc/$n answered '$r', want 200 $want"; serve_down; exit 1 ;; esac
done
# the search index the pages load
r="$(probe /rustdoc/search.index/root.js)"
case "$r" in "200 "*javascript*) ;; *) echo "    the search index answered '$r'"; serve_down; exit 1 ;; esac
# the mount without its slash is sent to the directory rather than answered as a file
r="$(probe /rustdoc)"
case "$r" in 30[178]\ *"$U/rustdoc/") ;; *) echo "    /rustdoc answered '$r', not a redirect to /rustdoc/"; serve_down; exit 1 ;; esac
# and nothing outside the tree is reachable through it
r="$(probe /rustdoc/../Cargo.toml)"
case "$r" in 200\ *) echo "    /rustdoc/../ reached outside the tree"; serve_down; exit 1 ;; esac
r="$(probe /rustdoc/%2e%2e/.gitignore)"
case "$r" in 200\ *) echo "    an encoded .. reached outside the tree"; serve_down; exit 1 ;; esac
serve_down

# ---------------------------------------------------------------- reachable from the site
# The site's menu links the mount the committed topology declares: read from the menu's own
# declaration (site/data/nav.toml), never assumed. scripts/site-check proves the other half on
# a real build — a published section in no menu is refused (13e), and a menu link that does
# not resolve is refused (13d); case 486 runs it.
mount="$(jq -r '.surfaces[] | select(.id == "rustdoc") | .mount' "$ROOT/docs/generated/web.json")"
[ -n "$mount" ] || { echo "    the repository's web.json declares no rustdoc surface"; exit 1; }
navs="$(sed -n "s#.*href = \"\\($mount/[^\"]*\\)\".*#\\1#p" "$ROOT/site/data/nav.toml" | LC_ALL=C sort -u)"
printf '%s\n' "$navs" | grep -qx "$mount/" \
  || { echo "    the site's menu (site/data/nav.toml) does not link $mount/; under it: ${navs:-nothing}"; exit 1; }
# A site of one page carrying those menu links, as Zola emits them (absolute, on the base
# URL), and a link to one item page, with the fixture's reference composed at the mount where
# scripts/site-build composes it. The check is the repository's own scripts/ci/link-check, run
# over this directory (MJ_ROOT): it scans the site's pages and resolves every link they make
# into a composed surface against that surface's files.
L="$S/linked"
mkdir -p "$L/site/public" "$L/site/data/generated" "$L/docs/generated"
git -C "$L" init -q
cp "$ROOT/site/config.toml" "$L/site/config.toml"
cp "$ROOT/docs/generated/web.json" "$L/docs/generated/web.json"
jq '{repository_url}' "$ROOT/site/data/generated/project.json" > "$L/site/data/generated/project.json"
cp -R target/web/rustdoc "$L/site/public$mount"
base="$(sed -n 's/^base_url = "\(.*\)"/\1/p' "$L/site/config.toml")"
site_page() {   # the page, with its menu and the extra links given
  { printf '<!doctype html>\n<html lang="en"><head><title>fixture</title></head><body><nav>\n'
    for h in $navs "$@"; do printf '<a href="%s%s">link</a>\n' "${base%/}" "$h"; done
    printf '</nav></body></html>\n'; } > "$L/site/public/index.html"
}
site_page "$mount/majordomus_cli/alpha/struct.Thing.html"
expect_exit 0 env MJ_ROOT="$L" "$ROOT/scripts/ci/link-check"
expect_grep "under $mount/ are composed surfaces' own and were not scanned"
# a site link to a page the producer never wrote: refused, naming the link
site_page "$mount/majordomus_cli/alpha/struct.Gone.html"
expect_exit 10 env MJ_ROOT="$L" "$ROOT/scripts/ci/link-check"
expect_grep "^FAIL internal +/: ${base%/}$mount/majordomus_cli/alpha/struct\.Gone\.html resolves to no file under site/public$"
# and the menu's own link is judged the same way: with the reference absent it resolves to
# nothing, which is how a site that stopped composing it is refused
rm -rf "$L/site/public$mount"
site_page
expect_exit 10 env MJ_ROOT="$L" "$ROOT/scripts/ci/link-check"
expect_grep "^FAIL internal +/: ${base%/}$mount/ resolves to no file under site/public$"
exit 0
