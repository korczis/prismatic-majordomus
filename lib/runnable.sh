# Does this tool start? A name on PATH is not a tool that runs.
#
# A version manager's shim is on PATH whether or not it can start anything: asdf answers
# "No version is set for command node" and exits non-zero in any directory with no pinned
# version, and nvm, pyenv and rbenv do the same. A check that asks only `command -v node`
# takes the shim, and what follows reads its failure as the subject's: on 2026-10-17 a push
# ran sixty-eight minutes of gates and then failed seven cases and the UI check, none of
# them because of the change, all of them because `node` could not start in a fixture
# directory outside the repository. The same shape cost the JSON parser of `doctor` a day
# on 2026-10-08 (case 999, lib/doctor.sh, rule project.a-failed-read-is-not-an-empty-answer).
#
# So there is one question, asked in one place:
#
#   mj_runs CMD [PROBE...]   true when CMD is on PATH and starts (`CMD --version` by default)
#   mj_runs_why CMD          one line saying which of the two it was, and what to do about it
#   mj_node_env [ROOT]       make node runnable, or say that it cannot be
#   mj_zola_ready ROOT       zola runs and is at least the release CI builds the site with
#   mj_zola_why ROOT         why not, in one line
#   mj_ci_pin ROOT KEY       a toolchain pin from the one place CI declares it
#
# `mj_node_env` is for the entry points (test/run.sh, scripts/ci/run-plan, the scripts that
# drive node). When node does not start because the version manager has no version set, and
# the person did not choose one (ASDF_NODEJS_VERSION unset), it exports the newest installed
# one for this process and its children, and says so on stderr, once, with how to choose
# another. It never installs anything, never writes a version file, and never overrides a
# choice already made: it only stops a missing setting from looking like a broken change.
#
# Sourced by scripts and test cases, so it defines functions and sets nothing.

# mj_runs CMD [PROBE-ARGS...]
mj_runs() {
  mj_rn_cmd="$1"; shift
  command -v "$mj_rn_cmd" >/dev/null 2>&1 || return 1
  [ $# -gt 0 ] || set -- --version
  "$mj_rn_cmd" "$@" >/dev/null 2>&1
}

# mj_runs_why CMD: the reason, in one line. Empty when CMD runs.
mj_runs_why() {
  mj_rw_cmd="$1"
  if ! command -v "$mj_rw_cmd" >/dev/null 2>&1; then
    printf '%s is not installed' "$mj_rw_cmd"
    return 0
  fi
  mj_rw_err="$("$mj_rw_cmd" --version 2>&1 >/dev/null)" && return 0
  mj_rw_err="$(printf '%s\n' "$mj_rw_err" | sed -n '1p')"
  printf '%s is on PATH but does not start (%s)' "$mj_rw_cmd" "${mj_rw_err:-no message}"
  case "$mj_rw_err" in
    *'No version is set'*)
      printf '; a version manager has no version set here. Set one for this shell, for example'
      case "$mj_rw_cmd" in
        node) printf ' ASDF_NODEJS_VERSION=<one of `asdf list nodejs`>' ;;
        *) printf ' ASDF_%s_VERSION=<an installed version>' "$(printf '%s' "$mj_rw_cmd" | tr 'a-z' 'A-Z')" ;;
      esac
      ;;
  esac
}

# mj_ci_pin ROOT KEY: the default of an input of the site setup action, which is where CI
# declares the Node major and the Zola release it builds with (`node-version`, `zola-version`).
# Declared once there; read here, never copied. Empty when it cannot be read.
mj_ci_pin() {
  awk -v k="$2" '
    $0 ~ "^  " k ":" { f = 1; next }
    f && /^    default:/ { v = $2; gsub(/"/, "", v); print v; exit }
    f && /^  [a-z]/ { exit }' "$1/.github/actions/setup-site/action.yml" 2>/dev/null
}

# mj_node_newest_installed [MAJOR]: the node the version manager has installed that CI would
# use (the highest of MAJOR, when one is installed), else the newest installed. Or nothing.
mj_node_newest_installed() {
  command -v asdf >/dev/null 2>&1 || return 1
  mj_nn_all="$(asdf list nodejs 2>/dev/null | tr -d ' *' | grep -E '^[0-9]+(\.[0-9]+)*$' \
    | sort -t. -k1,1n -k2,2n -k3,3n)"
  mj_nn_pick=""
  if [ -n "${1:-}" ]; then
    mj_nn_pick="$(printf '%s\n' "$mj_nn_all" | grep -E "^$1(\.|$)" | tail -n 1)"
  fi
  [ -n "$mj_nn_pick" ] || mj_nn_pick="$(printf '%s\n' "$mj_nn_all" | tail -n 1)"
  [ -n "$mj_nn_pick" ] && printf '%s\n' "$mj_nn_pick"
}

# mj_node_env [ROOT]: 0 when node runs afterwards, 1 when it cannot be made to. With ROOT, the
# Node major CI declares is preferred among the installed versions.
mj_node_env() {
  mj_runs node && return 0
  # a version the person chose is theirs: if it does not work, that is what to report
  [ -z "${ASDF_NODEJS_VERSION:-}" ] || return 1
  mj_ne_v="$(mj_node_newest_installed "$(mj_ci_pin "${1:-/nonexistent}" node-version)")" || return 1
  [ -n "$mj_ne_v" ] || return 1
  ASDF_NODEJS_VERSION="$mj_ne_v" node --version >/dev/null 2>&1 || return 1
  ASDF_NODEJS_VERSION="$mj_ne_v"; export ASDF_NODEJS_VERSION
  printf 'majordomus: node: the version manager has no version set here; using the installed %s for this run (set ASDF_NODEJS_VERSION to choose another)\n' \
    "$mj_ne_v" >&2
  return 0
}

# mj_zola_ready ROOT: zola starts, and is at least the major.minor CI builds the site with. The
# site's templates are Tera 2 from Zola 0.23; an older zola does not fail the way a missing one
# does, it fails parsing a template in the middle of a build and reads as a broken change.
mj_zola_ready() {
  mj_runs zola || return 1
  mj_zr_pin="$(mj_ci_pin "$1" zola-version)"
  [ -n "$mj_zr_pin" ] || return 0
  mj_zr_have="$(zola --version 2>/dev/null | awk '{print $2}')"
  awk -v have="$mj_zr_have" -v pin="$mj_zr_pin" 'BEGIN {
    split(have, h, "."); split(pin, p, ".")
    exit !((h[1] + 0 > p[1] + 0) || (h[1] + 0 == p[1] + 0 && h[2] + 0 >= p[2] + 0)) }'
}

# mj_zola_why ROOT: one line, empty when zola is ready.
mj_zola_why() {
  mj_runs zola || { mj_runs_why zola; return 0; }
  mj_zola_ready "$1" && return 0
  printf 'zola %s is older than the %s the site is built with (.github/actions/setup-site/action.yml)' \
    "$(zola --version 2>/dev/null | awk '{print $2}')" "$(mj_ci_pin "$1" zola-version)"
}
