# majordomus-covers: none
# No entry path or gate builds (I3957, ADR 0224). On 2026-10-22 a pre-push sat forty minutes in
# one gate and timed out at ninety, queued behind the shared build lock with the tree's
# executable already built: nine gates ran `cargo run`, the launcher rebuilt a stale executable
# for every caller, the test harness ran a raw `cargo build` per Rust case, and the edit guard
# let an edit through in silence when the executable was missing. Now a missing or stale
# executable is answered through the installed release only when it is of this tree's
# generation, and otherwise refused with exit 12 naming `just build`; nothing builds.
#
# A stand-in cargo on PATH records every invocation, `cargo metadata` included, and the
# executable is pointed (CARGO_TARGET_DIR) at a directory where it is first missing and then
# stale. Every path is walked in both states, and the stand-in must have been called zero times:
#
#   1  bin/majordomus-cli (--help and a read-only call), bin/majordomus-mcp, bin/majordomus
#      handing words to it (and lib/plan.sh doing the same), and the just bridge
#   2  every gate of .ai/repo/ci/gates.yaml that runs the executable; no gate says `cargo run`,
#      and the ones that do build say so in their own words (scripts/rust-check, cargo test)
#   3  test/lib.sh's rust_bin, which holds no `cargo build` any more
#   4  each git hook, in a fixture whose other commands are stand-ins: the hook refuses (12)
#   5  the edit guard: the edit runs, and the skip is said and recorded — never silent
#   6  the installed release: refused when its generation differs from the tree (another
#      commit, no stamp, a crate moved since), used when it matches, through an installer's
#      launcher as well as directly
#   7  every caller declares its outcome on a `majordomus-on-stale-executable:` line
. "$ROOT/test/lib.sh"

fail() { printf '    %s\n' "$*"; exit 1; }
FAKE="$T/fake"; CALLS="$T/cargo.calls"; TARGET="$T/target"
mkdir -p "$FAKE" "$TARGET/debug"
cat > "$FAKE/cargo" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_CALLS"
exit 101
SH
chmod +x "$FAKE/cargo"
: > "$CALLS"

# with <state> <cmd...>: run a command with the executable in that state, no MAJORDOMUS_BIN,
# no installed release, the stand-in cargo first on PATH; sets RC and leaves stderr in $T/err
with() {
  local state="$1"; shift
  rm -f "$TARGET/debug/majordomus"
  if [ "$state" = stale ]; then
    printf '#!/bin/sh\necho STALE-RAN\n' > "$TARGET/debug/majordomus"; chmod +x "$TARGET/debug/majordomus"
    touch -t 202001010000 "$TARGET/debug/majordomus"
  fi
  RC=0
  env -u MAJORDOMUS_BIN -u MAJORDOMUS_BIN_COMMIT -u MAJORDOMUS_SHARE -u MAJORDOMUS_HOOK \
    PATH="$FAKE:$PATH" FAKE_CALLS="$CALLS" CARGO_TARGET_DIR="$TARGET" \
    MAJORDOMUS_INSTALLED_BIN="$T/no-installed-release" MAJORDOMUS_MIN_FREE_MB=0 "$@" \
    > "$T/out" 2> "$T/err" < /dev/null || RC=$?
}
# refused <what>: exit 12, nothing of the stale executable ran, the build named, no cargo
refused() {
  [ "$RC" = 12 ] || fail "$1: exit $RC, not 12: $(head -c 600 "$T/err")"
  ! grep -q STALE-RAN "$T/out" || fail "$1: the stale executable ran"
  grep -q 'run: just build' "$T/err" || fail "$1: the refusal does not name the explicit build: $(head -c 600 "$T/err")"
  [ ! -s "$CALLS" ] || fail "$1: cargo was called: $(cat "$CALLS")"
}

# ---------------------------------------------------------------- 1. the entry paths
# asked from the fixture directory by absolute path: each launcher finds its tree from its own
# path, and the refusal comes before anything could be written anywhere
paths=0
for state in missing stale; do
  with "$state" "$ROOT/bin/majordomus-cli" --help; refused "bin/majordomus-cli --help ($state)"
  with "$state" "$ROOT/bin/majordomus-cli" worktree status; refused "bin/majordomus-cli worktree status ($state)"
  with "$state" "$ROOT/bin/majordomus-mcp" --standalone; refused "bin/majordomus-mcp ($state)"
  with "$state" "$ROOT/bin/majordomus" worktree status; refused "bin/majordomus worktree status ($state)"
  with "$state" "$ROOT/bin/majordomus" plan delivery; refused "bin/majordomus plan delivery, through lib/plan.sh ($state)"
  paths=$((paths + 5))
  if command -v just >/dev/null 2>&1; then
    with "$state" just --justfile "$ROOT/justfile" --working-directory "$ROOT" bridge --check
    refused "just bridge ($state)"; paths=$((paths + 1))
  fi
done
command -v just >/dev/null 2>&1 || echo "    (no just here: the bridge is held by its generator's line only)"
# every recipe the bridge derives runs the launcher, never cargo: the generator says which
grep -q 'majordomus_executable := justfile_directory() / \\"bin/majordomus-cli\\"' \
  "$ROOT/apps/majordomus-cli/src/command_graph/bridge.rs" || fail "the bridge no longer runs bin/majordomus-cli"

# ---------------------------------------------------------------- 2. every gate
G="$ROOT/.ai/repo/ci/gates.yaml"
! grep -n '^    runs:.*cargo run' "$G" || fail "a gate still runs cargo run"
gates=0
while IFS= read -r line; do
  cmd="${line#    runs: }"
  case "$cmd" in bin/majordomus-cli\ *) ;; *) continue ;; esac
  for state in missing stale; do
    with "$state" bash -c 'cd "$1" && bash -c "$2"' _ "$ROOT" "$cmd"; refused "gate '$cmd' ($state)"
  done
  gates=$((gates + 1))
done < <(grep '^    runs: ' "$G")
# a gate script that runs the executable itself resolves it the same way
for state in missing stale; do
  with "$state" "$ROOT/scripts/ci/entity-check"; refused "gate scripts/ci/entity-check ($state)"
done
gates=$((gates + 1))
[ "$gates" -ge 10 ] || fail "only $gates gate(s) run the executable through the launcher; the nine cargo-run gates should"
# the gates that do build say so in their own words; anything else that reaches cargo is a defect
while IFS= read -r line; do
  cmd="${line#    runs: }"
  case "$cmd" in *cargo*|*rust-check*) ;; *) continue ;; esac
  printf '%s' "$cmd" | grep -qE '(^|&& )(scripts/rust-check|MJ_TEST_JOBS=[0-9]+ bash test/run.sh && cargo test)' \
    || fail "a gate reaches cargo without being a declared build: $cmd"
done < <(grep '^    runs: ' "$G")

# ---------------------------------------------------------------- 3. the test harness
! grep -nE '^[^#]*cargo (build|run)' "$ROOT/test/lib.sh" || fail "test/lib.sh still builds"
for state in missing stale; do
  with "$state" env T="$T" ROOT="$ROOT" bash -c 'cd "$T" && . "$ROOT/test/lib.sh" && rust_bin' 
  refused "test/lib.sh rust_bin ($state)"
done

# ---------------------------------------------------------------- 4. the git hooks, in a fixture
F="$T/hooks"
mkdir -p "$F/bin" "$F/lib" "$F/scripts/ci" "$F/apps/majordomus-cli/src" "$F/.githooks"
cp "$ROOT/bin/majordomus-cli" "$F/bin/"; cp "$ROOT/lib/rust_bin.sh" "$F/lib/"
cp "$ROOT"/.githooks/pre-commit "$ROOT"/.githooks/commit-msg "$ROOT"/.githooks/pre-push "$F/.githooks/"
printf '[package]\nname = "fixture"\n' > "$F/apps/majordomus-cli/Cargo.toml"
: > "$F/apps/majordomus-cli/src/main.rs"
# everything a hook asks besides the launcher answers yes and records that it was asked
for s in bin/majordomus scripts/pages scripts/ci/providers-check scripts/case-numbers scripts/rust-check scripts/ci/run-plan; do
  printf '#!/bin/sh\necho "%s $*" >> "%s/asked"\n' "$s" "$T" > "$F/$s"; chmod +x "$F/$s"
done
git -C "$F" init -q && git -C "$F" add -A \
  && git -C "$F" -c user.email=case@example.invalid -c user.name=case commit -qm fixture \
  || fail "could not build the hook fixture"
printf 'feat(x): a message\n' > "$T/msg"
for state in missing stale; do
  for h in pre-commit commit-msg; do
    RC=0; (cd "$F" && with "$state" sh ".githooks/$h" "$T/msg"; exit "$RC") || RC=$?
    refused ".githooks/$h ($state)"
  done
  RC=0; (cd "$F" && printf 'refs/heads/x %s refs/heads/x 0000000000000000000000000000000000000000\n' \
    "$(git -C "$F" rev-parse HEAD)" | { with "$state" sh .githooks/pre-push origin x; exit "$RC"; }) || RC=$?
  # pre-push reaches the executable only through bin/majordomus, a stand-in here: it must not
  # have built anything on the way
  [ ! -s "$CALLS" ] || fail ".githooks/pre-push ($state): cargo was called: $(cat "$CALLS")"
done
grep -q 'bin/majordomus finish --check' "$T/asked" || fail "the pre-push fixture did not reach its checks"

# ---------------------------------------------------------------- 5. the edit guard
E="$T/guard"
mkdir -p "$E/bin" "$E/lib" "$E/.claude/hooks" "$E/apps/majordomus-cli/src"
cp "$ROOT/bin/majordomus-cli" "$E/bin/"; cp "$ROOT/lib/rust_bin.sh" "$E/lib/"
cp "$ROOT/.claude/hooks/majordomus-edit-guard" "$E/.claude/hooks/"
printf '[package]\nname = "fixture"\n' > "$E/apps/majordomus-cli/Cargo.toml"
: > "$E/apps/majordomus-cli/src/main.rs"
for state in missing stale; do
  rm -f "$E/.ai/local/state/guard/skipped.log"
  RC=0; with "$state" sh -c 'printf "{\"tool_input\":{\"file_path\":\"%s/x\"}}" "$1" | "$1/.claude/hooks/majordomus-edit-guard"' _ "$E"
  [ "$RC" = 0 ] || fail "the edit guard refused an edit for a $state executable (exit $RC): $(cat "$T/err")"
  [ ! -s "$CALLS" ] || fail "the edit guard ($state): cargo was called: $(cat "$CALLS")"
  grep -q 'majordomus-edit-guard: the edit was not judged (exit 12)' "$T/err" \
    || fail "the edit guard passed a $state executable in silence: $(cat "$T/err")"
  grep -q "exit 12" "$E/.ai/local/state/guard/skipped.log" 2>/dev/null \
    || fail "the edit guard's skip ($state) was not recorded"
done

# ---------------------------------------------------------------- 6. the installed release
P="$T/release-tree"
mkdir -p "$P/bin" "$P/lib" "$P/apps/majordomus-cli/src" "$T/versions/9.9.9/bin" "$T/launcher"
cp "$ROOT/bin/majordomus-cli" "$P/bin/"; cp "$ROOT/lib/rust_bin.sh" "$P/lib/"
printf '[package]\nname = "fixture"\n' > "$P/apps/majordomus-cli/Cargo.toml"
printf 'fn main() {}\n' > "$P/apps/majordomus-cli/src/main.rs"; printf 'target/\n' > "$P/.gitignore"
git -C "$P" init -q && git -C "$P" add -A \
  && git -C "$P" -c user.email=case@example.invalid -c user.name=case commit -qm crate \
  || fail "could not build the release fixture"
REL="$T/versions/9.9.9/bin/majordomus"
printf '#!/bin/sh\n[ "$1" = --version ] && { echo "majordomus 9.9.9"; exit 0; }\necho "RELEASE $*"\n' > "$REL"; chmod +x "$REL"
# the installer's launcher, as install.sh writes it: two lines that exec the version
printf '#!/bin/sh\nMAJORDOMUS_LAUNCHER="%s"; export MAJORDOMUS_LAUNCHER\nexec "%s" "$@"\n' \
  "$T/launcher/majordomus" "$REL" > "$T/launcher/majordomus"; chmod +x "$T/launcher/majordomus"
stamp() { printf '{\n  "schema": "majordomus/build/v1",\n  "version": "9.9.9",\n  "commit": "%s"\n}\n' "$1" > "$T/versions/9.9.9/RELEASE.json"; }
release() { # <installed> -- the launcher in the release fixture, the executable missing
  local inst="$1"; RC=0
  (cd "$P" && with missing env MAJORDOMUS_INSTALLED_BIN="$inst" bin/majordomus-cli worktree status; exit "$RC") || RC=$?
}
for inst in "$REL" "$T/launcher/majordomus"; do
  stamp "$(git -C "$P" rev-parse HEAD)"
  release "$inst"
  [ "$RC" = 0 ] && grep -qx 'RELEASE worktree status' "$T/out" \
    || fail "a release of this generation was not used through $inst (exit $RC): $(cat "$T/err")"
  grep -q 'whose crate this tree carries unchanged' "$T/err" || fail "the fallback is not said: $(cat "$T/err")"
  stamp 0000000000000000000000000000000000000000
  release "$inst"; refused "a release of another commit through $inst"
  grep -q "is not of this tree's generation" "$T/err" || fail "the refusal does not say why: $(cat "$T/err")"
done
rm -f "$T/versions/9.9.9/RELEASE.json"
release "$REL"; refused "a release with no build stamp"
stamp "$(git -C "$P" rev-parse HEAD)"
printf 'fn main() { println!("moved"); }\n' > "$P/apps/majordomus-cli/src/main.rs"
release "$REL"; refused "a release built before the crate moved"
grep -q 'the crate differs from' "$T/err" || fail "the refusal does not say the crate moved: $(cat "$T/err")"

# ---------------------------------------------------------------- 7. every caller declares its outcome
decl='^# majordomus-on-stale-executable: (fallback, else )?(refuse|skip)$'
for f in bin/majordomus-mcp .githooks/pre-commit .githooks/commit-msg .githooks/pre-push \
    .claude/hooks/majordomus-edit-guard scripts/ci/run-plan scripts/ci/entity-check test/lib.sh; do
  grep -qE "$decl" "$ROOT/$f" || fail "$f does not declare what a missing or stale executable means"
done
grep -qE '^# majordomus-on-stale-executable: fallback, else skip$' "$ROOT/.claude/hooks/majordomus-edit-guard" \
  || fail "the edit guard does not declare its skip"
for f in "$ROOT"/.githooks/* "$ROOT"/.claude/hooks/*; do
  grep -q 'bin/majordomus-cli' "$f" || continue
  grep -qE "$decl" "$f" || fail "${f#"$ROOT"/} runs the launcher and declares no outcome"
done

echo "    $paths entry path(s), $gates gate(s), test/lib.sh, three hooks and the edit guard, missing and stale: 0 cargo calls; a release is used only of this generation"
