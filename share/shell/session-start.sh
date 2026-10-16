# Majordomus: a bare `claude` in a supervised repository starts by joining the cluster.
#
# A SessionStart hook can only add context to a session; nothing but a first message starts
# a turn, so a session that is merely opened waits for a person to type (ADR 0172). This
# wraps `claude` so that, inside a repository that has an .ai/ layer, the first message is
# the repository's own `session-start` prompt: join the board, announce, take up the active
# task. The prompt is rendered from the repository (`majordomus prompt render session-start`,
# .ai/repo/prompts/session-start.md), so changing it is a commit, and every checkout that
# pulls it is updated.
#
# Source it from ~/.bashrc or ~/.zshrc; it works in both and defines one function.
#
#   . /path/to/prismatic-majordomus/share/shell/session-start.sh
#
# Applies only to a bare `claude`. Any argument (-p, -c, --resume, a prompt of your own) is
# passed through untouched, and so is every run outside a supervised repository or when the
# prompt cannot be rendered: this never stands between a person and their session.
#
#   MAJORDOMUS_NO_AUTOSTART=1 claude    start without the prompt
#   MAJORDOMUS_SESSION_PROMPT=<name>    render another prompt asset (default: session-start)
claude() {
  if [ "$#" -eq 0 ] && [ -z "${MAJORDOMUS_NO_AUTOSTART:-}" ]; then
    _mj_root="$(git rev-parse --show-toplevel 2>/dev/null)" || _mj_root=
    if [ -n "$_mj_root" ] && [ -f "$_mj_root/.ai/manifest.yaml" ] && command -v majordomus >/dev/null 2>&1; then
      _mj_prompt="$(majordomus --repo "$_mj_root" prompt render "${MAJORDOMUS_SESSION_PROMPT:-session-start}" 2>/dev/null </dev/null)" || _mj_prompt=
      if [ -n "$_mj_prompt" ]; then
        command claude "$_mj_prompt"
        set -- "$?"
        unset _mj_root _mj_prompt
        return "$1"
      fi
    fi
    unset _mj_root _mj_prompt
  fi
  command claude "$@"
}
