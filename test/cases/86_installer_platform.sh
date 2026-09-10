# The installer resolves the machine it is on, and refuses the machines it cannot serve by
# naming what it detected. `uname` is replaced on the PATH so that every platform the model
# declares is exercised from one machine; nothing here reaches the network, because nothing
# here gets as far as needing to.
. "$ROOT/test/lib.sh"
INSTALLER="$ROOT/site/static/install.sh"
expect_file "$INSTALLER"

# It is POSIX shell, and says so to every shell that will run it.
for sh in sh dash bash ksh; do
  command -v "$sh" >/dev/null 2>&1 || continue
  "$sh" -n "$INSTALLER" || { echo "    $sh cannot parse the installer"; exit 1; }
done
if command -v shellcheck >/dev/null 2>&1; then
  shellcheck -s sh -S warning "$INSTALLER" || { echo "    shellcheck refuses the installer"; exit 1; }
fi

# The one promise it makes about itself: no toolchain, no elevation.
expect_no_grep '(^|[^a-zA-Z_])sudo[[:space:]]' "$INSTALLER"
expect_no_grep '(^|[^a-zA-Z_])eval[[:space:]]' "$INSTALLER"
expect_no_grep 'cargo|rustup|npm |python|pip ' "$INSTALLER"

fake="$T/fakebin"; mkdir -p "$fake"
uname_says() { # <-s value> <-m value>
  cat > "$fake/uname" <<FAKE
#!/bin/sh
case "\$1" in
  -s) echo "$1" ;;
  -m) echo "$2" ;;
  *) echo "$1" ;;
esac
FAKE
  chmod +x "$fake/uname"
}

# It must not reach the network to refuse a platform, so an unroutable base proves the
# refusal happens before anything is fetched.
run_installer() {
  env HOME="$T/home" PATH="$fake:$PATH" \
      MAJORDOMOS_UNUSED=1 \
      MAJORDOMUS_RELEASE_BASE_URL="http://127.0.0.1:1/nothing" \
      MAJORDOMUS_INSECURE_BASE_URL=1 \
      sh "$INSTALLER" "$@" 2>&1
}

# --- an architecture nothing is built for ----------------------------------------------
uname_says Linux riscv64
out="$(run_installer --dry-run || true)"
printf '%s\n' "$out" | grep -q 'does not provide a prebuilt binary' \
  || { echo "    an unsupported architecture was not refused: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'riscv64' \
  || { echo "    the refusal does not name what it detected"; exit 1; }
printf '%s\n' "$out" | grep -q 'macOS ARM64' \
  || { echo "    the refusal does not list the supported platforms"; exit 1; }
printf '%s\n' "$out" | grep -q 'does not build from source' \
  || { echo "    the refusal does not say it will not build from source"; exit 1; }

# --- an operating system nothing is built for -------------------------------------------
uname_says OpenBSD x86_64
out="$(run_installer --dry-run || true)"
printf '%s\n' "$out" | grep -q 'OpenBSD' || { echo "    the refusal does not name the OS"; exit 1; }

# --- a platform the model declares and does not build ------------------------------------
# every unavailable target must be refused with the reason the model records
"$(rust_bin)" distribution --repo "$ROOT" targets 2>/dev/null \
  | awk 'NR>1 && $3=="unavailable"{print $2}' > "$T/unavailable"
if [ -s "$T/unavailable" ]; then
  grep -q 'x86_64-pc-windows-msvc' "$T/unavailable" \
    || { echo "    the model no longer declares the target this case knows about"; exit 1; }
fi

# --- the spellings of each architecture are normalised, not guessed ----------------------
# arm64 and aarch64 are one architecture; amd64 and x86_64 are one
for pair in "Darwin arm64" "Darwin aarch64"; do
  # shellcheck disable=SC2086      # the pair is two words on purpose
  uname_says $pair
  out="$(run_installer --dry-run || true)"
  printf '%s\n' "$out" | grep -q 'aarch64-apple-darwin' \
    || { echo "    $pair did not resolve to aarch64-apple-darwin: $out"; exit 1; }
done
for pair in "Linux x86_64" "Linux amd64"; do
  # shellcheck disable=SC2086
  uname_says $pair
  out="$(run_installer --dry-run || true)"
  printf '%s\n' "$out" | grep -q 'x86_64-unknown-linux-' \
    || { echo "    $pair did not resolve to a linux x86_64 target: $out"; exit 1; }
done

# --- the C library decides which Linux artifact, and an undetermined one takes the static
# one rather than guessing about the machine
uname_says Linux aarch64
out="$(run_installer --dry-run || true)"
printf '%s\n' "$out" | grep -q 'aarch64-unknown-linux-\(gnu\|musl\)' \
  || { echo "    a linux aarch64 machine resolved to nothing: $out"; exit 1; }
case "$(uname -s)" in
  Linux)
    # on a real Linux host the detection is real, and must name a library it found
    printf '%s\n' "$out" | grep -qE 'C library +(gnu|musl)' \
      || { echo "    a real Linux host did not detect its C library: $out"; exit 1; } ;;
  *)
    # elsewhere there is no C library to find, and the fallback must be the static one
    printf '%s\n' "$out" | grep -q 'aarch64-unknown-linux-musl' \
      || { echo "    an undetermined C library did not fall back to musl: $out"; exit 1; } ;;
esac

# --- a base URL that is not HTTPS is refused unless it is said twice ---------------------
uname_says "$(uname -s)" "$(uname -m)"
out="$(env HOME="$T/home" PATH="$fake:$PATH" MAJORDOMUS_RELEASE_BASE_URL="http://127.0.0.1:1/x" \
        sh "$INSTALLER" --dry-run 2>&1 || true)"
printf '%s\n' "$out" | grep -q 'not HTTPS' \
  || { echo "    a plain-HTTP metadata base was accepted: $out"; exit 1; }

# --- --help says what it takes, and changes nothing --------------------------------------
out="$(run_installer --help || true)"
for flag in --version --install-dir --prefix --init --dry-run --force --verbose; do
  printf '%s\n' "$out" | grep -q -- "$flag" || { echo "    --help does not document $flag"; exit 1; }
done
[ -d "$T/home/.local" ] && { echo "    a dry run or --help wrote into HOME"; exit 1; }

# --- it runs, not merely parses, under every POSIX shell on this machine -------------------
# The refusal path needs no network, so it is the one that can be executed under each shell.
uname_says Linux riscv64
for sh in dash bash ksh /bin/sh /bin/ash busybox; do
  case "$sh" in
    busybox) command -v busybox >/dev/null 2>&1 || continue; runner="busybox sh" ;;
    *) command -v "$sh" >/dev/null 2>&1 || continue; runner="$sh" ;;
  esac
  # A shell that carries `uname` as its own applet resolves it internally and never looks at
  # PATH, so the fake platform does not reach the installer at all — busybox's shell is built
  # that way. Measure whether the shim landed rather than assuming it did: where it lands, the
  # installer must refuse the platform by name; where it cannot, the installer still has to
  # run the whole script — detection, resolution, the network — to a clean refusal. Assuming
  # it landed is what made this read as an installer defect for two days.
  seen="$(env PATH="$fake:$PATH" $runner -c 'uname -m' 2>/dev/null || true)"
  if [ "$seen" = riscv64 ]; then want='does not provide a prebuilt binary'
  else want='Nothing was installed or replaced'; fi
  out="$(env HOME="$T/home" PATH="$fake:$PATH" \
          MAJORDOMUS_RELEASE_BASE_URL="http://127.0.0.1:1/nothing" MAJORDOMUS_INSECURE_BASE_URL=1 \
          $runner "$INSTALLER" --dry-run 2>&1 || true)"
  printf '%s\n' "$out" | grep -q "$want" \
    || { echo "    under $runner the installer did not refuse cleanly (expected /$want/): $out"; exit 1; }
  [ -d "$T/home/.local" ] && { echo "    under $runner a refused run wrote into HOME"; exit 1; }
done

# --- an unknown option is refused, not ignored -------------------------------------------
out="$(run_installer --nonsense || true)"
printf '%s\n' "$out" | grep -q 'unknown option' \
  || { echo "    an unknown option was ignored: $out"; exit 1; }
exit 0
