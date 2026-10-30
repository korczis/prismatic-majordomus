# majordomus-covers: none
# `install.sh --dry-run` changes nothing, initialising a repository included (I2942). With the
# resolved version already installed, the installer used to run `majordomus init` before it
# reached its dry-run branch, so `--dry-run --init` initialised the repository it was asked
# only to describe. site/static/install.sh against a local origin, with a stand-in launcher
# that reports the resolved version and records every call it receives.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || skip "no curl or wget"
SRC="$ROOT/site/static/releases/v0.28.0.json"
expect_file "$SRC"

FIX="$T/origin"; mkdir -p "$FIX/releases"
start_http "$FIX" || skip "no python3 or node to serve a fixture origin"
trap 'stop_http' EXIT INT TERM HUP
cp "$SRC" "$FIX/releases/latest.json"

H="$T/home"; mkdir -p "$H/.local/bin"
cat > "$H/.local/bin/majordomus" <<STUB
#!/bin/sh
printf '%s\n' "\$*" >> "$T/calls"
[ "\$1" = version ] && echo "majordomus 0.28.0"
exit 0
STUB
chmod +x "$H/.local/bin/majordomus"
R="$T/repo"; mkdir -p "$R"; ( cd "$R" && git init -q . )

install_run() {
  ( cd "$R" && env HOME="$H" MAJORDOMUS_RELEASE_BASE_URL="$HTTP_BASE" MAJORDOMUS_INSECURE_BASE_URL=1 \
      sh "$ROOT/site/static/install.sh" "$@" 2>&1 )
}

# ---------------------------------------------------------------- 1. a dry run initialises nothing
: > "$T/calls"
out="$(install_run --dry-run --init)" || { echo "    the dry run failed:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -q "already installed" || { echo "    the installed version was not recognised:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
if grep -q '^init' "$T/calls"; then
  echo "    --dry-run --init ran init: $(tr '\n' ' ' < "$T/calls")"; exit 1
fi
printf '%s\n' "$out" | grep -q "dry run: would run majordomus init in .*; nothing was changed" \
  || { echo "    the dry run does not say what it would have run:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }

# ---------------------------------------------------------------- 2. without --dry-run, init still runs
: > "$T/calls"
install_run --init > "$T/out" || true
grep -q '^init' "$T/calls" || { echo "    --init without --dry-run no longer runs init"; sed 's/^/      /' "$T/out"; exit 1; }
echo "    with the resolved version installed, --dry-run --init describes the init it would run and runs nothing; --init alone still runs it"
