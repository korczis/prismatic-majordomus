# majordomus-covers: none
# claims: session-start-prompt
# A bare `claude` in a supervised repository starts with the repository's session-start
# prompt as its first message, and nothing else does.
#
# Hooks add context; only a first message starts a turn. On 2026-10-16 a session opened in
# this repository, received the whole startup briefing, and waited for a person to type —
# so it was never on the board (ADR 0156, ADR 0172). share/shell/session-start.sh wraps
# `claude` so that the first message is `majordomus prompt render session-start`. The
# fixture stubs `claude` and `majordomus`, and runs the function in bash and, where there is
# one, zsh: the same file, because it is sourced from either.
. "$ROOT/test/lib.sh"

SH="$ROOT/share/shell/session-start.sh"
[ -f "$SH" ] || { echo "    share/shell/session-start.sh is missing"; exit 1; }
[ -f "$ROOT/.ai/repo/prompts/session-start.md" ] || { echo "    the session-start prompt asset is missing"; exit 1; }

S="$(mktemp -d "${TMPDIR:-/tmp}/mj3178.XXXXXX")"; trap 'rm -rf "$S"' EXIT
mkdir -p "$S/bin" "$S/repo/.ai" "$S/plain"
printf 'ai-repository/v1\n' > "$S/repo/.ai/manifest.yaml"
git -C "$S/repo" init -q; git -C "$S/plain" init -q
# the stubs record how they were called; the "claude" is the real thing's stand-in
printf '#!/bin/sh\nprintf "ARGC=%%d\\n" "$#" >> "%s/claude.log"\nfor a in "$@"; do printf "ARG=%%s\\n" "$a" >> "%s/claude.log"; done\nexit "${STUB_RC:-0}"\n' "$S" "$S" > "$S/bin/claude"
printf '#!/bin/sh\n[ -n "${STUB_FAIL:-}" ] && exit 3\nprintf "RENDERED %%s\\n" "$*" >> "%s/majordomus.log"\n[ -n "${STUB_EMPTY:-}" ] && exit 0\nprintf "JOIN THE CLUSTER (%%s)\\n" "$4"\n' "$S" > "$S/bin/majordomus"
chmod +x "$S/bin/claude" "$S/bin/majordomus"

fail() { printf '    %s\n    claude.log: %s\n' "$1" "$(cat "$S/claude.log" 2>/dev/null)"; exit 1; }

# in <shell> <dir> <ENV=VALUE...> -- <args...>: source the function, call claude
call() {
  local sh="$1" dir="$2"; shift 2
  local envs=()
  while [ "$1" != "--" ]; do envs+=("$1"); shift; done; shift
  rm -f "$S/claude.log" "$S/majordomus.log"
  ( cd "$dir" && env -i HOME="$S/home" PATH="$S/bin:/usr/bin:/bin:/usr/local/bin" ${envs[@]+"${envs[@]}"} \
      "$sh" -c '. "$1"; shift; claude "$@"' _ "$SH" "$@" ) >/dev/null 2>&1
}

for sh in bash zsh; do
  command -v "$sh" >/dev/null 2>&1 || { echo "    (no $sh here; skipped that shell)"; continue; }

  # 1. bare, in a supervised repository: the rendered prompt is the one argument
  call "$sh" "$S/repo" -- || fail "$sh: a bare claude failed"
  grep -qx 'ARGC=1' "$S/claude.log" || fail "$sh: a bare claude did not get exactly one argument"
  grep -q '^ARG=JOIN THE CLUSTER' "$S/claude.log" || fail "$sh: the first message is not the rendered prompt"
  grep -q 'prompt render session-start' "$S/majordomus.log" || fail "$sh: the prompt was not rendered from the repository by name"
  grep -q -- "--repo $S/repo\$\|--repo .*/repo prompt" "$S/majordomus.log" || fail "$sh: the prompt was not rendered for this repository"

  # 2. a subdirectory of the repository is still the repository
  mkdir -p "$S/repo/sub/dir"
  call "$sh" "$S/repo/sub/dir" -- || fail "$sh: a bare claude in a subdirectory failed"
  grep -q '^ARG=JOIN THE CLUSTER' "$S/claude.log" || fail "$sh: a subdirectory of the repository did not start the prompt"

  # 3. every argument is the person's own and is passed through untouched
  call "$sh" "$S/repo" -- -c || fail "$sh: claude -c failed"
  grep -qx 'ARG=-c' "$S/claude.log" && grep -qx 'ARGC=1' "$S/claude.log" || fail "$sh: claude -c was changed"
  [ ! -e "$S/majordomus.log" ] || fail "$sh: an argument still rendered the prompt"
  call "$sh" "$S/repo" -- "my own prompt" || fail "$sh: claude with a prompt failed"
  grep -qx 'ARG=my own prompt' "$S/claude.log" || fail "$sh: the person's own prompt was replaced"

  # 4. outside a supervised repository, and in a git repository with no .ai/ layer, nothing changes
  call "$sh" "$S/plain" -- || fail "$sh: claude outside a supervised repository failed"
  grep -qx 'ARGC=0' "$S/claude.log" || fail "$sh: claude outside a supervised repository was given a prompt"
  call "$sh" "$S" -- || fail "$sh: claude outside any repository failed"
  grep -qx 'ARGC=0' "$S/claude.log" || fail "$sh: claude outside any repository was given a prompt"

  # 5. the opt-out
  call "$sh" "$S/repo" MAJORDOMUS_NO_AUTOSTART=1 -- || fail "$sh: the opt-out failed"
  grep -qx 'ARGC=0' "$S/claude.log" || fail "$sh: MAJORDOMUS_NO_AUTOSTART=1 still started the prompt"

  # 6. a prompt that cannot be rendered never stands between the person and the session
  call "$sh" "$S/repo" STUB_FAIL=1 -- || fail "$sh: a failing render stopped the session"
  grep -qx 'ARGC=0' "$S/claude.log" || fail "$sh: a failing render still passed an argument"
  call "$sh" "$S/repo" STUB_EMPTY=1 -- || fail "$sh: an empty render stopped the session"
  grep -qx 'ARGC=0' "$S/claude.log" || fail "$sh: an empty render was sent as a first message"

  # 7. the session's own exit status is the wrapper's
  rc=0; ( cd "$S/repo" && env -i HOME="$S/home" PATH="$S/bin:/usr/bin:/bin" STUB_RC=7 "$sh" -c '. "$1"; claude' _ "$SH" ) >/dev/null 2>&1 || rc=$?
  [ "$rc" = 7 ] || fail "$sh: the exit status of the session was lost (got $rc)"

  # 8. it leaves nothing behind in the person's shell
  out="$( cd "$S/repo" && env -i HOME="$S/home" PATH="$S/bin:/usr/bin:/bin" "$sh" -c '. "$1"; claude >/dev/null 2>&1; echo "[${_mj_root:-}${_mj_prompt:-}${_mj_rc:-}]"' _ "$SH" 2>/dev/null )"
  [ "$out" = "[]" ] || fail "$sh: the wrapper leaked variables into the shell: $out"
done

# the file defines one function and nothing else at load time
n="$(grep -cE '^[a-z_]+\(\) \{' "$SH")"; [ "$n" = 1 ] || { echo "    expected one function in session-start.sh, found $n"; exit 1; }
