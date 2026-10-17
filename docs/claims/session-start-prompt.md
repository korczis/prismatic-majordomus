# A bare claude in a supervised repository starts with the repository's session-start prompt as its first message, and a run with an argument, outside such a repository, with the opt-out, or with a prompt that cannot be rendered is left exactly as it was

## What it means

A hook can only add context to a session; nothing but a first message starts a turn. So `claude` opened in this repository used to receive the whole briefing and then wait for a person to type. With the wrapper sourced, a bare `claude` here starts with the repository's own prompt as its first message: say how you are attached, read the board, find the work that is owed, check for collisions, announce, then work inside the task's scope and keep everything local.

## How it works

`share/shell/session-start.sh` defines one function, `claude`, for bash and zsh. With no arguments, inside a git repository whose root has `.ai/manifest.yaml`, it runs `majordomus --repo <root> prompt render session-start` and passes the result as the one argument to the real `claude`. The prompt is `.ai/repo/prompts/session-start.md`, so changing it is a commit and every checkout that pulls it is updated. Any argument, any other directory, `MAJORDOMUS_NO_AUTOSTART=1`, and a render that fails or prints nothing all fall through to a plain `claude "$@"`. The session's exit status is the wrapper's, and no variable is left behind in the shell.

## How to see it

```bash
. share/shell/session-start.sh        # from ~/.zshrc or ~/.bashrc, once
claude                                # here: starts with the session-start prompt
MAJORDOMUS_NO_AUTOSTART=1 claude      # here: starts as before
majordomus prompt render session-start
bash test/run.sh 3178_a_bare_session_start_joins_the_cluster
```

## What it does not cover

It is not installed for you: the installer touches nothing outside its prefix, so sourcing the file is one line a person adds. Only Claude Code is wrapped; Codex and Gemini are not claimed until each is proved against the real tool. The prompt asks the worker to stay local, but the wrapper does not enforce it; scope is enforced by the task record at `check` and `finish`.

## Why it exists

On 2026-10-16 the owner asked why a session opened in this repository did not join the mesh through the hook and start working. The hook had run and the mesh was active; the session simply had no turn to act in. ADR 0156 names that state, a worker that behaves as the only one; ADR 0172 decided the wrapper. The case runs the file in bash and zsh against stubs of `claude` and `majordomus`.
