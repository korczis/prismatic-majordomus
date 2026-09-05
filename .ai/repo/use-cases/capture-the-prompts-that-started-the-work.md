---
id: capture-the-prompts-that-started-the-work
kind: use-case
title: 'Keep the prompts that started the work, below the model rather than around it'
summary: 'Wire the provider hook, prove it by running a payload through it, and see that a prompt the tool could not parse is reported rather than silently dropped.'
category: continuity
status: active
target: guaranteed
actors: [maintainer, agent]
difficulty: basic
commands: [capture]
doctrines: [majordomus.prompt-capture]
claims: [prompt-capture]
responsibilities: [state]
applications: [long-running-work]
scenario:
  setup: capture-installed
  given:
    - 'Majordomus installed, with the provider hook written and the executable on PATH'
  steps:
    - id: is-it-real
      run: ['capture', 'status']
      note: 'the state is read from the provider configuration and a payload driven through the shim, not from a flag somebody set'
      expect:
        exit: 0
        stdout_contains: ['claude-code', 'majordomus-capture']
    - id: one-prompt-one-record
      run: ['capture', 'prompt', '--provider', 'claude-code']
      stdin: claude-code-prompt.json
      note: 'what the provider sends on UserPromptSubmit; the record is the person half of the exchange'
      expect:
        exit: 0
        files_exist: ['.ai/local/prompts']
    - id: never-rejects-a-prompt
      run: ['capture', 'prompt', '--provider', 'claude-code']
      stdin: not-a-payload.txt
      note: 'in a hook, exit 2 rejects the person prompt, so a payload the tool cannot read is written to the failure log and the turn goes on'
      expect:
        exit: 0
        files_exist: ['.ai/local/prompts/.capture.log']
    - id: a-subcommand-that-is-not-one
      run: ['capture', 'nonsense']
      note: 'the command line still refuses what a person typed wrong: only the hook path is forbidden to exit 2'
      expect:
        exit: 2
        stdout_contains: ['unknown subcommand']
  then:
    - 'the archive is written under the ignored half of the layer and never committed'
    - 'a prompt the tool could not read is a named failure, not a silence'
---

# Situation

The prompt that started the work is the one thing nobody keeps. It lives in a provider transcript that the next session cannot read, so six weeks later the repository holds the change and no record of what was asked for.

# Outcome

The prompts are captured below the model, by the provider hook, into files the repository ignores and nothing loads into a context. Whether the wiring is real is answered by running a payload through it rather than by a claim, and a payload that cannot be read is reported instead of dropped.
