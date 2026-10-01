# majordomus-covers: none
# claims: rust-binary-artifact
# The Rust executable the rust gate built is published with its provenance, and a Rust case
# drives the executable MAJORDOMUS_BIN names instead of building one.
#
# A full `scripts/rust-check` builds and lints the whole crate, which is CI's cost and not this
# case's. So the repository's own scripts/rust-check runs here unmodified, copied into a
# fixture repository, with `cargo` on PATH replaced by a stand-in that records what it was
# asked and "builds" by placing the executable MAJORDOMUS_BIN names where cargo would. What is
# asserted is what the script owns: --artifact copies the executable the gate built, and
# majordomus-cli.json beside it names the commit, the target triple, the toolchain and the
# Cargo.lock digest, each compared with the fact it claims. rustc is the real one, since the
# triple and the toolchain are its answers; without rustc that half skips.
#
# Then the consumer half: the workflow uploads that directory under the triple's name, the
# suite jobs hand the built executable to the cases, rust_bin hands a case MAJORDOMUS_BIN
# without building, and no case builds the crate itself outside rust_bin.
. "$ROOT/test/lib.sh"
WF="$ROOT/.github/workflows/validate.yml"

RB="$(rust_bin)" || rust_bin_exit $?

# ------------------------------------------------------------------ the artifact and its manifest
if command -v rustc >/dev/null 2>&1; then
  F="$T/fixture"; STUB="$T/stub"; LOG="$T/cargo.log"
  mkdir -p "$F/scripts" "$F/lib" "$F/apps/majordomus-cli" "$STUB"
  cp "$ROOT/scripts/rust-check" "$ROOT/scripts/rust-coverage-threshold" "$ROOT/scripts/session-coverage-threshold" "$F/scripts/"
  cp "$ROOT/lib/rust_bin.sh" "$F/lib/"
  # the pinned toolchain, so the toolchain the manifest records is the one this repository builds with
  for p in rust-toolchain.toml rust-toolchain; do [ -f "$ROOT/$p" ] && cp "$ROOT/$p" "$F/"; done
  cp "$ROOT/apps/majordomus-cli/Cargo.toml" "$ROOT/apps/majordomus-cli/Cargo.lock" "$F/apps/majordomus-cli/"
  ( cd "$F" && git init -q . && git config user.email t@example.com && git config user.name t \
    && git add -A && git commit -qm fixture )
  # cargo: record every call; `build` places the executable where the target directory says;
  # `llvm-cov --version` fails, as on a machine without it
  cat > "$STUB/cargo" <<'SH'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$STUB_CARGO_LOG"
case "$1" in
  build) mkdir -p "$CARGO_TARGET_DIR/debug" && cp "$STUB_BUILT" "$CARGO_TARGET_DIR/debug/majordomus" ;;
  llvm-cov) exit 1 ;;
esac
exit 0
SH
  chmod +x "$STUB/cargo"
  rc=0
  # the stand-in shadows cargo only: rustc may be a version manager's shim that needs the rest of PATH
  env PATH="$STUB:$PATH" STUB_CARGO_LOG="$LOG" STUB_BUILT="$RB" \
      CARGO_TARGET_DIR="$T/target" bash "$F/scripts/rust-check" --integration --artifact "$T/dist" \
      > "$T/rust-check.out" 2>&1 || rc=$?
  [ "$rc" = 0 ] || { tail -20 "$T/rust-check.out"; echo "    scripts/rust-check --integration --artifact exited $rc"; exit 1; }
  grep -qx 'build --quiet' "$LOG" || { cat "$LOG"; echo "    scripts/rust-check did not build before publishing the artifact"; exit 1; }
  # the artifact is the executable the gate built, byte for byte
  [ -x "$T/dist/majordomus" ] || { echo "    the artifact directory holds no executable"; exit 1; }
  cmp -s "$T/dist/majordomus" "$T/target/debug/majordomus" \
    || { echo "    the published executable is not the one the gate built"; exit 1; }
  M="$T/dist/majordomus-cli.json"
  expect_file "$M"
  jq -e . "$M" >/dev/null || { cat "$M"; echo "    majordomus-cli.json is not JSON"; exit 1; }
  field() { jq -r --arg k "$1" '.[$k] // ""' "$M"; }
  want_commit="$(git -C "$F" rev-parse HEAD)"
  want_target="$(cd "$F/apps/majordomus-cli" && rustc -vV | sed -n 's/^host: //p')"
  want_rustc="$(cd "$F/apps/majordomus-cli" && rustc -V)"
  [ -n "$want_target" ] && [ -n "$want_rustc" ] || { echo "    rustc answered no host or version in the fixture"; exit 1; }
  want_lock="$(sha256_of_file "$F/apps/majordomus-cli/Cargo.lock")"
  [ "$(field commit)" = "$want_commit" ] || { echo "    the manifest names commit '$(field commit)', the tree is at $want_commit"; exit 1; }
  [ "$(field target)" = "$want_target" ] || { echo "    the manifest names target '$(field target)', rustc's host is $want_target"; exit 1; }
  [ "$(field rustc)" = "$want_rustc" ] || { echo "    the manifest names toolchain '$(field rustc)', rustc is '$want_rustc'"; exit 1; }
  [ "$(field cargo_lock_sha256)" = "$want_lock" ] || { echo "    the manifest's Cargo.lock digest '$(field cargo_lock_sha256)' is not the lock file's $want_lock"; exit 1; }
  [ "$(field profile)" = debug ] || { echo "    the manifest names profile '$(field profile)', not debug"; exit 1; }
  [ "$(field name)" = majordomus ] || { echo "    the manifest names '$(field name)', not majordomus"; exit 1; }
  case "$(field built_at)" in
    [0-9][0-9][0-9][0-9]-[0-1][0-9]-[0-3][0-9]T[0-2][0-9]:[0-5][0-9]:[0-5][0-9]Z) ;;
    *) echo "    the manifest's built_at '$(field built_at)' is not a UTC timestamp"; exit 1 ;;
  esac
else
  echo "    skip: rustc not installed (the manifest's target and toolchain are rustc's answers)"
fi

# ------------------------------------------------------------------ the workflow publishes it
rust_job="$(awk '/^  rust:/{f=1} /^  coverage:/{f=0} f' "$WF")"
[ -n "$rust_job" ] || { echo "    validate.yml has no rust job"; exit 1; }
printf '%s\n' "$rust_job" | grep -qE 'scripts/rust-check "\$MODE" --artifact dist$' \
  || { echo "    the rust job does not ask scripts/rust-check for the artifact in dist"; exit 1; }
printf '%s\n' "$rust_job" | grep -qF 'name: majordomus-cli-${{ steps.target.outputs.triple }}' \
  || { echo "    the rust job does not upload the artifact under majordomus-cli-<target>"; exit 1; }
# The upload that carries the executable is the one named for the triple, whichever upload of
# the job it happens to be. The job publishes more than one artifact — the crate's test output
# the evidence recorder reads, and the timings — and which goes first is an ordering, not the
# property. What is asserted is the pairing: the artifact named majordomus-cli-<target> is the
# dist directory rust-check wrote.
printf '%s\n' "$rust_job" \
  | awk '/name: majordomus-cli-\$\{\{ steps\.target\.outputs\.triple \}\}/{f=1; next} f&&/path:/{print; exit}' \
  | grep -qE 'path: dist$' \
  || { echo "    the artifact named majordomus-cli-<target> is not the dist directory rust-check wrote"; exit 1; }
printf '%s\n' "$rust_job" | grep -qF "triple=\$(rustc -vV | sed -n 's/^host: //p')" \
  || { echo "    the artifact's name is not the triple the manifest records"; exit 1; }
# the suite jobs build once and hand the executable to every case
n="$(grep -c 'echo "MAJORDOMUS_BIN=$PWD/apps/majordomus-cli/target/debug/majordomus" >> "$GITHUB_ENV"' "$WF" || true)"
[ "$n" -ge 1 ] || { echo "    no job of validate.yml hands the cases MAJORDOMUS_BIN"; exit 1; }

# ------------------------------------------------------------------ a case drives MAJORDOMUS_BIN
# rust_bin hands the named executable back without asking cargo anything
STUB="$T/stub"
if [ ! -x "$STUB/cargo" ]; then   # the half above skipped: a cargo that only records its calls
  mkdir -p "$STUB"
  printf '#!/bin/sh\nprintf "%%s\\n" "$*" >> "$STUB_CARGO_LOG"\n' > "$STUB/cargo"; chmod +x "$STUB/cargo"
fi
: > "$T/cargo-case.log"
got="$(PATH="$STUB:$PATH" STUB_CARGO_LOG="$T/cargo-case.log" STUB_BUILT=/nonexistent CARGO_TARGET_DIR="$T/nowhere" \
       MAJORDOMUS_BIN="$RB" rust_bin)"
[ "$got" = "$RB" ] || { echo "    rust_bin handed back '$got', not MAJORDOMUS_BIN"; exit 1; }
[ ! -s "$T/cargo-case.log" ] || { echo "    rust_bin called cargo with MAJORDOMUS_BIN set: $(cat "$T/cargo-case.log")"; exit 1; }
# a named executable that is not there is a failure, never a skip
rc=0; MAJORDOMUS_BIN="$T/no-such-majordomus" rust_bin >/dev/null 2>&1 || rc=$?
[ "$rc" = 1 ] || { echo "    rust_bin accepted a MAJORDOMUS_BIN that is not executable (rc $rc)"; exit 1; }
# the launcher honours the same variable
expect_grep 'MAJORDOMUS_BIN' "$ROOT/bin/majordomus-mcp"

# No case builds the crate's executable itself: every one goes through rust_bin, which is what
# honours MAJORDOMUS_BIN. `cargo build` of the crate outside test/lib.sh is a case that
# rebuilds on a runner that was handed the executable. Cases whose cargo work is cargo's own
# (doc tests, benchmark builds, the crate's suites) use other subcommands and are not matched;
# lines that write `cargo build` into a fixture or a message are not a build and are excluded
# by requiring the manifest of this crate on the same line.
#
# No exception is left: cases 92 and 93 were the last two, and now call rust_bin.
KNOWN=''
found="$(grep -lE '^[^#]*cargo build[^|]*--manifest-path "\$(MANIFEST|ROOT/apps/majordomus-cli/Cargo\.toml)"' "$ROOT"/test/cases/*.sh 2>/dev/null \
  | sed 's#.*/##; s#\.sh$##' | LC_ALL=C sort || true)"
[ "$found" = "$KNOWN" ] || {
  printf '    the cases that build the crate themselves instead of calling rust_bin changed\n    known:\n%s\n    found:\n%s\n' "$KNOWN" "$found"
  exit 1
}
