+++
title = "A credential never reaches a log, an argument list or an environment a coding agent can read"
description = "A credential never reaches a log, an argument list or an environment a coding agent can read"
weight = 1
[extra]
kind = "rule"
slug = "majordomus-a-credential-never-reaches-an-agent-1"
identity = "majordomus.a-credential-never-reaches-an-agent@1"
status = "active"
source = ".ai/repo/rules/vendor/majordomus/rules/a-credential-never-reaches-an-agent.v1.md"
+++
{% raw %}

## Rationale

An agent that runs as the same user as the person it works for can read every file that user
can read and the arguments and environment of every process that user runs. Mode 0644 on a
log, a token on a command line or a variable in an inherited environment is therefore not a
private place. It is a place the agent reads.

Observed in OpenTTD, a public fork that adopted Majordomus, over three days with about ten
coding agents sharing one workstation:

- A local web panel printed its owner secret to stdout when it started. The launcher sent
  stdout to a log of mode 0644, so every agent could read the secret that the panel used to
  tell its owner apart from those agents.
- The launcher then opened the panel in the browser with the secret in the URL, which put it
  in the argument list of `open` and in the browser's history.
- Helper processes the launcher started inherited the user's whole environment, including
  personal access tokens for two code hosts and the agent runtime's own messaging token.
  None of the helpers needed any of them.
- A secret that changed on every restart was found again by reading the newest log. So the
  log became the way to obtain the secret, which is the opposite of what the secret was for.

Each step was the convenient default. Together they let anything running as the user act as
the owner.

## Required behaviour

- **Read it from a file the owner alone can read.** A credential lives in a file of mode
  0600 inside a directory of mode 0700. A process refuses to start when that file can be
  read by anyone else, and says which `chmod` fixes it.
- **Say where it lives, not what it is.** On start, a process prints the path of its
  credential file and a URL without the credential in it.
- **Never in a log, an argument or a URL.** No credential appears in stdout, stderr, a log
  file, a process's argument list or a URL query. To sign a browser in, a process issues a
  one-time code that expires within a minute, and answers it with a redirect to the same
  URL without the query.
- **Not in an agent's environment.** A launcher that starts helpers passes each of them only
  the variables it needs. It does not hand on the user's tokens. A helper that must
  authenticate to the launcher gets a credential minted for that run, kept in a 0600 file
  and never given the owner's rights.
- **Logs are private by default.** A log a service writes is mode 0600, or lives in a
  directory of mode 0700.
- **When it leaked, rotate it.** A credential that was in a readable log, an argument list or
  an inherited environment is replaced. Deleting the log does not take the credential back.

## What is not this rule

- The text Majordomus itself hands back or sends is already redacted by
  `apps/majordomus-cli/src/redaction.rs`. That covers Majordomus's answers, not what an
  adopter's own programs write.
- The inputs of a Majordomus execution follow `project.executions-carry-no-secret` in this
  repository.
- A person may still read their own credential. The rule is about what an agent running as
  that person can read, not about locking the person out.
{% endraw %}
