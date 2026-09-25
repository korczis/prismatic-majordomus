# majordomus-covers: none
# claims: rustdoc-complete
# A module added to the crate becomes a required page of its rustdoc with no list edited, and
# a page left behind by a module the crate no longer has is a finding (ADR 0086, the rule
# project.the-crate-reference-is-published, "References").
#
# `majordomus quality rustdoc` joins the tree the producer wrote with the crate's own
# measurement of what it exports (quality::source::Inventory) and derives every page it
# expects from that. So the regression this case exists for is a list: a module missing from
# a list kept for the check is a module whose missing page is never reported, and that list
# goes stale in exactly the direction that passes. Here nothing but the crate's source
# changes between the rounds, and the set of pages the check requires follows it.
#
# The fixture is a crate at the path the repository's own crate lives, documented by the real
# producer (scripts/rust-check --doc) under the repository's toolchain pin. Without cargo or jq
# it says what it did not measure and ends; under CI, where the job installs both, that
# absence fails the case (skip_case).
. "$ROOT/test/lib.sh"
command -v cargo >/dev/null 2>&1 || skip_case "no cargo, so no tree was produced and no page was required"
command -v jq >/dev/null 2>&1 || skip_case "no jq, so the check's findings were not read"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null
rustdoc_fixture_crate
git add -A >/dev/null && git commit -qm "the fixture crate"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj490.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# the check's verdict, in the machine-readable form, and the findings of one kind as
# "<file>" lines; the text form is asserted once, at the end
judge() { "$RB" quality rustdoc --format json > "$S/report.json" 2>/dev/null || true; }
found() { jq -r --arg k "$1" '.findings[] | select(.kind == $k) | .file' "$S/report.json"; }

# ---------------------------------------------------------------- a tree the crate agrees with
rustdoc_fixture_produce || exit 1
expect_exit 0 "$RB" quality rustdoc
expect_grep '^rustdoc: clean$'
judge
jq -e '.verdict == "clean" and (.findings | length) == 0' "$S/report.json" >/dev/null \
  || { echo "    the fresh tree is not clean:"; jq -c '.findings[]' "$S/report.json"; exit 1; }
jq -e '[.modules[].route] | index("majordomus_cli/alpha/index.html") != null' "$S/report.json" >/dev/null \
  || { echo "    the module alpha has no derived route:"; jq -c '.modules' "$S/report.json"; exit 1; }

# ---------------------------------------------------------------- a page the tree lost
# The crate does not change and one item page leaves the tree: that page is required, by
# name, and it is the only one missing.
mv target/web/rustdoc/majordomus_cli/alpha/struct.Thing.html "$S/thing.html"
expect_exit 10 "$RB" quality rustdoc
judge
[ "$(found missing-page)" = majordomus_cli/alpha/struct.Thing.html ] \
  || { echo "    the removed page is not the one missing page:"; jq -c '.findings[]' "$S/report.json"; exit 1; }
mv "$S/thing.html" target/web/rustdoc/majordomus_cli/alpha/struct.Thing.html
expect_exit 0 "$RB" quality rustdoc

# ---------------------------------------------------------------- a module the tree lacks
# One file added and one line of the crate root: nothing else in the repository changes, and
# the check requires two more pages — the module's and its struct's.
rustdoc_fixture_module beta
git add apps/majordomus-cli/src && git commit -qm "the crate gains a module"
changed="$(git diff --name-only HEAD~1 HEAD | LC_ALL=C sort | tr '\n' ' ')"
[ "$changed" = "apps/majordomus-cli/src/beta.rs apps/majordomus-cli/src/lib.rs " ] \
  || { echo "    adding the module changed more than the crate's source: $changed"; exit 1; }
# the tree says it is this commit's, and lacks the new module: only its absence is judged
jq --arg h "$(git rev-parse HEAD)" '.built_from = $h' target/web/rustdoc/surface.json > "$S/s.json" \
  && mv "$S/s.json" target/web/rustdoc/surface.json
expect_exit 10 "$RB" quality rustdoc
judge
missing="$(found missing-page | LC_ALL=C sort | tr '\n' ' ')"
[ "$missing" = "majordomus_cli/beta/index.html majordomus_cli/beta/struct.Thing.html " ] \
  || { echo "    the new module's pages were not required: missing-page is '$missing'"; jq -c '.findings[]' "$S/report.json"; exit 1; }
[ -z "$(found stale)" ] || { echo "    a tree declared this commit's was judged stale"; exit 1; }
[ "$(jq -r '.verdict' "$S/report.json")" = findings ] || { echo "    the verdict is not 'findings'"; exit 1; }

# the producer documents the module, and the same check is clean without a word changed in it
rustdoc_fixture_produce || exit 1
expect_exit 0 "$RB" quality rustdoc
judge
jq -e '[.modules[].route] | index("majordomus_cli/beta/index.html") != null' "$S/report.json" >/dev/null \
  || { echo "    the module beta has no derived route after the producer ran"; exit 1; }

# ---------------------------------------------------------------- a page without its item
# The module leaves the crate and the tree does not change: its pages are orphans, each named.
grep -vx 'pub mod beta;' apps/majordomus-cli/src/lib.rs > "$S/lib.rs" && cp "$S/lib.rs" apps/majordomus-cli/src/lib.rs
git rm -q apps/majordomus-cli/src/beta.rs
git add apps/majordomus-cli/src && git commit -qm "the crate loses a module"
jq --arg h "$(git rev-parse HEAD)" '.built_from = $h' target/web/rustdoc/surface.json > "$S/s.json" \
  && mv "$S/s.json" target/web/rustdoc/surface.json
expect_exit 10 "$RB" quality rustdoc
expect_grep 'orphan-page'
judge
found orphan-page | grep -qx 'majordomus_cli/beta/index.html' \
  || { echo "    the removed module's page is not an orphan finding"; jq -c '.findings[]' "$S/report.json"; exit 1; }
found orphan-page | grep -qx 'majordomus_cli/beta/struct.Thing.html' \
  || { echo "    the removed struct's page is not an orphan finding"; jq -c '.findings[]' "$S/report.json"; exit 1; }

# ---------------------------------------------------------------- nothing to judge
# No tree is never a pass over nothing: exit 12, with the producer named.
rm -rf target/web/rustdoc
expect_exit 12 "$RB" quality rustdoc
expect_grep 'scripts/rust-check --doc'
exit 0
