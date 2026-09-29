# majordomus-covers: none
# claims: rustdoc-referenced
# Every builtin module's page on the site links the page the crate's reference documents that
# module on, and only in a build that composed the reference (ADR 0086, the rule
# project.the-crate-reference-is-published, "References").
#
# The mapping from a module to its rustdoc page is the crate's, and it is decided once: the
# rustdoc check derives every exported module's page from the crate's own inventory
# (quality::rustdoc::module_routes), holds the published tree to it, and the site's registry
# dataset carries the same answer as `rustdoc` on each module (site/data/registry/registry.json,
# written by `majordomus generate`). The template joins that with the surfaces the build
# composed (data/build.json, which scripts/site-build writes from docs/generated/web.json) and
# derives nothing. So the regressions this case exists for are three:
#
#   - a second mapping: a template or a generator that turns a source path into a rustdoc path
#     by its own rule links pages rustdoc never wrote — a pub(crate) module's, or one whose
#     rule moved with the toolchain — and every such link answers 404;
#   - a dropped link: a module the crate exports whose page carries no link;
#   - a link to a surface the build did not carry: the topology without the reference, and
#     the site still pointing at /rustdoc/.
#
# So the case asks the dataset against the check's own answer (`majordomus quality rustdoc`,
# its `modules`), builds a clone of this checkout with the reference composed and reads every
# module page (each exported module links a file that is in the composed site and in the
# producer's tree; no other module links anything), then builds it again from a topology
# without the surface and requires that no module page links the reference at all.
#
# The reference's tree is a producer output, never committed, so the clone borrows this
# checkout's, as cases 486 and 489 do. It must document the crate as HEAD has it: built from
# HEAD, or from a commit whose crate sources HEAD has not changed. Under CI the suite job
# produced it from this commit first (.github/actions/rustdoc) and installed zola, node and jq,
# so any of them absent fails the case; anywhere else it is what the person has not run yet
# (scripts/rust-check --doc), and the case says what it did not measure and ends (skip_case).
# The builds are real; they skip the stylesheet (--no-css), which no link depends on.
. "$ROOT/test/lib.sh"
command -v zola >/dev/null || skip_case "zola is absent, so no site was built and no module page was read"
command -v node >/dev/null || skip_case "node is absent, so no site was built and no module page was read"
command -v jq >/dev/null 2>&1 || skip_case "no jq, so the dataset and the topology were not read"
RB="$(rust_bin)" || rust_bin_exit $?
S="$(mktemp -d "${TMPDIR:-/tmp}/mj492.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git clone -q "$ROOT" "$S/work" 2>/dev/null
cd "$S/work" || exit 1
TREE="$ROOT/target/web/rustdoc"
[ -f "$TREE/surface.json" ] \
  || skip_case "target/web/rustdoc was not produced (run: scripts/rust-check --doc), so no module page was read"
built="$(jq -r '.built_from // empty' "$TREE/surface.json")"
if [ "$built" != "$(git rev-parse HEAD)" ]; then
  git cat-file -e "${built:-none}^{commit}" 2>/dev/null \
    && git diff --quiet "$built" HEAD -- apps/majordomus-cli rust-toolchain.toml \
    || skip_case "target/web/rustdoc was built from ${built:-no recorded commit}, whose crate is not HEAD's (run: scripts/rust-check --doc), so no module page was read"
fi
mkdir -p target/web
ln -s "$TREE" target/web/rustdoc
REG=site/data/registry/registry.json
EX=site/data/generated/executable.json

# ---------------------------------------------------------------- one mapping, the check's own
# Every builtin module links a page exactly when the rustdoc check lists its Rust module, and
# at the page the check derives for it. Nothing else decides which modules have a page.
# the report's module routes are the crate's and are answered whatever the verdict, so the
# exit status (0 clean, 10 findings) is not what this reads
MAJORDOMUS_SHARE="$PWD/share" "$RB" quality rustdoc --format json > "$S/rustdoc.json" 2> "$S/rustdoc.err" || true
jq -e '(.modules | length) > 0' "$S/rustdoc.json" >/dev/null 2>&1 \
  || { echo "    majordomus quality rustdoc answered no module routes:"; sed 's/^/    | /' "$S/rustdoc.err"; exit 1; }
jq -r --slurpfile q "$S/rustdoc.json" '
  . as $reg
  | ($q[0].modules | map({(.path): .route}) | add) as $pages
  | $reg.registry.modules[] | select(.source == "builtin") | .id as $id
  | ([$reg.registry.builtin[] | select(.module == $id) | .provenance.module] | first // "") as $rust
  | if (.rustdoc // null) == null then
      (if $pages[$rust] then "\($id): the check documents \($rust) at \($pages[$rust]) and the dataset links nothing" else empty end)
    elif .rustdoc.module != $rust then "\($id): the dataset links \(.rustdoc.module), and the module is \($rust)"
    elif $pages[$rust] == null then "\($id): the dataset links \(.rustdoc.path), a page the check does not derive for \($rust)"
    elif .rustdoc.path != $pages[$rust] then "\($id): the dataset links \(.rustdoc.path), the check derives \($pages[$rust])"
    else empty end' "$REG" > "$S/disagree.txt"
[ ! -s "$S/disagree.txt" ] || { echo "    the dataset and the rustdoc check disagree about module pages:"; sed 's/^/    /' "$S/disagree.txt"; exit 1; }
linked="$(jq '[.registry.modules[] | select(.source == "builtin" and .rustdoc)] | length' "$REG")"
unlinked="$(jq '[.registry.modules[] | select(.source == "builtin" and (.rustdoc | not))] | length' "$REG")"
[ "$linked" -gt 0 ] || { echo "    no builtin module has a page in the crate's reference"; exit 1; }
# the crate declares some builtin modules pub(crate); were there none, the half of this case
# that proves such a module links nothing would pass over an empty set
[ "$unlinked" -gt 0 ] || { echo "    every builtin module has a page, so nothing proves a module without one links nothing"; exit 1; }

# ---------------------------------------------------------------- composed: every page reads right
base="$(sed -n 's/^base_url = "\(.*\)"/\1/p' site/config.toml)"; base="${base%/}"
expect_exit 0 scripts/site-build --no-data --no-css
expect_grep '^== compose rustdoc at /rustdoc \(from target/web/rustdoc\)$'
mount="$(jq -r '.surfaces[] | select(.id == "rustdoc") | .mount' site/data/build.json)"
[ "$mount" = /rustdoc ] || { echo "    build.json does not record the reference at /rustdoc:"; jq -c '.surfaces' site/data/build.json; exit 1; }
jq -r --slurpfile ex "$EX" '
  ($ex[0].modules | map({(.id): .route}) | add) as $routes
  | .registry.modules[] | select(.source == "builtin")
  | [.id, $routes[.id], (.rustdoc.path // ""), (.rustdoc.module // "")] | @tsv' "$REG" > "$S/modules.tsv"
bad=0; seen=0
while IFS="$(printf '\t')" read -r id route path module; do
  page="site/public${route}index.html"
  [ -f "$page" ] || { echo "    module $id has no page at $page"; bad=1; continue; }
  # every page's menu links the reference's root; a module's own link names a page in it
  hrefs="$(grep -o 'href="[^"]*/rustdoc/[^"]*\.html"' "$page" | sed 's/^href="//; s/"$//' | tr '\n' ' ' || true)"
  if [ -z "$path" ]; then
    grep -q '>crate reference<' "$page" && { echo "    module $id has no rustdoc page and its site page links one anyway: $hrefs"; bad=1; }
    continue
  fi
  want="$base$mount/$path"
  grep -qF "href=\"$want\"" "$page" || { echo "    module $id does not link $want (links: ${hrefs:-none})"; bad=1; continue; }
  grep -qF ">$module</a>" "$page" || { echo "    module $id links its page without naming $module"; bad=1; }
  # the file the link names is in the composed site and in the producer's tree
  [ -f "site/public$mount/$path" ] || { echo "    module $id links $want, and the composed site has no site/public$mount/$path"; bad=1; }
  [ -f "target/web/rustdoc/$path" ] || { echo "    module $id links $path, which the producer's tree does not hold"; bad=1; }
  seen=$((seen + 1))
done < "$S/modules.tsv"
[ "$bad" = 0 ] || exit 1
[ "$seen" = "$linked" ] || { echo "    $seen module page(s) link the reference, and the dataset has $linked with a page"; exit 1; }

# ---------------------------------------------------------------- not composed: no page links it
# The topology the build reads lacks the surface. site-build composes nothing, records nothing,
# and no module page may point at a reference this build does not carry.
jq 'del(.surfaces[] | select(.id == "rustdoc"))' docs/generated/web.json > "$S/web-without.json"
cp "$S/web-without.json" docs/generated/web.json
expect_exit 0 scripts/site-build --no-data --no-css
expect_no_grep 'compose rustdoc'
jq -e '.surfaces == []' site/data/build.json >/dev/null \
  || { echo "    a build whose topology lacks the reference recorded a composed surface:"; jq -c '.surfaces' site/data/build.json; exit 1; }
[ ! -e site/public/rustdoc ] || { echo "    a build whose topology lacks the surface still composed it"; exit 1; }
leaked="$(grep -l '>crate reference<' site/public/registry/modules/*/index.html 2>/dev/null || true)"
[ -z "$leaked" ] || { echo "    module pages link the crate reference in a build that did not compose it:"; printf '%s\n' "$leaked" | sed 's/^/    /'; exit 1; }
git checkout -q -- docs/generated/web.json
echo "    $linked module page(s) link their rustdoc page, $unlinked link none; none do without the surface"
