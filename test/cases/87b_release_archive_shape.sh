# The shape of a release archive, and the guard that refuses a malformed one.
#
# This case exists because of release v0.2.0, which was never published. scripts/release-package
# built its tar from a sorted list of every path in the staged tree; the invocation did not yet
# pass --no-recursion, so GNU tar walked the list *and* recursed into every directory on it and
# archived each file twice. GNU tar writes the second copy of a path as a hard link. Hard links
# are what release-verify refuses by name — 931 of them — so the linux-x86_64-gnu build exited 10,
# fail-fast cancelled the other five targets, publish and smoke never ran, no release record was
# written, no latest.json was ever published, and the advertised one-line installer answered
# "no stable release is published yet" for every user who tried it.
#
# The invocation was fixed. What this case holds is the property, not the flag: an archive this
# packer writes carries every path once and carries nothing but regular files and directories,
# and a packer that violates that is stopped where it packs rather than where it publishes.
#
# Proves claim release-archive-shape (docs/CLAIMS.yaml), whose `test:` names this case.
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v tar >/dev/null 2>&1 || { echo "    skip: no tar"; exit 0; }

TAG="v$("$ROOT/scripts/release-version")"
TARGET="$("$MJB" distribution --repo "$ROOT" build --format json 2>/dev/null \
  | sed -n 's/.*"target": "\([^"]*\)".*/\1/p' | head -n 1)"
[ -n "$TARGET" ] || { echo "    the model names no target for this machine"; exit 1; }

# The executable to pack. A release build for this host is what the case wants and almost never
# what the tree has, so the debug executable stands in: this case is about the archive's shape,
# and the shape does not depend on the profile.
OUT="$T/dist"; mkdir -p "$OUT"
pack() { env MAJORDOMUS_BIN="$MJB" MAJORDOMUS_DIST_BIN="$MJB" "$@" \
  "$ROOT/scripts/release-package" --target "$TARGET" --tag "$TAG" --out "$OUT" --no-build 2>&1; }

archive="$(pack env)" || { echo "    the packer failed on a tree it should pack:"; printf '%s\n' "$archive" | sed 's/^/      /'; exit 1; }
archive="$(printf '%s\n' "$archive" | tail -n 1)"
expect_file "$archive"

# --- every entry is a regular file or a directory: no hard link, no symlink, no device ------
listing="$T/listing"
tar -tzvf "$archive" > "$listing" || { echo "    the archive does not read back as a gzipped tar"; exit 1; }
irregular="$(sed -n '/^[^-d]/p' "$listing" | head -n 5)"
[ -z "$irregular" ] || {
  echo "    the archive carries an entry that is neither a regular file nor a directory:"
  printf '%s\n' "$irregular" | sed 's/^/      /'
  echo "    this is the v0.2.0 failure: a recursing tar writes the second copy of a path as a hard link"
  exit 1
}

# --- and it carries every path exactly once -------------------------------------------------
duplicated="$(sed 's/^.*[0-9][0-9]:[0-9][0-9] //' "$listing" | sed 's:/$::' | LC_ALL=C sort | uniq -d | head -n 5)"
[ -z "$duplicated" ] || {
  echo "    the archive carries a path more than once:"
  printf '%s\n' "$duplicated" | sed 's/^/      /'
  exit 1
}
entries="$(wc -l < "$listing" | tr -d ' ')"
[ "$entries" -gt 100 ] || { echo "    the archive holds only $entries entries; that is not a packed tool"; exit 1; }

# --- the verifier agrees, on the archive that was actually written ---------------------------
env MAJORDOMUS_DIST_BIN="$MJB" "$ROOT/scripts/release-verify" \
  --archive "$archive" --target "$TARGET" --tag "$TAG" > "$T/verify.log" 2>&1 \
  || { echo "    release-verify refused an archive release-package wrote:"; tail -20 "$T/verify.log" | sed 's/^/      /'; exit 1; }

# --- a packer that recurses is stopped where it packs, not where it publishes -----------------
# The failure is reproduced at its source: a tar on PATH that drops --no-recursion is exactly
# the packer v0.2.0 had. release-package must refuse its own output and leave no archive behind,
# so that no such archive can reach a checksum, a release record, or a user.
SHIM="$T/shim"; mkdir -p "$SHIM"
real_tar="$(command -v tar)"
cat > "$SHIM/tar" <<SHIMEOF
#!/bin/sh
# the v0.2.0 packer: every --no-recursion silently dropped, so tar walks the list and recurses
args=''
for a in "\$@"; do
  [ "\$a" = "--no-recursion" ] && continue
  args="\$args \$(printf '%s' "\$a" | sed "s/'/'\\\\\\\\''/g; s/^/'/; s/\$/'/")"
done
eval exec "$real_tar" \$args
SHIMEOF
chmod 0755 "$SHIM/tar"

rm -f "$OUT"/*.tar.gz
out="$(pack env PATH="$SHIM:$PATH")" && {
  echo "    the packer accepted a recursing tar; the v0.2.0 archive would be built again"
  printf '%s\n' "$out" | sed 's/^/      /'
  exit 1
}
printf '%s\n' "$out" | grep -q 'more than once\|neither a regular file nor a directory' || {
  echo "    the packer failed, but not because it caught the duplicated entries:"
  printf '%s\n' "$out" | sed 's/^/      /'
  exit 1
}
ls "$OUT"/*.tar.gz >/dev/null 2>&1 && {
  echo "    the packer left the malformed archive on disk; a later step could publish it"
  exit 1
}
exit 0
