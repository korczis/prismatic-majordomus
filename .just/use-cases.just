# Executable use cases: what a person does with the tool, run against the real tool in
# disposable repositories.

# Every use case: id, category, status, whether it has a scenario, the commands it runs.
[group('use-cases')]
usecase-list *args:
    bin/majordomus usecase list {{args}}

# Execute every scenario against the real tool in disposable repositories; evidence under .ai/local/evidence/use-cases/.
[group('use-cases')]
usecase-run *args:
    bin/majordomus usecase run {{args}}

# Every public command, guaranteed claim and MCP tool against the use cases that name and run it; exit 10 on a required gap.
[group('use-cases')]
usecase-coverage *args:
    bin/majordomus usecase coverage --check {{args}}

# What a change reaches: the commands, rules, use cases, scenarios and cases, from the files changed since the upstream.
[group('use-cases')]
usecase-impact *args:
    bin/majordomus usecase impact {{args}}
