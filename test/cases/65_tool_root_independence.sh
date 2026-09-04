# The tool distribution is read-only and its location is not part of the repository's
# data. Running the same version from a checkout, from an unrelated absolute path, and
# through PATH must use the same repository files and write nothing into the distribution.
. "$ROOT/test/lib.sh"

# --- a second copy of the distribution, somewhere that is not the repository
DIST="$(mktemp -d "${TMPDIR:-/tmp}/mj-dist.XXXXXX")"
# the distribution as the design defines it: CLI, libraries, shared data, tests, docs, CI
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$ROOT/test" "$ROOT/docs" "$ROOT/.github" "$DIST/"
before="$(cd "$DIST" && find . -type f | LC_ALL=C sort | xargs shasum -a 256 | shasum -a 256)"

# --- an absolute path outside the repository initialises this repository, not itself
expect_exit 0 "$DIST/bin/majordomus" init
expect_exit 0 "$DIST/bin/majordomus" update
git add -A >/dev/null; git commit -qm install
[ -f CLAUDE.md ] || { echo "    update from an external distribution wrote no projection"; exit 1; }
[ ! -e "$DIST/.majordomus" ] && [ ! -e "$DIST/.ai" ] || { echo "    the distribution directory gained project data"; exit 1; }

# --- the checkout copy and the external copy read the same repository and agree
expect_exit 10 "$MJ" doctor
out_checkout="$LAST_OUT"
expect_exit 10 "$DIST/bin/majordomus" doctor
out_dist="$LAST_OUT"
a="$(printf '%s\n' "$out_checkout" | grep -E '^(OK|FAIL|WARN)' | sed 's/  \[reproduce:.*//' | sort)"
b="$(printf '%s\n' "$out_dist" | grep -E '^(OK|FAIL|WARN)' | sed 's/  \[reproduce:.*//' | sort)"
[ "$a" = "$b" ] || { echo "    two locations of one version disagree about one repository"; diff <(printf '%s\n' "$a") <(printf '%s\n' "$b"); exit 1; }

# --- through PATH, from another directory, with --repo naming the repository
elsewhere="$(mktemp -d "${TMPDIR:-/tmp}/mj-elsewhere.XXXXXX")"
( cd "$elsewhere" && PATH="$DIST/bin:$PATH" majordomus --repo "$T" start "path task" --scope docs ) > "$elsewhere/path.out" 2>&1 \
  || { echo "    the CLI on PATH could not start a task in --repo"; cat "$elsewhere/path.out"; exit 1; }
grep -q '^started t-' "$elsewhere/path.out" || { echo "    start through PATH did not report a task"; exit 1; }
expect_exit 0 "$MJ" check
rm -rf "$elsewhere"

# --- and the distribution is byte for byte what it was
after="$(cd "$DIST" && find . -type f | LC_ALL=C sort | xargs shasum -a 256 | shasum -a 256)"
[ "$before" = "$after" ] || { echo "    the distribution was written to; it must be usable read-only"; exit 1; }
rm -rf "$DIST"
