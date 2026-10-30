# claims: release-archive-shape
# A release names its archive from the build matrix, without building a debug executable to ask.
#
# v0.26.0's macOS Intel build spent nine of its twenty-four minutes compiling a debug executable
# whose only use was `majordomus distribution artifact`: two names, the archive's and its root's.
# Every other runner paid the same two to five minutes ahead of the release build. The build
# matrix (docs/generated/distribution-matrix.json) is the generated projection of the same model
# and carries both names with the tag left as {tag}; the release workflow's plan job holds it to
# the model before any build. So scripts/release-package and scripts/release-verify take
# --names-from the matrix, and the workflow passes it to both. This case holds four things:
#
#   1. for every target, the matrix answers exactly the names the model derives
#   2. with --names-from, packing and verifying need no debug executable at all
#   3. a target the matrix does not publish is refused, exit 10, by both scripts
#   4. the workflow's build job passes --names-from to both
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v tar >/dev/null 2>&1 || skip "no tar"

MATRIX="$ROOT/docs/generated/distribution-matrix.json"
FILTER="$ROOT/scripts/lib/distribution-names.jq"
WF="$ROOT/.github/workflows/release.yml"
expect_file "$MATRIX"
expect_file "$FILTER"
TAG="v$("$ROOT/scripts/release-version")"

# --- 1. the matrix and the model name every target the same way ------------------------------
targets="$(jq -r '.include[].target' "$MATRIX")"
[ -n "$targets" ] || { echo "    the build matrix lists no target"; exit 1; }
for target in $targets; do
  model="$("$MJB" distribution --repo "$ROOT" artifact --target "$target" --tag "$TAG" --format text 2>/dev/null | sed -n '1,2p')"
  matrix="$(jq -r --arg target "$target" --arg tag "$TAG" -f "$FILTER" "$MATRIX")"
  [ -n "$model" ] || { echo "    the model derives no names for $target"; exit 1; }
  [ "$model" = "$matrix" ] || {
    echo "    for $target the matrix answers"; printf '%s\n' "$matrix" | sed 's/^/      /'
    echo "    and the model derives"; printf '%s\n' "$model" | sed 's/^/      /'
    exit 1
  }
done

# --- 2. with --names-from, nothing asks for a debug executable --------------------------------
# MAJORDOMUS_DIST_BIN names a path that does not exist, so a script that still asked the model
# dies on it; and the cargo first on PATH refuses to run, so a script that still built one
# fails loudly instead of quietly spending the minutes this case exists to save.
TARGET="$("$MJB" distribution --repo "$ROOT" build --format json 2>/dev/null \
  | sed -n 's/.*"target": "\([^"]*\)".*/\1/p' | head -n 1)"
[ -n "$TARGET" ] || { echo "    the model names no target for this machine"; exit 1; }
NOBIN="$T/no-such-debug-executable"
SHIM="$T/shim"; mkdir -p "$SHIM"
printf '#!/bin/sh\necho "cargo was asked to build: $*" >&2\nexit 97\n' > "$SHIM/cargo"
chmod 0755 "$SHIM/cargo"
OUT="$T/dist"; mkdir -p "$OUT"
expect_exit 0 env PATH="$SHIM:$PATH" MAJORDOMUS_DIST_BIN="$NOBIN" MAJORDOMUS_BIN="$MJB" \
  "$ROOT/scripts/release-package" --target "$TARGET" --tag "$TAG" --out "$OUT" --no-build \
  --names-from "$MATRIX"
name="$(jq -r --arg target "$TARGET" --arg tag "$TAG" -f "$FILTER" "$MATRIX" | sed -n 1p)"
expect_file "$OUT/$name"
expect_exit 0 env PATH="$SHIM:$PATH" MAJORDOMUS_DIST_BIN="$NOBIN" \
  "$ROOT/scripts/release-verify" --archive "$OUT/$name" --target "$TARGET" --tag "$TAG" \
  --names-from "$MATRIX"

# --- 3. a target the matrix does not publish is refused, not packed under an empty name -------
expect_exit 10 env MAJORDOMUS_DIST_BIN="$NOBIN" MAJORDOMUS_BIN="$MJB" \
  "$ROOT/scripts/release-package" --target no-such-target --tag "$TAG" --out "$T/none" --no-build \
  --names-from "$MATRIX"
expect_exit 10 env MAJORDOMUS_DIST_BIN="$NOBIN" \
  "$ROOT/scripts/release-verify" --archive "$OUT/$name" --target no-such-target --tag "$TAG" \
  --names-from "$MATRIX"

# --- 4. the release workflow names both archives from the matrix ------------------------------
build_job="$(awk '/^  build:/{p=1; next} /^  [a-z0-9_-]+:[ \t]*$/{p=0} p' "$WF")"
printf '%s\n' "$build_job" | grep -v '^[ \t]*#' | grep -A2 'scripts/release-package' \
  | grep -qF -- '--names-from docs/generated/distribution-matrix.json' \
  || { echo "    the build job packs without --names-from, so every runner builds a debug executable first"; exit 1; }
printf '%s\n' "$build_job" | grep -v '^[ \t]*#' | grep -A3 'scripts/release-verify' \
  | grep -qF -- '--names-from docs/generated/distribution-matrix.json' \
  || { echo "    the build job verifies without --names-from"; exit 1; }
exit 0
