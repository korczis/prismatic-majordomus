# majordomus-covers: none
# The entry adapter decides what it always decided, with fewer processes.
#
# On 2026-09-15 bin/majordomus-env was measured running ten processes before it handed
# over to the executable — dirname(1), a subshell per `$(...)`, a nested subshell inside
# mj_rust_bin, find piped into head — and under a load average of 90 that shell half cost
# more wall time than the executable's own fast resolution. The cut replaced each capture
# with parameter expansion or lib/rust_bin.sh's `_into` twins, and `| head -n 1` with
# find's own `-quit`. A cut like that is only worth anything if it decides nothing
# differently, so this case holds the new forms to the old ones:
#
#   1. every `_into` form assigns exactly what its printing form prints, across the
#      variables that steer the answer
#   2. mj_rust_stale gives the verdict of the `find ... | head -n 1` it replaced, on each
#      shape of tree that verdict depends on: a newer file deep in src/, a newer Cargo.lock
#      or Cargo.toml alone, a directory touched with no file changed, nothing newer, and no
#      executable at all
#   3. the adapter resolves the same root, executable and share however it is invoked —
#      relative, absolute, from its own directory, through PATH, through `..` — as the
#      `cd "$(dirname "$0")" && pwd` it no longer runs
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_BIN MAJORDOMUS_BUILD_PROFILE CARGO_TARGET_DIR CARGO_BUILD_TARGET_DIR MAJORDOMUS_SHARE

mkdir -p "$T/bin" "$T/lib" "$T/share"
cp "$ROOT/bin/majordomus-env" "$T/bin/"
cp "$ROOT/lib/rust_bin.sh" "$T/lib/"
: > "$T/share/kinds.yaml"
. "$T/lib/rust_bin.sh"

# --- 1. the `_into` forms and the printing forms are one decision
same() { # <label> <printed> <assigned>
  [ "$2" = "$3" ] || { echo "    $1: printed '$2', assigned '$3'"; exit 1; }
}
check_forms() { # <label>
  mj_rust_bin_into got "$T"; same "$1 mj_rust_bin" "$(mj_rust_bin "$T")" "$got"
  mj_rust_share_into got "$T"; same "$1 mj_rust_share" "$(mj_rust_share "$T")" "$got"
  mj_cargo_target_dir_into got "$T/apps/majordomus-cli"
  same "$1 mj_cargo_target_dir" "$(mj_cargo_target_dir "$T/apps/majordomus-cli")" "$got"
}
check_forms "defaults"
[ "$(mj_rust_bin "$T")" = "$T/apps/majordomus-cli/target/debug/majordomus" ] \
  || { echo "    the default executable moved: $(mj_rust_bin "$T")"; exit 1; }
[ "$(mj_rust_share "$T")" = "$T/share" ] || { echo "    the shipped share was not found"; exit 1; }
MAJORDOMUS_BUILD_PROFILE=release check_forms "release profile"
CARGO_TARGET_DIR="$T/elsewhere" check_forms "CARGO_TARGET_DIR"
CARGO_BUILD_TARGET_DIR="$T/other" check_forms "CARGO_BUILD_TARGET_DIR"
MAJORDOMUS_BIN="$T/named" check_forms "MAJORDOMUS_BIN"
MAJORDOMUS_SHARE="$T/named-share" check_forms "MAJORDOMUS_SHARE"
mkdir -p "$T/libexec"; printf '#!/bin/sh\n' > "$T/libexec/majordomus-cli"; chmod +x "$T/libexec/majordomus-cli"
check_forms "a shipped executable"
rm -rf "$T/libexec"
rm "$T/share/kinds.yaml"
check_forms "no share"
[ -z "$(mj_rust_share "$T")" ] || { echo "    a tree without a share printed one"; exit 1; }
: > "$T/share/kinds.yaml"

# --- 2. the staleness verdict is the one the pipeline it replaced gave
crate="$T/apps/majordomus-cli"
mkdir -p "$crate/src/a/b/c" "$crate/target/debug"
printf 'fn main() {}\n' > "$crate/src/main.rs"
printf 'mod x;\n' > "$crate/src/a/b/c/deep.rs"
printf '[package]\n' > "$crate/Cargo.toml"
printf '# lock\n' > "$crate/Cargo.lock"
bin="$crate/target/debug/majordomus"
printf '#!/bin/sh\n' > "$bin"; chmod +x "$bin"

reference() { # the predicate as it stood before the cut
  [ -x "$2" ] || return 0
  [ -n "$(find "$1/apps/majordomus-cli/src" "$1/apps/majordomus-cli/Cargo.toml" \
    "$1/apps/majordomus-cli/Cargo.lock" -type f -newer "$2" 2>/dev/null | head -n 1)" ]
}
agree() { # <label> <expected: stale|current>
  # cases run under `bash -eu`: a verdict is read in a condition, never from a bare `$?`
  if mj_rust_stale "$T" "$bin"; then now=0; else now=1; fi
  if reference "$T" "$bin"; then was=0; else was=1; fi
  [ "$now" = "$was" ] || { echo "    $1: mj_rust_stale says $now, the pipeline it replaced says $was"; exit 1; }
  want=1; [ "$2" = stale ] && want=0
  [ "$now" = "$want" ] || { echo "    $1: expected $2, got exit $now"; exit 1; }
}
# mtimes are set explicitly, a minute apart, so no filesystem's timestamp granularity decides
set_old() { touch -t 202601010000 "$crate/src/main.rs" "$crate/src/a/b/c/deep.rs" \
  "$crate/Cargo.toml" "$crate/Cargo.lock" "$crate/src" "$crate/src/a" "$crate/src/a/b" "$crate/src/a/b/c"; }
set_old; touch -t 202601010001 "$bin"
agree "nothing newer" current
touch -t 202601010002 "$crate/src/a/b/c/deep.rs"; agree "a newer file three directories down" stale
set_old; touch -t 202601010002 "$crate/Cargo.lock"; agree "a newer Cargo.lock alone" stale
set_old; touch -t 202601010002 "$crate/Cargo.toml"; agree "a newer Cargo.toml alone" stale
set_old; touch -t 202601010002 "$crate/src/a/b"; agree "a touched directory, no file changed" current
set_old; printf 'fn f() {}\n' > "$crate/src/a/new.rs"; touch -t 202601010002 "$crate/src/a/new.rs"
agree "a newly added source" stale
rm "$crate/src/a/new.rs"; set_old
rm "$bin"; mj_rust_stale "$T" "$bin" || { echo "    a missing executable is not stale"; exit 1; }
MAJORDOMUS_BIN="$bin" mj_rust_stale "$T" "$bin" \
  || { echo "    a missing MAJORDOMUS_BIN must still be stale"; exit 1; }
printf '#!/bin/sh\n' > "$bin"; chmod +x "$bin"; touch -t 202601010000 "$bin"
MAJORDOMUS_BIN="$bin" mj_rust_stale "$T" "$bin" \
  && { echo "    an explicitly named executable was judged stale"; exit 1; }

# --- 3. however the adapter is invoked, it resolves what the dirname-and-cd form resolved
# The executable is a stub that prints what the adapter handed it, so the comparison is of
# what would really run, not of a copy of the adapter's logic.
cat > "$bin" <<'STUB'
#!/bin/sh
printf '%s|%s|%s\n' "$0" "${MAJORDOMUS_SHARE:-}" "${MAJORDOMUS_RUNTIME:-}"
STUB
chmod +x "$bin"
expected="$bin|$T/share|"
real_t="$(cd "$T" && pwd)"
for spelling in rel abs dot path dotdot; do
  case "$spelling" in
    rel) got="$(cd "$T" && bin/majordomus-env enter 2>/dev/null)" ;;
    abs) got="$(cd / && "$T/bin/majordomus-env" enter 2>/dev/null)" ;;
    dot) got="$(cd "$T/bin" && ./majordomus-env enter 2>/dev/null)" ;;
    path) got="$(cd "$T/share" && PATH="$T/bin:$PATH" majordomus-env enter 2>/dev/null)" ;;
    dotdot) got="$(cd "$T" && bin/../bin/majordomus-env enter 2>/dev/null)" ;;
  esac
  case "$got" in
    "$expected"|"$real_t/apps/majordomus-cli/target/debug/majordomus|$real_t/share|") ;;
    *) echo "    invoked as '$spelling', the adapter ran '$got', expected '$expected'"; exit 1 ;;
  esac
done

# and a stale executable still turns the runtime off, as it did
touch -t 202601010001 "$bin"; touch "$crate/src/main.rs"
got="$(cd "$T" && bin/majordomus-env enter 2>/dev/null)"
case "$got" in *"|off") ;; *) echo "    a stale executable ensured a runtime: '$got'"; exit 1 ;; esac
exit 0
