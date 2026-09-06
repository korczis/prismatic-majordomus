#!/bin/sh
# GENERATED FILE — DO NOT EDIT DIRECTLY
#   Source:    share/install/install.sh.in (behaviour) and share/distribution.yaml (facts)
#   Generator: majordomus generate --target distribution
#
# Majordomus installer.
#
#   curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh
#
# It downloads one archive for this machine, verifies its SHA-256 against the release
# metadata, unpacks it into a versioned directory under a prefix in your home, and writes
# two launchers. It needs no git, no Rust, no Node, no Python and no root, and it never
# runs sudo. Nothing outside the prefix and the launchers is touched.
#
# The trust path is: the release metadata this script fetches over HTTPS names an artifact
# and its digest; the artifact is verified against that digest before anything is unpacked;
# the unpacked tree is inspected and its executable is run once, in a temporary directory,
# to prove it reports the version that was resolved; only then is a launcher replaced, by
# an atomic rename. A failure anywhere before that leaves the previous installation exactly
# as it was.
#
# Read it before running it, if you like:
#
#   curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh -o install.sh
#   less install.sh && sh install.sh
#
# This file has two halves. The block below is generated from share/distribution.yaml and
# holds every platform, name and URL; everything after it is behaviour and states no fact
# of its own. See docs/DISTRIBUTION.md.

set -eu

# >>> generated from share/distribution.yaml — do not edit here >>>
# Every value below comes from share/distribution.yaml. Change the model and run
# `just derive`; editing this region is undone by the next generation and refused by CI.
MJ_BINARY='majordomus'
MJ_REPOSITORY='korczis/prismatic-majordomus'
MJ_BASE_URL='https://korczis.github.io/prismatic-majordomus'
MJ_DOWNLOAD_PREFIX='https://github.com/korczis/prismatic-majordomus/releases/download/'
MJ_ARCHIVE_EXTENSION='tar.gz'
MJ_DEFAULT_INSTALL_DIR="$HOME/.local/bin"
MJ_DEFAULT_PREFIX="$HOME/.local/share/majordomus"

# One line per target: <os> <arch> <libc or -> <rust target> <status>
MJ_TARGETS='macos aarch64 - aarch64-apple-darwin supported
macos x86_64 - x86_64-apple-darwin supported
linux x86_64 gnu x86_64-unknown-linux-gnu supported
linux x86_64 musl x86_64-unknown-linux-musl supported
linux aarch64 gnu aarch64-unknown-linux-gnu supported
linux aarch64 musl aarch64-unknown-linux-musl supported
windows x86_64 - x86_64-pc-windows-msvc unavailable'

# The supported platforms, as the unsupported-platform message lists them
MJ_SUPPORTED_TITLES='macOS ARM64
macOS x86_64
Linux x86_64 glibc
Linux x86_64 musl
Linux ARM64 glibc
Linux ARM64 musl'

# Why a declared target is not built: <rust target> <reason>
MJ_UNAVAILABLE='x86_64-pc-windows-msvc The command a person runs is bash: bin/majordomus and lib/*.sh are POSIX shell, and the archive carries them beside the executable. Until that half has a Windows answer, publishing a Windows archive would ship an executable nothing can drive.'
# <<< generated <<<

# --------------------------------------------------------------------------- output

MJ_PROGRAM='majordomus installer'

mj_colour_on=''
mj_colour_off=''
if [ -t 2 ] && [ -z "${NO_COLOR:-}" ] && [ -z "${CI:-}" ]; then
  mj_colour_on=$(printf '\033[1m')
  mj_colour_off=$(printf '\033[0m')
fi

say() { printf '%s\n' "$*" >&2; }
step() { printf '%s%s%s\n' "$mj_colour_on" "$*" "$mj_colour_off" >&2; }
debug() { [ "$MJ_VERBOSE" = 1 ] && printf '  %s\n' "$*" >&2 || true; }

# Every failure names what failed, what was involved, and what is still intact.
fail() {
  printf '\n%s: %s\n' "$MJ_PROGRAM" "$1" >&2
  shift
  for line in "$@"; do printf '  %s\n' "$line" >&2; done
  if [ "$MJ_INSTALLED_ANYTHING" = 0 ]; then
    printf '\nNothing was installed or replaced; any existing installation is untouched.\n' >&2
  fi
  exit 1
}

# --------------------------------------------------------------------------- usage

usage() {
  cat <<USAGE
$MJ_PROGRAM — install $MJ_BINARY

usage:
  curl -fsSL $MJ_BASE_URL/install.sh | sh
  curl -fsSL $MJ_BASE_URL/install.sh | sh -s -- [options]

options:
  --version <VERSION>   install this exact release (v0.2.0 or 0.2.0); the default is the
                        latest stable release
  --install-dir <DIR>   where the launchers go (default: \$MAJORDOMUS_INSTALL_DIR or
                        $MJ_DEFAULT_INSTALL_DIR)
  --prefix <DIR>        where the versioned trees go (default: \$MAJORDOMUS_PREFIX or
                        $MJ_DEFAULT_PREFIX)
  --init                run '$MJ_BINARY init' in the current directory afterwards
  --dry-run             resolve and report; change nothing
  --force               install even when the same or a newer version is already there
  --verbose             say what is being decided, and why
  --help                this text

environment:
  MAJORDOMUS_VERSION, MAJORDOMUS_INSTALL_DIR, MAJORDOMUS_PREFIX — the same as the options
  NO_COLOR, CI — plain output

Documentation: $MJ_BASE_URL/docs/install/
USAGE
}

# --------------------------------------------------------------------------- arguments

MJ_VERBOSE=0
MJ_DRY_RUN=0
MJ_INIT=0
MJ_FORCE=0
MJ_INSTALLED_ANYTHING=0
MJ_WANTED_VERSION="${MAJORDOMUS_VERSION:-}"
MJ_INSTALL_DIR="${MAJORDOMUS_INSTALL_DIR:-$MJ_DEFAULT_INSTALL_DIR}"
MJ_PREFIX="${MAJORDOMUS_PREFIX:-$MJ_DEFAULT_PREFIX}"

parse_arguments() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --version) [ $# -ge 2 ] || fail "--version needs a value"; MJ_WANTED_VERSION=$2; shift 2 ;;
      --version=*) MJ_WANTED_VERSION=${1#--version=}; shift ;;
      --install-dir) [ $# -ge 2 ] || fail "--install-dir needs a value"; MJ_INSTALL_DIR=$2; shift 2 ;;
      --install-dir=*) MJ_INSTALL_DIR=${1#--install-dir=}; shift ;;
      --prefix) [ $# -ge 2 ] || fail "--prefix needs a value"; MJ_PREFIX=$2; shift 2 ;;
      --prefix=*) MJ_PREFIX=${1#--prefix=}; shift ;;
      --init) MJ_INIT=1; shift ;;
      --dry-run) MJ_DRY_RUN=1; shift ;;
      --force) MJ_FORCE=1; shift ;;
      --verbose|-v) MJ_VERBOSE=1; shift ;;
      --help|-h) usage; exit 0 ;;
      *) usage >&2; fail "unknown option: $1" ;;
    esac
  done
}

# --------------------------------------------------------------------------- detection

# uname -s, normalised. An unknown value is passed through so that the refusal can name it.
detect_os() {
  case "$(uname -s 2>/dev/null || echo unknown)" in
    Darwin) echo macos ;;
    Linux) echo linux ;;
    *) uname -s 2>/dev/null || echo unknown ;;
  esac
}

# uname -m, normalised. The two spellings of each architecture, and nothing guessed.
detect_arch() {
  case "$(uname -m 2>/dev/null || echo unknown)" in
    x86_64|amd64) echo x86_64 ;;
    aarch64|arm64) echo aarch64 ;;
    *) uname -m 2>/dev/null || echo unknown ;;
  esac
}

# The C library, on Linux only, from whichever evidence answers first. No single command
# is trusted: musl's ldd exits non-zero, Alpine has no getconf, and a container may have
# neither. The loader on disk is the most direct evidence there is, so it is asked first.
detect_libc() {
  [ "$MJ_OS" = linux ] || { echo none; return; }
  for loader in /lib/ld-musl-*.so.1 /lib/ld-musl-*; do
    [ -e "$loader" ] && { echo musl; return; }
  done
  for loader in /lib*/ld-linux*.so.* /lib/*-linux-gnu/ld-linux*.so.*; do
    [ -e "$loader" ] && { echo gnu; return; }
  done
  if command -v getconf >/dev/null 2>&1 && getconf GNU_LIBC_VERSION >/dev/null 2>&1; then
    echo gnu; return
  fi
  if command -v ldd >/dev/null 2>&1; then
    case "$(ldd --version 2>&1 || true)" in
      *musl*) echo musl; return ;;
      *GNU*|*GLIBC*|*glibc*) echo gnu; return ;;
    esac
  fi
  echo unknown
}

# The Rust target for this machine, from the generated table. A machine whose C library
# could not be determined is given the statically linked artifact, which runs either way;
# that is a stated fallback, not a guess about the machine.
resolve_target() {
  os=$1 arch=$2 libc=$3
  [ "$libc" = unknown ] && libc=musl
  [ "$libc" = none ] && libc='-'
  MJ_TARGET=''
  MJ_TARGET_STATUS=''
  while read -r t_os t_arch t_libc t_triple t_status; do
    [ -n "${t_os:-}" ] || continue
    [ "$t_os" = "$os" ] || continue
    [ "$t_arch" = "$arch" ] || continue
    [ "$t_libc" = "$libc" ] || continue
    MJ_TARGET=$t_triple
    MJ_TARGET_STATUS=$t_status
    break
  done <<TARGETS
$MJ_TARGETS
TARGETS
}

# Why a declared target is not built, when the machine resolved to one that is not.
unavailable_reason() {
  triple=$1
  while read -r u_triple u_reason; do
    [ -n "${u_triple:-}" ] || continue
    if [ "$u_triple" = "$triple" ]; then printf '%s\n' "$u_reason"; return; fi
  done <<UNAVAILABLE
$MJ_UNAVAILABLE
UNAVAILABLE
}

refuse_platform() {
  detail=$1
  {
    printf '\n%s does not provide a prebuilt binary for:\n\n' "$MJ_BINARY"
    printf '  operating system: %s (uname -s: %s)\n' "$MJ_OS" "$(uname -s 2>/dev/null || echo '?')"
    printf '  architecture:     %s (uname -m: %s)\n' "$MJ_ARCH" "$(uname -m 2>/dev/null || echo '?')"
    [ "$MJ_LIBC" = none ] || printf '  C library:        %s\n' "$MJ_LIBC"
    [ -n "$detail" ] && printf '\n%s\n' "$detail"
    printf '\nSupported platforms:\n\n'
    while read -r title; do [ -n "$title" ] && printf '  %s\n' "$title"; done <<TITLES
$MJ_SUPPORTED_TITLES
TITLES
    printf '\nBuilding from source is documented for contributors at\n'
    printf '  %s/docs/install/\n' "$MJ_BASE_URL"
    printf '  https://github.com/%s\n' "$MJ_REPOSITORY"
    printf 'This installer does not build from source: it would need a toolchain it promises not to require.\n'
  } >&2
  exit 1
}

# --------------------------------------------------------------------------- network

# The base the release metadata is read from. Overridable so that the tests can serve
# fixtures; a base that is not HTTPS is refused unless the override is stated twice.
resolve_base_url() {
  MJ_RESOLVED_BASE=${MAJORDOMUS_RELEASE_BASE_URL:-$MJ_BASE_URL}
  MJ_SECURE=1
  case "$MJ_RESOLVED_BASE" in
    https://*) ;;
    *)
      if [ "${MAJORDOMUS_INSECURE_BASE_URL:-0}" = 1 ]; then
        MJ_SECURE=0
        say "warning: reading release metadata from $MJ_RESOLVED_BASE over an insecure transport, because MAJORDOMUS_INSECURE_BASE_URL=1"
      else
        fail "release metadata base is not HTTPS: $MJ_RESOLVED_BASE" \
             "Set MAJORDOMUS_INSECURE_BASE_URL=1 as well if this is deliberate; it is meant for the installer's own tests."
      fi
      ;;
  esac
}

have() { command -v "$1" >/dev/null 2>&1; }

# Fetch a URL into a file. Answers with the HTTP status where the downloader reports one,
# so that a 404 is told apart from a network that is not there.
http_get() {
  url=$1 dest=$2
  case "$url" in
    https://*) ;;
    http://*) [ "$MJ_SECURE" = 0 ] || fail "refusing a plain-HTTP URL: $url" ;;
    *) fail "refusing a URL that is neither http nor https: $url" ;;
  esac
  if have curl; then
    if [ "$MJ_SECURE" = 1 ]; then
      set -- --proto '=https' --proto-redir '=https'
    else
      set --
    fi
    code=$(curl -sSL "$@" --connect-timeout 15 --max-time 600 \
             -w '%{http_code}' -o "$dest" "$url" 2>"$MJ_TMP/curl.err") || {
      fail "download failed: $url" "$(cat "$MJ_TMP/curl.err" 2>/dev/null || true)" \
           "Check that the machine is online and that $MJ_RESOLVED_BASE is reachable."
    }
    debug "GET $url -> HTTP $code"
    [ "$code" = 200 ] || return 1
    return 0
  fi
  if have wget; then
    if [ "$MJ_SECURE" = 1 ]; then set -- --https-only; else set --; fi
    wget -q "$@" --timeout=30 -O "$dest" "$url" && { debug "GET $url -> ok (wget)"; return 0; }
    return 1
  fi
  fail "no downloader found" \
       "This installer needs curl or wget. Install either and run it again."
}

# --------------------------------------------------------------------------- metadata

# One field of the release metadata. The document is generated with one artifact per line
# and every value is checked against the shape it must have, so that nothing read over the
# network reaches a command line, a path, or the shell.
metadata_field() {
  file=$1 key=$2
  sed -n "s/^  \"$key\": \"\\([^\"]*\\)\".*/\\1/p" "$file" | head -n 1
}

artifact_field() {
  file=$1 triple=$2 key=$3
  sed -n "s/^    \"$triple\": .*\"$key\": \"\\([^\"]*\\)\".*/\\1/p" "$file" | head -n 1
}

artifact_size() {
  file=$1 triple=$2
  sed -n "s/^    \"$triple\": .*\"size\": \\([0-9][0-9]*\\).*/\\1/p" "$file" | head -n 1
}

# Every value read out of the release metadata is checked against the shape it must have
# before it is used. The patterns are literal character classes, so nothing read over the
# network can be interpreted as a glob, an option, a path or a command.
is_digits() { case "${1:-}" in '' | *[!0-9]*) return 1 ;; esac; }

is_hex64() {
  case "${1:-}" in '' | *[!0-9a-f]*) return 1 ;; esac
  [ "${#1}" -eq 64 ]
}

is_filename() {
  case "${1:-}" in '' | *[!A-Za-z0-9._-]* | -* | .*) return 1 ;; esac
}

# Three numbers, optionally a pre-release suffix from the same safe alphabet.
is_version() {
  v=${1:-}
  case "$v" in '' | *[!0-9A-Za-z.-]*) return 1 ;; esac
  core=${v%%-*}
  case "$core" in *[!0-9.]* | .* | *. | *..*) return 1 ;; esac
  case "$core" in *.*.*.*) return 1 ;; *.*.*) ;; *) return 1 ;; esac
}

refuse_metadata() {
  what=$1 value=${2:-}
  if [ -z "$value" ]; then shown=missing; else shown="\"$value\""; fi
  fail "the release metadata is not usable" \
       "$what is $shown" \
       "Metadata: $MJ_METADATA_URL" \
       "Nothing was downloaded from it."
}

resolve_release() {
  if [ -n "$MJ_WANTED_VERSION" ]; then
    case "$MJ_WANTED_VERSION" in v*) tag=$MJ_WANTED_VERSION ;; *) tag=v$MJ_WANTED_VERSION ;; esac
    is_version "${tag#v}" || fail "that is not a version this installer can resolve" \
      "requested: $MJ_WANTED_VERSION" \
      "A version is three numbers, such as v0.2.0, optionally with a pre-release suffix."
    MJ_METADATA_URL="$MJ_RESOLVED_BASE/releases/$tag.json"
    MJ_PINNED=1
  else
    MJ_METADATA_URL="$MJ_RESOLVED_BASE/releases/latest.json"
    MJ_PINNED=0
  fi
  debug "release metadata: $MJ_METADATA_URL"
  if ! http_get "$MJ_METADATA_URL" "$MJ_TMP/release.json"; then
    if [ "$MJ_PINNED" = 1 ]; then
      fail "no release metadata for ${MJ_WANTED_VERSION}" \
           "$MJ_METADATA_URL could not be read." \
           "Every published release is listed at $MJ_BASE_URL/docs/install/."
    fi
    fail "no stable release is published yet, or its metadata could not be read" \
         "$MJ_METADATA_URL could not be read." \
         "If this machine is online, the project may not have published a release yet."
  fi
  MJ_VERSION=$(metadata_field "$MJ_TMP/release.json" version)
  MJ_TAG=$(metadata_field "$MJ_TMP/release.json" tag)
  is_version "$MJ_VERSION" || refuse_metadata "the resolved version" "$MJ_VERSION"
  is_version "${MJ_TAG#v}" || refuse_metadata "the resolved tag" "$MJ_TAG"
  [ "$MJ_TAG" = "v$MJ_VERSION" ] || fail "the release metadata is not usable" \
    "the tag \"$MJ_TAG\" and the version \"$MJ_VERSION\" do not agree" \
    "Metadata: $MJ_METADATA_URL"

  MJ_ARTIFACT=$(artifact_field "$MJ_TMP/release.json" "$MJ_TARGET" name)
  MJ_URL=$(artifact_field "$MJ_TMP/release.json" "$MJ_TARGET" url)
  MJ_SHA256=$(artifact_field "$MJ_TMP/release.json" "$MJ_TARGET" sha256)
  MJ_SIZE=$(artifact_size "$MJ_TMP/release.json" "$MJ_TARGET")
  [ -n "$MJ_ARTIFACT" ] || fail "release $MJ_TAG publishes no artifact for $MJ_TARGET" \
    "The release metadata names no artifact for this machine's target." \
    "Metadata: $MJ_METADATA_URL"
  is_filename "$MJ_ARTIFACT" || refuse_metadata "the artifact name" "$MJ_ARTIFACT"
  is_hex64 "$MJ_SHA256" || refuse_metadata "the artifact digest" "$MJ_SHA256"
  is_digits "$MJ_SIZE" || refuse_metadata "the artifact size" "$MJ_SIZE"
  # The URL may only address this project's own release assets.
  case "$MJ_URL" in
    "$MJ_DOWNLOAD_PREFIX$MJ_TAG/$MJ_ARTIFACT") ;;
    *) if [ "$MJ_SECURE" = 0 ] && [ "${MJ_URL#"$MJ_RESOLVED_BASE"}" != "$MJ_URL" ]; then
         debug "artifact served from the test base"
       else
         fail "the release metadata is not usable" \
              "the artifact URL is \"$MJ_URL\"" \
              "It must be $MJ_DOWNLOAD_PREFIX$MJ_TAG/$MJ_ARTIFACT and nothing else." \
              "Nothing was downloaded from it."
       fi ;;
  esac
  MJ_ROOT="${MJ_ARTIFACT%".$MJ_ARCHIVE_EXTENSION"}"
  [ "$MJ_ROOT" != "$MJ_ARTIFACT" ] || fail "the release metadata is not usable" \
    "the artifact $MJ_ARTIFACT is not a .$MJ_ARCHIVE_EXTENSION archive"
}

# --------------------------------------------------------------------------- versions

# Compare two versions. Answers 0 when the first is greater or equal. A pre-release sorts
# below the release of the same numbers; nothing else about the suffix is interpreted.
version_ge() {
  a=$1 b=$2
  a_pre=''; b_pre=''
  case "$a" in *-*) a_pre=${a#*-}; a=${a%%-*} ;; esac
  case "$b" in *-*) b_pre=${b#*-}; b=${b%%-*} ;; esac
  i=1
  while [ "$i" -le 3 ]; do
    x=$(printf '%s' "$a" | cut -d. -f"$i"); y=$(printf '%s' "$b" | cut -d. -f"$i")
    x=${x:-0}; y=${y:-0}
    case "$x$y" in *[!0-9]*) x=0; y=0 ;; esac
    [ "$x" -gt "$y" ] && return 0
    [ "$x" -lt "$y" ] && return 1
    i=$((i + 1))
  done
  [ -z "$a_pre" ] && return 0
  [ -z "$b_pre" ] && return 1
  [ "$a_pre" = "$b_pre" ]
}

installed_version() {
  launcher="$MJ_INSTALL_DIR/$MJ_BINARY"
  [ -x "$launcher" ] || return 1
  out=$("$launcher" version 2>/dev/null) || return 1
  printf '%s\n' "$out" | sed -n "s/^$MJ_BINARY \\([0-9][0-9.]*[0-9A-Za-z.-]*\\)\$/\\1/p" | head -n 1
}

# --------------------------------------------------------------------------- digests

sha256_of() {
  file=$1
  if have sha256sum; then sha256sum "$file" | cut -d' ' -f1; return; fi
  if have shasum; then shasum -a 256 "$file" | cut -d' ' -f1; return; fi
  if have openssl; then openssl dgst -sha256 "$file" | sed 's/.*= //'; return; fi
  fail "no way to compute a SHA-256 digest" \
       "This installer needs sha256sum, shasum or openssl. It will not install anything it cannot verify."
}

verify_checksum() {
  file=$1 expected=$2
  actual=$(sha256_of "$file")
  debug "sha256 expected $expected"
  debug "sha256 actual   $actual"
  [ "$actual" = "$expected" ] || fail "the downloaded archive does not match its digest" \
    "artifact: $MJ_ARTIFACT" \
    "expected: $expected" \
    "actual:   $actual" \
    "The download was discarded and nothing was installed. Try again; if it keeps failing, do not force it."
}

# --------------------------------------------------------------------------- archive

# Every entry of the archive, before anything is written. Only ordinary files and
# directories under the archive's own root are allowed: no absolute path, no traversal,
# no link of any kind, no device.
validate_archive() {
  file=$1 root=$2
  tar -tzvf "$file" > "$MJ_TMP/listing" 2>/dev/null || fail "the archive is not readable" \
    "artifact: $MJ_ARTIFACT" \
    "It downloaded and matched its digest, and tar cannot read it. Nothing was installed."
  [ -s "$MJ_TMP/listing" ] || fail "the archive is empty" "artifact: $MJ_ARTIFACT"
  bad=0
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    case "$line" in
      -*|d*) ;;
      *) say "refused archive entry (not a file or directory): $line"; bad=1; continue ;;
    esac
    name=$(printf '%s\n' "$line" | sed 's/^.*[0-9][0-9]:[0-9][0-9] //')
    case "$name" in
      /*) say "refused archive entry (absolute path): $name"; bad=1 ;;
      *..*) say "refused archive entry (traversal): $name"; bad=1 ;;
      "$root"|"$root"/*) ;;
      *) say "refused archive entry (outside $root/): $name"; bad=1 ;;
    esac
  done < "$MJ_TMP/listing"
  [ "$bad" = 0 ] || fail "the archive contains entries this installer will not unpack" \
    "artifact: $MJ_ARTIFACT" \
    "Every refused entry is listed above. Nothing was unpacked and nothing was installed."
}

# --------------------------------------------------------------------------- install

lock_prefix() {
  MJ_LOCK="$MJ_PREFIX/.lock"
  waited=0
  while ! mkdir "$MJ_LOCK" 2>/dev/null; do
    [ "$waited" -ge 60 ] && fail "another installation is in progress" \
      "$MJ_LOCK has been held for over a minute." \
      "If no other installer is running, remove that directory and try again."
    sleep 1
    waited=$((waited + 1))
  done
  MJ_HOLD_LOCK=1
}

write_launcher() {
  name=$1 target=$2
  tmp="$MJ_INSTALL_DIR/.$name.$$"
  cat > "$tmp" <<LAUNCHER
#!/bin/sh
# Generated by the Majordomus installer. Reinstalling rewrites it.
# MAJORDOMUS_LAUNCHER is the path that does not move when the version does: it is what the
# tool writes into the git hook lines it asks you to add.
MAJORDOMUS_LAUNCHER="$MJ_INSTALL_DIR/$name"; export MAJORDOMUS_LAUNCHER
exec "$target" "\$@"
LAUNCHER
  chmod 0755 "$tmp"
  mv -f "$tmp" "$MJ_INSTALL_DIR/$name"
}

install_tree() {
  version_dir="$MJ_PREFIX/versions/$MJ_VERSION"
  replaced=''
  if [ -e "$version_dir" ]; then
    replaced="$MJ_PREFIX/versions/.$MJ_VERSION.replaced.$$"
    mv "$version_dir" "$replaced"
  fi
  mkdir -p "$MJ_PREFIX/versions"
  mv "$MJ_STAGE/$MJ_ROOT" "$version_dir"
  MJ_INSTALLED_ANYTHING=1
  write_launcher "$MJ_BINARY" "$version_dir/bin/$MJ_BINARY"
  write_launcher "$MJ_BINARY-mcp" "$version_dir/bin/$MJ_BINARY-mcp"
  [ -n "$replaced" ] && rm -rf "$replaced"
  # Older trees are removed only once the launchers point at the new one.
  for old in "$MJ_PREFIX"/versions/*; do
    [ -d "$old" ] || continue
    [ "$old" = "$version_dir" ] && continue
    debug "removing the superseded tree $old"
    rm -rf "$old"
  done
}

# --------------------------------------------------------------------------- path

path_hint() {
  case ":${PATH:-}:" in
    *":$MJ_INSTALL_DIR:"*) return 0 ;;
  esac
  return 1
}

# --------------------------------------------------------------------------- lifecycle

MJ_TMP=''
MJ_STAGE=''
MJ_HOLD_LOCK=0
MJ_LOCK=''

cleanup() {
  code=$?
  [ -n "$MJ_STAGE" ] && [ -d "$MJ_STAGE" ] && rm -rf "$MJ_STAGE"
  [ -n "$MJ_TMP" ] && [ -d "$MJ_TMP" ] && rm -rf "$MJ_TMP"
  [ "$MJ_HOLD_LOCK" = 1 ] && [ -n "$MJ_LOCK" ] && rmdir "$MJ_LOCK" 2>/dev/null
  exit "$code"
}

main() {
  parse_arguments "$@"
  MJ_TMP=$(mktemp -d 2>/dev/null || mktemp -d -t majordomus) || {
    printf '%s: cannot create a temporary directory\n' "$MJ_PROGRAM" >&2; exit 1; }
  trap cleanup EXIT INT TERM HUP

  MJ_OS=$(detect_os)
  MJ_ARCH=$(detect_arch)
  MJ_LIBC=$(detect_libc)
  debug "detected os=$MJ_OS arch=$MJ_ARCH libc=$MJ_LIBC"
  resolve_target "$MJ_OS" "$MJ_ARCH" "$MJ_LIBC"
  [ -n "$MJ_TARGET" ] || refuse_platform ''
  if [ "$MJ_TARGET_STATUS" = unavailable ]; then
    refuse_platform "This platform is declared and not built: $(unavailable_reason "$MJ_TARGET")"
  fi
  debug "resolved target $MJ_TARGET ($MJ_TARGET_STATUS)"

  resolve_base_url
  step "Resolving the release ..."
  resolve_release

  current=$(installed_version || true)
  if [ -n "$current" ] && [ "$MJ_FORCE" = 0 ]; then
    if [ "$current" = "$MJ_VERSION" ]; then
      say "$MJ_BINARY v$MJ_VERSION is already installed at $MJ_INSTALL_DIR/$MJ_BINARY."
      [ "$MJ_INIT" = 1 ] && run_init
      exit 0
    fi
    if [ "$MJ_PINNED" = 0 ] && version_ge "$current" "$MJ_VERSION"; then
      say "$MJ_BINARY v$current is installed and is newer than the latest stable release (v$MJ_VERSION); nothing was changed."
      say "Install it anyway with --force, or pin a version with --version v$MJ_VERSION."
      exit 0
    fi
  fi

  if [ "$MJ_DRY_RUN" = 1 ]; then
    cat <<REPORT
$MJ_PROGRAM — dry run, nothing will be changed

  operating system    $MJ_OS (uname -s: $(uname -s 2>/dev/null || echo '?'))
  architecture        $MJ_ARCH (uname -m: $(uname -m 2>/dev/null || echo '?'))
  C library           $MJ_LIBC
  target              $MJ_TARGET ($MJ_TARGET_STATUS)
  release metadata    $MJ_METADATA_URL
  resolved release    $MJ_TAG$( [ "$MJ_PINNED" = 1 ] && echo ' (pinned)' || echo ' (latest stable)')
  artifact            $MJ_ARTIFACT ($MJ_SIZE bytes)
  artifact URL        $MJ_URL
  sha-256             $MJ_SHA256
  installed tree      $MJ_PREFIX/versions/$MJ_VERSION
  launchers           $MJ_INSTALL_DIR/$MJ_BINARY, $MJ_INSTALL_DIR/$MJ_BINARY-mcp
  already installed   ${current:-none}
  would run init      $( [ "$MJ_INIT" = 1 ] && echo yes || echo no)
REPORT
    exit 0
  fi

  step "Downloading $MJ_ARTIFACT ..."
  http_get "$MJ_URL" "$MJ_TMP/$MJ_ARTIFACT" || fail "the artifact could not be downloaded" \
    "url: $MJ_URL" \
    "The release metadata named it, and the download did not succeed."
  actual_size=$(wc -c < "$MJ_TMP/$MJ_ARTIFACT" | tr -d ' ')
  [ "$actual_size" = "$MJ_SIZE" ] || fail "the download is not the size the release records" \
    "artifact: $MJ_ARTIFACT" \
    "expected: $MJ_SIZE bytes" \
    "received: $actual_size bytes" \
    "The download was discarded; this is what a truncated transfer looks like."

  step "Verifying the digest ..."
  verify_checksum "$MJ_TMP/$MJ_ARTIFACT" "$MJ_SHA256"
  validate_archive "$MJ_TMP/$MJ_ARTIFACT" "$MJ_ROOT"

  mkdir -p "$MJ_PREFIX" || fail "cannot create the installation prefix" \
    "prefix: $MJ_PREFIX" \
    "Choose another with --prefix. This installer never uses sudo."
  [ -w "$MJ_PREFIX" ] || fail "the installation prefix is not writable" \
    "prefix: $MJ_PREFIX" \
    "Choose another with --prefix, or make it writable. This installer never uses sudo."
  mkdir -p "$MJ_INSTALL_DIR" || fail "cannot create the launcher directory" \
    "directory: $MJ_INSTALL_DIR" \
    "Choose another with --install-dir. This installer never uses sudo."
  [ -w "$MJ_INSTALL_DIR" ] || fail "the launcher directory is not writable" \
    "directory: $MJ_INSTALL_DIR" \
    "Choose another with --install-dir, or make it writable. This installer never uses sudo."

  lock_prefix
  MJ_STAGE="$MJ_PREFIX/.staging.$$"
  rm -rf "$MJ_STAGE"; mkdir -p "$MJ_STAGE"
  step "Unpacking ..."
  tar -xzf "$MJ_TMP/$MJ_ARTIFACT" -C "$MJ_STAGE"
  candidate="$MJ_STAGE/$MJ_ROOT/bin/$MJ_BINARY"
  [ -f "$candidate" ] || fail "the archive holds no $MJ_ROOT/bin/$MJ_BINARY" \
    "artifact: $MJ_ARTIFACT" "Nothing was installed."
  chmod 0755 "$candidate" "$MJ_STAGE/$MJ_ROOT/bin/$MJ_BINARY-mcp" 2>/dev/null || true
  [ -x "$candidate" ] || fail "the unpacked $MJ_BINARY is not executable" "Nothing was installed."

  reported=$("$candidate" version 2>/dev/null | sed -n "s/^$MJ_BINARY \\(.*\\)\$/\\1/p" | head -n 1)
  [ "$reported" = "$MJ_VERSION" ] || fail "the unpacked executable reports another version" \
    "release:  v$MJ_VERSION" \
    "reported: ${reported:-nothing}" \
    "This is what a mismatched release looks like. Nothing was installed."

  step "Installing ..."
  install_tree

  printf '\n%s%s v%s installed successfully.%s\n\n' "$mj_colour_on" "$MJ_BINARY" "$MJ_VERSION" "$mj_colour_off" >&2
  printf 'Binary:\n  %s\n\n' "$MJ_INSTALL_DIR/$MJ_BINARY" >&2
  if path_hint; then
    printf 'Verify:\n  %s --version\n\nStart:\n  %s init\n' "$MJ_BINARY" "$MJ_BINARY" >&2
  else
    printf '%s is not on your PATH. Add it:\n\n  export PATH="%s:$PATH"\n\n' \
      "$MJ_INSTALL_DIR" "$MJ_INSTALL_DIR" >&2
    case "${SHELL:-}" in
      */fish) printf 'For fish, add to ~/.config/fish/config.fish:\n  fish_add_path %s\n\n' "$MJ_INSTALL_DIR" >&2 ;;
      */zsh)  printf 'For zsh, add that line to ~/.zshrc.\n\n' >&2 ;;
      */bash) printf 'For bash, add that line to ~/.bashrc (or ~/.bash_profile on macOS).\n\n' >&2 ;;
    esac
    printf 'Then:\n  %s --version\n  %s init\n' "$MJ_BINARY" "$MJ_BINARY" >&2
  fi
  printf '\nDocumentation: %s/docs/install/\n' "$MJ_BASE_URL" >&2

  [ "$MJ_INIT" = 1 ] && run_init
  exit 0
}

run_init() {
  printf '\n' >&2
  step "Running $MJ_BINARY init in $(pwd) ..."
  if "$MJ_INSTALL_DIR/$MJ_BINARY" init; then
    return 0
  fi
  code=$?
  printf '\n%s: installation succeeded; initialisation did not.\n' "$MJ_PROGRAM" >&2
  printf '  %s is installed at %s and works.\n' "$MJ_BINARY" "$MJ_INSTALL_DIR/$MJ_BINARY" >&2
  printf '  "%s init" exited %s in %s. Read its output above, then run it again where you meant to.\n' \
    "$MJ_BINARY" "$code" "$(pwd)" >&2
  exit "$code"
}

main "$@"
