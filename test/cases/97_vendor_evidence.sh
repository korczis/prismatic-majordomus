# A rule's `x-majordomus.tests` and `x-majordomus.claims` are the vendor's evidence for its
# own rule. They name paths in the vendor's source tree — test/cases/NN_*.sh, the claims
# ledger, the CI workflow — and `scripts/release-package` ships none of that: the archive is
# bin/ lib/ libexec/ share/ LICENSE and a RELEASE.json stamp. Checking those paths from
# inside a packaged distribution therefore reported, to every repository that adopted
# Majordomus, that it was missing 107 files it was never given (docs/ADOPTION_FIRST_RUN.md).
#
# The fix skips the evidence half of the check when the distribution is packaged, and the
# thing that makes it a fix rather than a second defect is this case. "Skip when the file is
# missing" would be the same diff and a check that can never fail anywhere, including at
# home, and a package could then ship rules whose evidence had rotted with nothing to say
# so. So both directions are asserted here:
#
#   source tree      -> the evidence is read, and a rotted path is still a failure
#   packaged archive -> the evidence is not read, and doctor says so by name
#
# The discriminator is the RELEASE.json stamp, never the presence of the evidence itself.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- the fixture
# A distribution staged the way scripts/release-package stages one (its lines 65-97), so
# that this case measures the tree a person actually installs rather than a mock of it. The
# two trees differ in exactly one file, which is the point.
stage_dist() {   # <dir> — bin/ lib/ libexec/ share/ LICENSE, no test/ and no docs/
  local d="$1"
  mkdir -p "$d/bin" "$d/lib" "$d/libexec" "$d/share"
  cp "$ROOT/bin/majordomus" "$ROOT/bin/majordomus-mcp" "$ROOT/bin/majordomus-cli" "$d/bin/"
  cp "$ROOT"/lib/* "$d/lib/"
  cp -R "$ROOT/share/." "$d/share/"
  cp "$ROOT/LICENSE" "$d/LICENSE"
  chmod 0755 "$d/bin/majordomus" "$d/bin/majordomus-mcp" "$d/bin/majordomus-cli"
}
stamp_release() {   # <dir> — what release-package writes, and release-verify refuses without
  cat > "$1/RELEASE.json" <<JSON
{
  "schema": "majordomus/build/v1",
  "version": "0.0.0",
  "tag": "v0.0.0",
  "target": "test",
  "commit": "test",
  "built_at": "1970-01-01T00:00:00Z"
}
JSON
}

# An adopted repository: what `majordomus init` makes of an ordinary project. $T is already
# a git repository with one commit.
printf 'print("hi")\n' > "$T/app.py"
printf '# Sample\n\nA small project. See AGENTS.md.\n' > "$T/README.md"
git -C "$T" add -A >/dev/null 2>&1
git -C "$T" commit -q -m project >/dev/null 2>&1

PKG="$T/dist-packaged"; stage_dist "$PKG"; stamp_release "$PKG"
SRC="$T/dist-source";   stage_dist "$SRC"          # identical but for the stamp

# ---------------------------------------------------------------- packaged: not read, said so
cd "$T" || exit 1
expect_exit 0 env -u MAJORDOMUS_SHARE "$PKG/bin/majordomus" init || exit 1
expect_exit 0 env -u MAJORDOMUS_SHARE "$PKG/bin/majordomus" update || exit 1

env -u MAJORDOMUS_SHARE "$PKG/bin/majordomus" doctor > "$T/packaged.txt" 2>&1 || true

# Not one of the vendor's evidence paths is reported against this repository.
if grep -qE 'test test/cases/.* does not exist' "$T/packaged.txt"; then
  echo "    a packaged distribution reported the vendor's test cases as missing from the adopting repository"
  grep -E 'test test/cases/' "$T/packaged.txt" | head -3 | sed 's/^/      /'
  exit 1
fi
if grep -q 'which is not in docs/CLAIMS.yaml' "$T/packaged.txt"; then
  echo "    a packaged distribution reported the vendor's claims ledger against the adopting repository"
  grep 'docs/CLAIMS.yaml' "$T/packaged.txt" | head -3 | sed 's/^/      /'
  exit 1
fi
if grep -q 'no .github/workflows/validate.yml' "$T/packaged.txt"; then
  echo "    a packaged distribution demanded the vendor's own CI workflow of the adopting repository"
  exit 1
fi
if grep -q 'test/run.sh does not glob' "$T/packaged.txt"; then
  echo "    a packaged distribution demanded the vendor's own test runner of the adopting repository"
  exit 1
fi

# Silence would be indistinguishable from a check that passed, so it is named.
expect_grep 'INFO +doctrine +vendor evidence' "$T/packaged.txt" || {
  echo "    the skip is silent: a reader cannot tell it from evidence that was checked and passed"
  exit 1
}

# The wiring half is not part of the bargain: it reads lib/, which every archive ships, and
# it still runs and still reports.
expect_grep 'doctrine +[0-9]+ doctrines' "$T/packaged.txt" || {
  echo "    the wiring half of the check stopped reporting in a packaged distribution"
  exit 1
}

# ---------------------------------------------------------------- source tree: still read
# The same tree without the stamp is a source tree as far as the check is concerned, and
# there the evidence is read. This one has no test/ and no docs/CLAIMS.yaml, so it must
# fail, loudly, over exactly the paths the packaged run stayed quiet about. This assertion
# is what a lazy "skip when the path is missing" implementation would break.
rm -rf "$T/repo2"; mkdir -p "$T/repo2"; cd "$T/repo2" || exit 1
git init -q . && git config user.email t@example.com && git config user.name t
git commit -q --allow-empty -m init
expect_exit 0 env -u MAJORDOMUS_SHARE "$SRC/bin/majordomus" init || exit 1
expect_exit 0 env -u MAJORDOMUS_SHARE "$SRC/bin/majordomus" update || exit 1

env -u MAJORDOMUS_SHARE "$SRC/bin/majordomus" doctor > "$T/source.txt" 2>&1 || true

expect_grep 'test test/cases/.* does not exist' "$T/source.txt" || {
  echo "    an unstamped tree did not read the evidence: the skip fires everywhere, which is the defect it replaced"
  exit 1
}
expect_grep 'which is not in docs/CLAIMS.yaml' "$T/source.txt" || {
  echo "    an unstamped tree did not check the claims: the skip fires everywhere"
  exit 1
}
expect_no_grep 'INFO +doctrine +vendor evidence' "$T/source.txt" || {
  echo "    an unstamped tree announced the packaged skip"
  exit 1
}

# ---------------------------------------------------------------- and one file decides it
# Stamping the source tree flips it, with nothing else changed. If the two runs above ever
# agree, the discriminator has stopped discriminating.
stamp_release "$SRC"
env -u MAJORDOMUS_SHARE "$SRC/bin/majordomus" doctor > "$T/source-stamped.txt" 2>&1 || true
expect_no_grep 'test test/cases/.* does not exist' "$T/source-stamped.txt" || {
  echo "    RELEASE.json did not change the reading; the stamp is not what decides it"
  exit 1
}

# ---------------------------------------------------------------- the sibling command
# `doctrine status` counted the same evidence and exited 10 in every adopting repository for
# the same reason. It reads the count from MJ_HOME exactly as the validator did.
cd "$T" || exit 1
expect_exit 0 env -u MAJORDOMUS_SHARE "$PKG/bin/majordomus" doctrine status || {
  echo "    doctrine status still refuses an adopting repository over the vendor's test cases"
  exit 1
}
expect_grep 'without a test file: +not readable here' || {
  echo "    doctrine status hides that it could not read the evidence"
  exit 1
}
env -u MAJORDOMUS_SHARE "$PKG/bin/majordomus" doctrine status --json > "$T/status.json" 2>&1 || true
expect_grep '"evidence_readable": *false' "$T/status.json" || {
  echo "    the JSON projection of doctrine status does not say the evidence was unreadable"
  exit 1
}

echo "    ok: vendor evidence is read where it lives and skipped by name where it does not"
