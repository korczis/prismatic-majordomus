# majordomus-covers: none
# release-check holds a published pre-release to its record as it holds a stable release, ignores
# a tag whose suffix is not a pre-release, and orders versions by semantic-version precedence
# (I2829). A fixture repository in the shape of case 3029, with a stub `gh` on PATH
# (MJ_RELEASE_CHECK_REPO names the repository, so nothing reaches the network):
#
#   1  v9.9.0-alpha.1 tagged and released, with no record and no record branch: the finding;
#      v0.2.0-ai-documents tagged beside it is never mentioned
#   2  the same with its record: no finding
#   3  the tree declares 9.9.0 and the newest record is 9.9.0-alpha.1: not behind (a plain
#      `sort -t.` ranked the pre-release above its own final and called the tree behind)
#   4  the tree declares 0.29.0 and 9.9.0-alpha.1 is recorded: behind, the pre-release counts
. "$ROOT/test/lib.sh"

P="$T/published"; O="$T/origin.git"; FX="$T/fx"; LOG="$T/gh.log"
mkdir -p "$P/scripts/ci" "$P/apps/majordomus-cli/src/release" "$P/share" "$P/.ai/repo/releases" "$FX" "$T/bin"
cp "$ROOT/scripts/ci/release-check" "$P/scripts/ci/"
cp "$ROOT/scripts/release-version" "$P/scripts/"
cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$ROOT/apps/majordomus-cli/Cargo.lock" "$P/apps/majordomus-cli/"
cp "$ROOT/apps/majordomus-cli/src/release/version.rs" "$P/apps/majordomus-cli/src/release/"
cp -R "$ROOT/bin" "$ROOT/lib" "$P/"
printf 'nothing about releases\n' > "$P/README.md"
V="$("$ROOT/scripts/release-version")"
declare_version() {   # <version>: what the fixture's crate, and so its shell tool, states
  local m="$P/apps/majordomus-cli"
  awk -v v="$1" '/^\[package\]/ { p = 1 } p && /^version *=/ { $0 = "version = \"" v "\""; p = 0 } 1' \
    "$m/Cargo.toml" > "$m/Cargo.toml.new" && mv "$m/Cargo.toml.new" "$m/Cargo.toml"
  awk -v v="$1" '$0 == "name = \"majordomus-cli\"" { p = 1; print; next } p && /^version = / { $0 = "version = \"" v "\""; p = 0 } 1' \
    "$m/Cargo.lock" > "$m/Cargo.lock.new" && mv "$m/Cargo.lock.new" "$m/Cargo.lock"
  printf 'version=%s\n' "$1" > "$P/share/version.txt"
}
record() {   # <version>: the release record of v<version>
  printf 'schema: release/v1\nversion: "%s"\ntag: v%s\nchannel: %s\n' "$1" "$1" "$2" > "$P/.ai/repo/releases/v$1.yaml"
}
declare_version "$V"
record "$V" stable

g() { git -C "$P" "$@"; }
git init -q --bare "$O"
g init -q -b master
g config user.email t@example.com
g config user.name T
g remote add origin "$O"
g add -A
g commit -qm "the tree the tags were cut from"
for t in "v$V" v9.9.0-alpha.1 v0.2.0-ai-documents; do g tag "$t"; done
g push -q origin master --tags
g fetch -q origin

cat > "$T/bin/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$GH_STUB_LOG"
case "$*" in
  *"/actions/workflows/release.yml/runs?branch="*)
    echo '{"workflow_runs":[{"id":7,"status":"completed","conclusion":"success","created_at":"2026-10-17T12:00:00Z","html_url":"https://example.invalid/runs/7"}]}' ;;
  *"/releases/tags/"*) echo '{"tag_name":"x"}' ;;
  *) echo "stub gh: no answer for $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$T/bin/gh"
# MJ_ROOT unset: the gate honours it, and a caller that exports it (majordomus finish does) would
# otherwise have the gate measure that tree instead of this fixture
gate() { ( cd "$P" && env -u MJ_ROOT PATH="$T/bin:$PATH" GH_STUB_LOG="$LOG" GH_STUB_FX="$FX" MJ_RELEASE_CHECK_REPO=o/r ./scripts/ci/release-check ); }

# ---------------------------------------------------------------- 1. a released alpha with no record
expect_exit 10 gate || { echo "    a published pre-release with no record was excused"; exit 1; }
expect_grep "FAIL  v9.9.0-alpha.1 is published and the layer holds no release record for it" \
  || { echo "    the unrecorded pre-release is not named"; exit 1; }
expect_no_grep 'v0.2.0-ai-documents' || { echo "    a tag whose suffix is not a pre-release was treated as a release"; exit 1; }

# ---------------------------------------------------------------- 2. its record answers it
record 9.9.0-alpha.1 prerelease
declare_version 9.9.0-alpha.1
g add -A; g commit -qm "chore(release): record v9.9.0-alpha.1"
expect_exit 0 gate || { echo "    the recorded pre-release is still refused"; exit 1; }
expect_grep "ok    the tree declares 9.9.0-alpha.1 and the newest release is 9.9.0-alpha.1" \
  || { echo "    the pre-release record is not read as the newest release"; exit 1; }

# ---------------------------------------------------------------- 3. the final is above its pre-release
declare_version 9.9.0
g add -A; g commit -qm "the bump to the final"
expect_exit 0 gate || { echo "    a tree declaring 9.9.0 was called behind its own pre-release"; exit 1; }
expect_grep "ok    the tree declares 9.9.0 and the newest release is 9.9.0-alpha.1" || exit 1

# ---------------------------------------------------------------- 4. a published pre-release counts
declare_version "$V"
g add -A; g commit -qm "a tree that never made the bump"
expect_exit 10 gate || { echo "    a tree behind a published pre-release passed"; exit 1; }
expect_grep "FAIL  the tree declares $V and 9.9.0-alpha.1 is already published" || exit 1
echo "    a published pre-release owes its record and counts as published; another suffix is not a release; 9.9.0 is above 9.9.0-alpha.1"
