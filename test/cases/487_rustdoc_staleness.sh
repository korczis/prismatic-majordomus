# majordomus-covers: none
# claims: rustdoc-verified-live
# A rustdoc tree built from one commit and presented as another is refused: by the integrity
# check before publication (`majordomus quality rustdoc`, built_from against HEAD) and by the
# public verification after it (`scripts/pages verify-rustdoc`, against the commit that was
# deployed), which also refuses a published tree that is incomplete (ADR 0086).
#
# The regressions this case exists for are the two ways generation succeeding gets read as
# a correct deployment. A tree left over from an earlier build is a reference to code that is
# no longer live, and every page in it still renders; a public site that serves this commit's
# /build.json can still serve last week's reference, or half of this one. So the tree is built
# from one commit, the repository moves on, and both checks are asked; then the public check
# is asked of a site served over a real socket, whole, from another commit, with the surface's
# own declaration naming another commit than the site's identity does, with the crate's own
# COMMIT page disagreeing with the declaration (a tree assembled from two builds), with a
# stylesheet missing, with a module page missing, and with nothing answering at all.
#
# The third check of the same staleness, the site's own (scripts/site-check section 13m,
# which refuses a composed surface whose built_from is not the commit of the build it is
# in), needs a real site build and is asked in case 489, over the build that is deployed.
#
# The fixture crate is documented by the real producer (scripts/rust-check --doc) under the
# repository's toolchain pin; the site is a directory served by python3 or node (start_http).
# Without cargo, jq, curl or a server to start it says what it did not measure and ends;
# under CI, where the job installs all four, that absence fails the case (skip_case).
. "$ROOT/test/lib.sh"
command -v cargo >/dev/null 2>&1 || skip_case "no cargo, so no tree was produced and no staleness was judged"
command -v jq >/dev/null 2>&1 || skip_case "no jq, so no staleness was judged"
command -v curl >/dev/null 2>&1 || skip_case "no curl, so the public verification was not asked"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj487.XXXXXX")"
trap 'stop_http; rm -rf "$S"' EXIT
"$MJ" init >/dev/null
rustdoc_fixture_crate
git add -A >/dev/null && git commit -qm "the fixture crate"

# ---------------------------------------------------------------- built from A
rustdoc_fixture_produce || exit 1
a="$(git rev-parse HEAD)"
expect_exit 0 "$RB" quality rustdoc
grep -qF "&quot;$a" target/web/rustdoc/majordomus_cli/constant.COMMIT.html \
  || { echo "    the COMMIT page of a tree built from $a does not name it"; exit 1; }

# ---------------------------------------------------------------- checked as B
# The repository moves on and the tree does not: every page still renders, and the tree is
# refused as stale, naming both commits.
git commit -q --allow-empty -m "the repository moves on"
b="$(git rev-parse HEAD)"
expect_exit 10 "$RB" quality rustdoc --kind stale
expect_grep "built from $a, and the repository's HEAD is $b"
"$RB" quality rustdoc --format json > "$S/report.json" 2>/dev/null || true
jq -e '[.findings[].kind] == ["stale"]' "$S/report.json" >/dev/null \
  || { echo "    staleness was not the only finding:"; jq -c '.findings[]' "$S/report.json"; exit 1; }

# ---------------------------------------------------------------- the public verification
# A site as the deploy composes it: the tree at the topology's mount, and the build's identity
# naming it built from A. The mount is the repository's (docs/generated/web.json), read by the
# verification itself; the case only puts the tree where the site would carry it.
mount="$(jq -r '.surfaces[] | select(.id == "rustdoc") | .mount' "$ROOT/docs/generated/web.json")"
[ -n "$mount" ] || { echo "    the repository's web.json declares no rustdoc surface"; exit 1; }
mkdir -p "$S/site$mount"
cp -R target/web/rustdoc/. "$S/site$mount/"
jq -n --arg c "$a" --arg m "$mount" \
  '{schema: 1, commit: $c, dirty: false, surfaces: [{id: "rustdoc", mount: $m, built_from: $c}]}' \
  > "$S/site/build.json"
start_http "$S/site" || skip_case "no python3 or node to serve the site, so the public verification was not asked"
verify() { expect_exit "$1" "$ROOT/scripts/pages" verify-rustdoc --url "$HTTP_BASE" --commit "$2" --timeout 0; }

# whole and A's: verified, the COMMIT page among what was read
verify 0 "$a"
expect_grep "constant\.COMMIT\.html .* ok$"
expect_grep "search\.index/root\.js .* ok$"
# asked as B: the site serves A's reference, which is not B's
verify 10 "$b"
expect_grep "surface\.json .*built_from=$a, want $b.* fail$"
# the site's identity still says A and the declaration published beside the pages says B: the
# reference the site serves is not the one the build says it composed, and the one beside the
# pages is read rather than trusted from the identity
cp "$S/site$mount/surface.json" "$S/surface.json"
jq --arg b "$b" '.built_from = $b' "$S/surface.json" > "$S/site$mount/surface.json"
verify 10 "$a"
expect_grep "/build\.json .*surfaces\.rustdoc\.built_from=$a\" ok$"
expect_grep "surface\.json .*id rustdoc, built_from=$b, want $a.* fail$"
cp "$S/surface.json" "$S/site$mount/surface.json"
# a tree assembled from two builds: the declarations say A, the crate's own page says B
cp "$S/site$mount/majordomus_cli/constant.COMMIT.html" "$S/commit.html"
sed "s/&quot;$a/\&quot;$b/" "$S/commit.html" > "$S/site$mount/majordomus_cli/constant.COMMIT.html"
verify 10 "$a"
expect_grep "constant\.COMMIT\.html .*COMMIT constant naming $a.* fail$"
cp "$S/commit.html" "$S/site$mount/majordomus_cli/constant.COMMIT.html"
verify 0 "$a"
# a stylesheet the crate's index loads is missing: the reference is published half
css="$(cd "$S/site$mount" && ls static.files/*.css | head -n 1)"
[ -n "$css" ] || { echo "    the tree has no stylesheet to remove"; exit 1; }
mv "$S/site$mount/$css" "$S/removed.css"
verify 10 "$a"
expect_grep "${css##*/}.* status=404 .* fail$"
mv "$S/removed.css" "$S/site$mount/$css"
# a module page the crate's index links is missing: the index and every asset answer, and the
# reference is still not whole. The fixture's one module is the first the index links.
mv "$S/site$mount/majordomus_cli/alpha/index.html" "$S/alpha.html"
verify 10 "$a"
expect_grep "majordomus_cli/alpha/index\.html .* status=404 .*majordomus_cli::alpha - Rust.* fail$"
mv "$S/alpha.html" "$S/site$mount/majordomus_cli/alpha/index.html"
verify 0 "$a"
# nothing answers: never judged, and never a pass
stop_http
verify 12 "$a"
exit 0
