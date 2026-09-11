# majordomus-covers: doctor doctrine
# The tool in the form a person installs it, supervising a repository.
#
# Every other case in this suite runs bin/majordomus from this checkout, where the tool
# distribution and the repository under test are the same directory: a path that resolves
# for one resolves for the other, and a check that reads across the two cannot be told
# apart from a check that reads one. A release archive is a different tree — bin, lib,
# libexec, share, LICENSE and RELEASE.json, and none of the development surface: no
# test/, no docs/CLAIMS.yaml, no .github/.
#
# doctor read that surface to reconcile each doctrine's test and claim, so from a release
# it failed 109 doctrines and exited 12 in every repository, this one included, for the one
# reason that said nothing about the repository being supervised. The advertised first run
# — install, init, update, doctor — could not go green for anyone who had not cloned this
# repository, and the suite could not see it because the suite never took a user's shape.
#
# What this case holds is that shape: the archive the packer writes, unpacked, run against
# a repository it has never seen.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v tar >/dev/null 2>&1 || { echo "    skip: no tar"; exit 0; }

# --- the artifact, packed the way a release is packed --------------------------------------
# Derived, never assembled by hand: a hand-made "release-shaped" directory is this case's
# own idea of the shape, and would keep passing on the day the packer's changed.
TAG="v$("$ROOT/scripts/release-version")"
TARGET="$("$MJB" distribution --repo "$ROOT" build --format json 2>/dev/null \
  | sed -n 's/.*"target": "\([^"]*\)".*/\1/p' | head -n 1)"
[ -n "$TARGET" ] || { echo "    the model names no target for this machine"; exit 1; }

OUT="$T/dist"; mkdir -p "$OUT"
archive="$(env MAJORDOMUS_BIN="$MJB" MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-package" \
  --target "$TARGET" --tag "$TAG" --out "$OUT" --no-build 2>&1)" \
  || { echo "    the packer failed on a tree it should pack:"; printf '%s\n' "$archive" | sed 's/^/      /'; exit 1; }
archive="$(printf '%s\n' "$archive" | tail -n 1)"
expect_file "$archive"

REL="$T/rel"; mkdir -p "$REL"
tar -xzf "$archive" -C "$REL" || { echo "    the archive does not unpack"; exit 1; }
# the archive carries its own versioned directory, the way the installer unpacks it under a
# versions/ prefix; find the executable rather than spelling that name out here
MJ="$(find "$REL" -type f -path '*/bin/majordomus' | head -n 1)"
[ -n "$MJ" ] && [ -x "$MJ" ] || { echo "    the unpacked archive has no executable bin/majordomus"; exit 1; }
REL="$(cd "$(dirname "$MJ")/.." && pwd)"

# --- it is the shape this case is about, and the shape is the archive's, not this case's ---
expect_file "$REL/RELEASE.json"
for d in test docs .github; do
  [ -e "$REL/$d" ] && { echo "    the archive now ships $d/; this case assumed it does not — check whether the packer changed on purpose"; exit 1; }
done

# --- a repository it has never seen --------------------------------------------------------
REPO="$T/repo"; mkdir -p "$REPO"
cd "$REPO" || exit 1
git init -q .
git config user.email t@example.com; git config user.name t
printf '# fixture\n\nThe agent bootstrap is AGENTS.md.\n' > README.md
"$MJ" init >/dev/null || { echo "    init failed from a release"; exit 1; }
"$MJ" update >/dev/null || { echo "    update failed from a release"; exit 1; }
mkdir -p .git/hooks
printf '#!/usr/bin/env bash\n%s doctor || exit $?\n' "$MJ" > .git/hooks/pre-commit
printf '#!/usr/bin/env bash\n%s finish --check || exit $?\n' "$MJ" > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push

# doctor passes, and the checks that could not be answered say so rather than failing
expect_exit 0 "$MJ" doctor
expect_no_grep 'FAIL'
expect_grep 'OK +doctrine +[0-9]+ doctrines .* validator, dispatch and propagation resolve'
expect_grep 'INFO +doctrine +proof surface .* did not run here'

# the counter does not turn the same absence into a number nobody can act on
expect_exit 0 "$MJ" doctrine status
expect_grep 'without a test file: +not shipped with this release'

# and the reads a worker actually makes still work from this form
expect_exit 0 "$MJ" doctrine list
expect_exit 0 "$MJ" rules list
expect_exit 0 "$MJ" context
expect_exit 0 "$MJ" watch

# --- the marker is what decides ------------------------------------------------------------
# Without RELEASE.json the same tree is a checkout whose proof surface was deleted, and that
# is a real failure. If this went green too, the fix would be "stop checking when the files
# are missing", which is the shape of a check that quietly stops being a check.
rm "$REL/RELEASE.json"
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL +doctrine .* does not exist'
