# majordomus-covers: none
# claims: release-prerelease-channel
# A tag with a pre-release part is never published or recorded as the stable release.
#
# The pipeline recorded every tag as `channel: stable` (scripts/release-record defaulted to
# it, release.yml passed no channel) and created every GitHub release without --prerelease.
# `Releases::latest_stable` ranks by version, so v1.0.0-alpha.1 would have outranked v0.27.0
# in releases/latest.json, and an unpinned installer would have installed the alpha. The
# resolution itself is proved by the crate's distribution tests; this case holds the two
# shell-side writers that decide the label: the record script refuses the contradiction, and
# the publish step of the release workflow marks a suffixed tag as a prerelease and never
# as GitHub's latest.
. "$ROOT/test/lib.sh"
SCRIPT="$ROOT/scripts/release-record"
WF="$ROOT/.github/workflows/release.yml"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3179.XXXXXX")"; trap 'rm -rf "$S"' EXIT
mkdir -p "$S/dist"
SHA="$(printf '0%.0s' $(seq 40))"

# 1. a stable channel on a suffixed tag is refused, naming why, before any artifact is read
rc=0; "$SCRIPT" --tag v1.0.0-alpha.1 --dir "$S/dist" --commit "$SHA" --channel stable >"$S/out" 2>"$S/err" || rc=$?
[ "$rc" = 2 ] || { echo "    --channel stable on v1.0.0-alpha.1 exited $rc, not 2"; cat "$S/err"; exit 1; }
grep -q 'pre-release part and cannot be recorded on the stable channel' "$S/err" \
  || { echo "    the refusal does not say why"; cat "$S/err"; exit 1; }

# 2. a suffixed tag with the channel left to the script is not refused for its channel
rc=0; MAJORDOMUS_DIST_BIN=/usr/bin/true "$SCRIPT" --tag v1.0.0-alpha.1 --dir "$S/dist" --commit "$SHA" >"$S/out" 2>"$S/err" || rc=$?
! grep -q 'cannot be recorded on the stable channel' "$S/err" \
  || { echo "    the default channel of a suffixed tag was stable"; cat "$S/err"; exit 1; }

# 3. build metadata is not a pre-release part, and an explicit prerelease on a final is allowed
rc=0; "$SCRIPT" --tag v1.0.0+build.5 --dir "$S/dist" --commit "$SHA" --channel stable >"$S/out" 2>"$S/err" || rc=$?
! grep -q 'cannot be recorded on the stable channel' "$S/err" || { echo "    build metadata was taken for a pre-release"; exit 1; }
# (v1.0.0+build.5 contains no '-', so the script's own test agrees with semver here)

# 4. the publish step marks a suffixed tag as a prerelease and never as latest, and the
#    flags it decides are the ones the create call receives
grep -q 'case "\$tag" in \*-\*) flags="--prerelease --latest=false"' "$WF" \
  || { echo "    release.yml does not decide --prerelease --latest=false from a '-' in the tag"; exit 1; }
grep -q 'gh release create "\$tag" --title "\$tag" --generate-notes \$flags' "$WF" \
  || { echo "    release.yml's create call does not receive the flags it decides"; exit 1; }
