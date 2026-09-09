# majordomus-covers: none
# Entering a repository must say when what it is about to show you is not current.
#
# The regression: every surface a person sees here — the banner, the `just` bridge, the
# completion, the shared MCP server — is a projection of one executable. When that file
# went missing (a peer reclaiming disk from an over-full volume took the build output with
# it) all of them stopped at once, and when it was merely old, all of them answered with
# code that was no longer in the tree. Neither state said so, and both read from the outside
# as features that had been deleted. Hours went into looking for the deletion.
#
# So: a missing executable names what is unavailable, not only that it is missing; an old
# one says the output is the previous build. Both are one line on standard error and both
# exit 0, because `.envrc` runs this on every `cd` and a non-zero exit there makes direnv
# report that the whole environment failed. Nothing here builds: a shell prompt is not the
# place to start a compiler. The staleness question itself lives in lib/rust_bin.sh so that
# this script and `bin/majordomus-cli` cannot disagree about the answer.
. "$ROOT/test/lib.sh"
fixture_repo "$T"

crate="$T/apps/majordomus-cli"
mkdir -p "$crate/src" "$crate/target/debug"
printf 'fn main() {}\n' > "$crate/src/main.rs"
printf '[package]\nname = "majordomus-cli"\n' > "$crate/Cargo.toml"
printf '# lock\n' > "$crate/Cargo.lock"
bin="$crate/target/debug/majordomus"

# --- 1. no executable: what is unavailable is named, and the exit is still 0
out="$("$T/bin/majordomus-env" export 2>&1 >/dev/null)"; code=$?
[ "$code" = 0 ] || { echo "    a missing executable exited $code; direnv reads that as the whole environment failing"; exit 1; }
for surface in banner "just" completion "shared server"; do
  case "$out" in
    *"$surface"*) ;;
    *) echo "    the missing-executable line does not name the $surface: $out"; exit 1 ;;
  esac
done
case "$out" in *"just build"*) ;; *) echo "    it does not name what to run: $out"; exit 1 ;; esac

# --- 2. an executable newer than its sources is current, and says nothing
cat > "$bin" <<'STUB'
#!/bin/sh
printf 'STUB RAN\n'
STUB
chmod +x "$bin"
out="$("$T/bin/majordomus-env" export 2>&1 >/dev/null)"
ran="$("$T/bin/majordomus-env" export 2>/dev/null)"
case "$out" in *older*) echo "    a current executable was called stale: $out"; exit 1 ;; esac
case "$ran" in *"STUB RAN"*) ;; *) echo "    the executable was not run"; exit 1 ;; esac

# --- 3. a source newer than the executable: it still runs, and it says the output is old
sleep 1; touch "$crate/src/main.rs"
out="$("$T/bin/majordomus-env" export 2>&1 >/dev/null)"; code=$?
[ "$code" = 0 ] || { echo "    a stale executable exited $code; entering a directory must not fail"; exit 1; }
case "$out" in
  *"older than the sources"*) ;;
  *) echo "    a stale executable said nothing: $out"; exit 1 ;;
esac
case "$out" in *"just build"*) ;; *) echo "    the staleness line does not name what to run: $out"; exit 1 ;; esac
ran="$("$T/bin/majordomus-env" export 2>/dev/null)"
case "$ran" in *"STUB RAN"*) ;; *) echo "    a stale executable must still answer, with a warning; it did not run"; exit 1 ;; esac

# --- 4. nothing is built on the way in, however stale it is
before="$(ls "$crate/target/debug")"
"$T/bin/majordomus-env" export >/dev/null 2>&1
[ "$(ls "$crate/target/debug")" = "$before" ] || { echo "    entering the repository changed the build output"; exit 1; }

# --- 5. an executable somebody named explicitly is never judged stale: whoever pointed at
#        a particular file owns whether it is current, and it need not sit beside sources
named="$T/named-majordomus"; cp "$bin" "$named"
out="$(MAJORDOMUS_BIN="$named" "$T/bin/majordomus-env" export 2>&1 >/dev/null)"
case "$out" in *older*) echo "    MAJORDOMUS_BIN was judged stale: $out"; exit 1 ;; esac

# --- 6. and the two callers agree, because they ask the same function
. "$T/lib/rust_bin.sh"
mj_rust_stale "$T" "$bin" || { echo "    lib/rust_bin.sh calls the stale executable current"; exit 1; }
touch "$bin"
mj_rust_stale "$T" "$bin" && { echo "    lib/rust_bin.sh calls a rebuilt executable stale"; exit 1; }
mj_rust_stale "$T" "$T/nowhere" || { echo "    a missing executable is not stale"; exit 1; }
