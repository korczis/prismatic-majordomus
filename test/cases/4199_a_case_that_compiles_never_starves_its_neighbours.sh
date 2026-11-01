# majordomus-covers: none
# A case that compiles the crate does not starve the cases beside it (I4199). The suite runs
# four cases at a time on CI's four-core runner, and cargo takes every core it can get. On
# train 5, case 431 spent its whole 609 s beside 77_rust_evidence's doc-example and benchmark
# builds and was killed at its 600 s bound; alone it takes about a minute. So a case whose
# script runs `cargo test`, `cargo bench` or `cargo build` on a manifest either runs alone
# (`# majordomus-exclusive:`) or caps cargo on that line (`CARGO_BUILD_JOBS=N`). A cargo
# command that is only text, such as a fixture written with printf or a pattern a check greps
# for, runs nothing and is not judged. The rule holds over every case here, and a fixture
# shows it catches the case that breaks it and passes both remedies.
. "$ROOT/test/lib.sh"

# One line per case in the directory that compiles in the parallel phase with cargo uncapped.
starving() {
  local f
  for f in "$1"/*.sh; do
    grep -q '^# majordomus-exclusive:' "$f" && continue
    awk -v f="$(basename "$f")" '
      /^[[:space:]]*#/ { next }
      /^[[:space:]]*([A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+)*cargo[[:space:]]+(test|bench|build)([[:space:]]|$)/ && /--manifest-path/ {
        if ($0 !~ /CARGO_BUILD_JOBS=/) print f ":" NR ": " $0
      }' "$f"
  done
}

found="$(starving "$ROOT/test/cases")"
if [ -n "$found" ]; then
  printf '%s\n' "$found" | sed 's/^/    /'
  echo "    a case compiles the crate in the parallel phase with every core cargo can take:"
  echo "    declare '# majordomus-exclusive: <reason>' or cap that line with CARGO_BUILD_JOBS=N"
  exit 1
fi

# The rule is not vacuous: the case that starved 431's neighbourhood runs alone ...
grep -q '^# majordomus-exclusive:' "$ROOT/test/cases/77_rust_evidence.sh" \
  || { echo "    77_rust_evidence no longer runs alone, and it compiles with every core"; exit 1; }

# ... and on a fixture it catches exactly the uncapped compile, while the capped one, the one
# that runs alone and cargo that is only text all pass.
F="$T/cases"
mkdir -p "$F"
# the fixtures carry $M and $FX literally: they are scripts this case writes, not commands it runs
# shellcheck disable=SC2016
{
printf '%s\n' '# majordomus-covers: none' 'RUSTFLAGS= cargo test -q --manifest-path "$M" --doc' > "$F/1_uncapped.sh"
printf '%s\n' '# majordomus-covers: none' 'CARGO_BUILD_JOBS=2 cargo test --manifest-path "$M" --test x' > "$F/2_capped.sh"
printf '%s\n' '# majordomus-exclusive: compiles' 'cargo bench --no-run --manifest-path "$M"' > "$F/3_alone.sh"
printf '%s\n' '# majordomus-covers: none' "printf 'cargo build --release --manifest-path x\n' >> \"\$FX/.envrc\"" \
  '# cargo test --manifest-path x' "expect_grep 'cargo build --manifest-path'" > "$F/4_text.sh"
}
got="$(starving "$F")"
if [ "$(printf '%s\n' "$got" | grep -c .)" != 1 ] || ! printf '%s\n' "$got" | grep -q '^1_uncapped.sh:2: '; then
  echo "    the rule did not catch exactly the uncapped compile:"
  printf '%s\n' "$got" | sed 's/^/      /'
  exit 1
fi
