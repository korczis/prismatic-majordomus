# majordomus-covers: none
# The first release said to read a layer's written_for is one that does (I1980).
#
# `release bump` stamps this repository's own manifest only once the last release reads the
# key, and WRITTEN_FOR_READ_SINCE (release/version.rs) says which release that is. The constant
# is typed by hand and a release's version is measured at the bump: a constant naming a
# release that shipped without the reader would stamp a layer the installed tool refuses, and
# refuse this repository to every session running it. scripts/ci/release-check (4b) holds the
# constant to what git and the tree say, and this case drives each arm against a probe
# repository with its own tags and records.
. "$ROOT/test/lib.sh"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj916.XXXXXX")"; trap 'rm -rf "$S"' EXIT
P="$S/probe"; MODULE=apps/majordomus-cli/src/release/version.rs
mkdir -p "$P/scripts/ci" "$P/apps/majordomus-cli/src/release" "$P/share" "$P/.ai/repo/releases"
cp "$ROOT/scripts/ci/release-check" "$P/scripts/ci/"
cp "$ROOT/scripts/release-version" "$P/scripts/"
cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$ROOT/apps/majordomus-cli/Cargo.lock" "$P/apps/majordomus-cli/"
cp "$ROOT/$MODULE" "$P/$MODULE"
cp -R "$ROOT/bin" "$ROOT/lib" "$P/"
V="$("$ROOT/scripts/release-version")"
printf 'version=%s\n' "$V" > "$P/share/version.txt"
major="${V%%.*}"
ahead="$((major + 1)).0.0"; further="$((major + 2)).0.0"

# the constant, and the version every file of the probe states, set to what an arm needs
constant() { sed -i.bak "s/^pub const WRITTEN_FOR_READ_SINCE: &str = \".*\";/pub const WRITTEN_FOR_READ_SINCE: \&str = \"$1\";/" "$P/$MODULE" && rm -f "$P/$MODULE.bak"; }
declare_version() {
  sed -i.bak "s/^version = \"$V\"/version = \"$1\"/" "$P/apps/majordomus-cli/Cargo.toml" && rm -f "$P/apps/majordomus-cli/Cargo.toml.bak"
  awk -v v="$1" '/^name = "majordomus-cli"$/{f=1;print;next} f&&/^version = /{print "version = \"" v "\"";f=0;next} {print}' \
    "$P/apps/majordomus-cli/Cargo.lock" > "$S/lock" && mv "$S/lock" "$P/apps/majordomus-cli/Cargo.lock"
  printf 'version=%s\n' "$1" > "$P/share/version.txt"
}
record() { printf 'schema: release/v1\nversion: "%s"\ntag: v%s\nchannel: stable\n' "$1" "$1" > "$P/.ai/repo/releases/v$1.yaml"; }
gate() { ( cd "$P" && ./scripts/ci/release-check > "$S/out" 2>&1; echo $? > "$S/code" ) || true; }
said() { grep -q -- "$1" "$S/out" || { echo "    the gate did not say: $1"; cat "$S/out"; exit 1; }; }
unsaid() { ! grep -q -- "$1" "$S/out" || { echo "    the gate said: $1"; cat "$S/out"; exit 1; }; }

record "$V"
( cd "$P" && git init -q . && git config user.email t@example.com && git config user.name T \
  && git add -A >/dev/null && git commit -qm "the last release" && git tag "v$V" )

echo "    not released yet: the constant is ahead of every release, and the tree is clean"
constant "$ahead"
gate
[ "$(cat "$S/code")" = 0 ] || { echo "    a constant ahead of every release was refused:"; cat "$S/out"; exit 1; }
said "WRITTEN_FOR_READ_SINCE is $ahead: not released yet, and ahead of the newest release $V"

echo "    a constant that is not ahead of the newest release, with no tag of its own, is refused"
constant "0.0.0"
gate
[ "$(cat "$S/code")" = 10 ] || { echo "    a constant behind the releases passed:"; cat "$S/out"; exit 1; }
said "FAIL  WRITTEN_FOR_READ_SINCE is 0.0.0, which is not ahead of the newest release $V"

echo "    the release being made is the reader's: a bump to another version refuses the constant"
constant "$ahead"; declare_version "$further"
gate
[ "$(cat "$S/code")" = 10 ] || { echo "    a bump past the constant passed:"; cat "$S/out"; exit 1; }
said "FAIL  the tree declares $further, the release being made, and WRITTEN_FOR_READ_SINCE is $ahead"
constant "$further"
gate
[ "$(cat "$S/code")" = 0 ] || { echo "    the constant set to the release being made was refused:"; cat "$S/out"; exit 1; }
unsaid "FAIL  the tree declares"

echo "    once that release is tagged, its tree must carry the reader"
record "$further"
( cd "$P" && git add -A >/dev/null && git commit -qm "the release that reads written_for" && git tag "v$further" )
gate
[ "$(cat "$S/code")" = 0 ] || { echo "    a tagged release that carries the reader was refused:"; cat "$S/out"; exit 1; }
said "v$further, the first release said to read written_for, carries the reader"

echo "    a constant naming a release that shipped without the reader is refused"
# the tag v$V was cut on a tree whose module names another release, so it carries the
# constant; a tag cut before the reader existed does not, which is the failure this arm is
( cd "$P" && git checkout -q "v$V" && sed -i.bak '/WRITTEN_FOR_READ_SINCE/d' "$MODULE" && rm -f "$MODULE.bak" \
  && git commit -qam "a release before the reader" && git tag -f "v$V" >/dev/null && git checkout -q - )
constant "$V"
gate
[ "$(cat "$S/code")" = 10 ] || { echo "    a release without the reader passed as the first that reads:"; cat "$S/out"; exit 1; }
said "FAIL  WRITTEN_FOR_READ_SINCE is $V and the tag v$V carries no reader of written_for"
