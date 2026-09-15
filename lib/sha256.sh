# shellcheck shell=bash
# SHA-256 without Perl, for the tool, the scripts and the cases alike.
#
# sha256sum where the machine has it (GNU coreutils on Linux; macOS ships one in /sbin), and
# openssl otherwise (LibreSSL on stock macOS, OpenSSL elsewhere), whose -r prints sha256sum's
# own "<hash> *<path>" and is rewritten here to sha256sum's "<hash>  <path>". Never shasum: on
# macOS it is a Perl script, and nothing in this repository runs Perl (case 364 holds it).
# With neither tool there is no digest, and every function here says so and returns non-zero
# rather than printing an empty hash a caller would compare against.
#
# MJ_SHA256_TOOL=openssl forces the second branch on a machine that has sha256sum, which is
# how that branch is exercised at all.

MJ_SHA256_MISSING="need sha256sum or openssl to compute a SHA-256 digest"

# Decides the tool once per shell. Returns 1, having said why, when there is none.
mj_sha256_tool() {
  case "${MJ_SHA256_TOOL:-}" in sha256sum | openssl) return 0 ;; esac
  if command -v sha256sum >/dev/null 2>&1; then MJ_SHA256_TOOL=sha256sum
  elif command -v openssl >/dev/null 2>&1; then MJ_SHA256_TOOL=openssl
  else printf '%s\n' "$MJ_SHA256_MISSING" >&2; return 1; fi
}

# sha256sum's lines for the files named, or for stdin when none is.
mj_sha256sum() {
  mj_sha256_tool || return 1
  case "$MJ_SHA256_TOOL" in
    sha256sum) sha256sum "$@" ;;
    *) openssl dgst -sha256 -r "$@" | sed 's/^\([0-9a-f]\{64\}\) \*/\1  /' ;;
  esac
}

# The same lines for the paths xargs reads from stdin; the arguments are xargs's own options
# (-0 for NUL-separated paths). One hashing process per batch, not one per file.
mj_sha256_xargs() {
  mj_sha256_tool || return 1
  case "$MJ_SHA256_TOOL" in
    sha256sum) xargs "$@" sha256sum ;;
    *) xargs "$@" openssl dgst -sha256 -r | sed 's/^\([0-9a-f]\{64\}\) \*/\1  /' ;;
  esac
}

# The bare hash of one file, or of stdin when no file is named.
mj_sha256_hex() {
  mj_sha256_tool || return 1
  mj_sha256sum "$@" | cut -d' ' -f1
}
