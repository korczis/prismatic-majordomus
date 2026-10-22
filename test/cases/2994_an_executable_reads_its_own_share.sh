# majordomus-covers: none
# An installed program reads the share it was installed with, not another version's.
#
# MAJORDOMUS_SHARE is inherited. A terminal multiplexer started in one checkout hands it to
# every shell it opens, in every repository, and an installed 0.22.0 server then read the
# share of a 0.21.0 checkout: no share/ui/, so every screen was an empty state and no asset
# answered; kinds, schemas and allow-lists of the wrong version, so the index was degraded
# over keys its own schema knows. An owner found it from an adopting repository.
#
# What is decidable is decided, in both halves of the tool and by the same table: a named
# share that states another version than the program's, while the share beside the program
# states the program's own, yields to that one, and the program says so once. A share that
# states no version (a fixture, a hand-made directory) and a share of the same version are
# taken as named, which is what the variable is for.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- the packaged layout
DIST="$(mktemp -d "${TMPDIR:-/tmp}/mj2994.dist.XXXXXX")"
OTHER="$(mktemp -d "${TMPDIR:-/tmp}/mj2994.other.XXXXXX")"
trap 'rm -rf "$DIST" "$OTHER"' EXIT
mkdir -p "$DIST/bin" "$DIST/lib" "$DIST/libexec" "$DIST/share"
cp "$ROOT/bin/majordomus" "$DIST/bin/"
cp "$ROOT"/lib/* "$DIST/lib/"
cp -R "$ROOT/share/." "$DIST/share/"
chmod 0755 "$DIST/bin/majordomus"

own="$(sed -n 's/^version=//p' "$DIST/share/version.txt")"
[ -n "$own" ] || { echo "    the distribution states no version (scripts/derive)"; exit 1; }
[ "$own" != 0.0.1 ] || { echo "    the fixture's other version is this distribution's own"; exit 1; }

# Another distribution's share: complete, so that reading it would work, and marked, so that
# reading it would show — its skeleton is gone, and `init` from it cannot succeed.
other_states() {   # <version>, or nothing for a share that states none
  rm -rf "$OTHER/share"; mkdir -p "$OTHER/share"
  cp -R "$ROOT/share/." "$OTHER/share/"
  rm -rf "$OTHER/share/skeleton"
  if [ -n "${1:-}" ]; then printf '# a fixture\nversion=%s\n' "$1" > "$OTHER/share/version.txt"
  else rm -f "$OTHER/share/version.txt"; fi
}
MJD() { env -u MAJORDOMUS_BIN -u MAJORDOMUS_LAUNCHER MAJORDOMUS_SHARE="$OTHER/share" "$DIST/bin/majordomus" "$@"; }

cd "$T" || exit 1
case "$DIST/" in "$T/"*) echo "    the distribution must not sit inside the repository"; exit 1 ;; esac

# ---------------------------------------------------------------- the shell tool
# another version: the share beside the program is read, and it says so once
other_states 0.0.1
expect_exit 0 MJD init || exit 1
said="$(printf '%s\n' "$LAST_OUT" | grep -c 'MAJORDOMUS_SHARE names' || true)"
[ "$said" = 1 ] || { printf '    a share of another version must be named once, and was named %s time(s):\n%s\n' "$said" "$LAST_OUT"; exit 1; }
printf '%s\n' "$LAST_OUT" | grep -q "the share of version 0.0.1; this is version $own" \
  || { printf '    the line does not name both versions:\n%s\n' "$LAST_OUT"; exit 1; }
[ -f .ai/manifest.yaml ] || { echo "    init from the program's own share wrote no layer"; exit 1; }
[ ! -e "$OTHER/share/skeleton" ] || { echo "    the fixture's marker is gone"; exit 1; }

# no version stated: taken as named, and nothing is said about versions
git clean -fdxq
other_states ""
if MJD init >"$DIST/none.out" 2>&1; then
  echo "    a share that states no version was not taken as named"; exit 1
fi
grep -q "skeleton missing at $OTHER/share" "$DIST/none.out" \
  || { echo "    the named share was not the one read:"; cat "$DIST/none.out"; exit 1; }
! grep -q 'MAJORDOMUS_SHARE names' "$DIST/none.out" \
  || { echo "    a share that states no version was judged by its version"; exit 1; }

# the same version: taken as named
git clean -fdxq
other_states "$own"
if MJD init >"$DIST/same.out" 2>&1; then
  echo "    a share of the same version was not taken as named"; exit 1
fi
grep -q "skeleton missing at $OTHER/share" "$DIST/same.out" \
  || { echo "    the named share of the same version was not the one read:"; cat "$DIST/same.out"; exit 1; }

# ---------------------------------------------------------------- the Rust executable
# Copied, never linked: the executable finds its share beside the path it was started from,
# and a link resolves to the build directory on one platform and not on another.
RB="$(rust_bin)" || rust_bin_exit $?
stated="$("$RB" --version 2>/dev/null | sed -n 's/^majordomus \([0-9][0-9.]*\).*/\1/p')"
[ "$stated" = "$own" ] || {
  printf '    the executable is %s and share/version.txt states %s: one of them is stale (scripts/derive, or the MAJORDOMUS_BIN this run was handed)\n' "${stated:-unknown}" "$own"
  exit 1
}
cp "$RB" "$DIST/libexec/majordomus-cli"; chmod 0755 "$DIST/libexec/majordomus-cli"
git clean -fdxq
expect_exit 0 env -u MAJORDOMUS_BIN -u MAJORDOMUS_SHARE "$DIST/bin/majordomus" init || exit 1
RUN() { env -u MAJORDOMUS_BIN MAJORDOMUS_SHARE="$OTHER/share" "$DIST/libexec/majordomus-cli" env; }
dist_share="$(cd "$DIST/share" && pwd -P)"
other_share() { (cd "$OTHER/share" && pwd -P); }

other_states 0.0.1
expect_exit 0 RUN || exit 1
printf '%s\n' "$LAST_OUT" | grep -q "share directory share=$dist_share origin=\"installation\"" \
  || { printf '    the executable did not read the share beside it:\n%s\n' "$LAST_OUT"; exit 1; }
said="$(printf '%s\n' "$LAST_OUT" | grep -c 'names the share of another version' || true)"
[ "$said" = 1 ] || { printf '    the executable must say it once, and said it %s time(s):\n%s\n' "$said" "$LAST_OUT"; exit 1; }

other_states ""
expect_exit 0 RUN || exit 1
printf '%s\n' "$LAST_OUT" | grep -q "share directory share=$(other_share) origin=\"MAJORDOMUS_SHARE\"" \
  || { printf '    a share that states no version was not taken as named by the executable:\n%s\n' "$LAST_OUT"; exit 1; }

other_states "$own"
expect_exit 0 RUN || exit 1
printf '%s\n' "$LAST_OUT" | grep -q "share directory share=$(other_share) origin=\"MAJORDOMUS_SHARE\"" \
  || { printf '    a share of the same version was not taken as named by the executable:\n%s\n' "$LAST_OUT"; exit 1; }
