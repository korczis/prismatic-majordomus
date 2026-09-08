# The task lifecycle: the shell tool's own commands, routed here so that `just` lists
# everything a person runs in this repository, whichever executable implements it.

# What the next worker needs to know now, within budget.
[group('lifecycle')]
context *args:
    "{{shell_bin}}" context {{args}}

# Is Majordomus itself healthy and wired here?
[group('lifecycle')]
doctor *args:
    "{{shell_bin}}" doctor {{args}}

# What has drifted: state, policy, projections, retention.
[group('lifecycle')]
watch *args:
    "{{shell_bin}}" watch {{args}}

# Is the current task consistent with policy, scope and state?
[group('lifecycle')]
check *args:
    "{{shell_bin}}" check {{args}}

# Regenerate AGENTS.md, CLAUDE.md and the other projections from the policy.
[group('lifecycle')]
update *args:
    "{{shell_bin}}" update {{args}}

# The repository's skills: list, show <id>, or check every one against its contract.
[group('lifecycle')]
skills *args:
    "{{shell_bin}}" skills {{args}}
