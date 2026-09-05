---
id: prove-performance-with-benchmarks
kind: use-case
title: 'Prove the tool is fast, not assume it, and refuse a regression'
summary: 'Measure every public command and every capability of the executable against a committed baseline, and let the check fail on a regression rather than a reviewer noticing later.'
category: performance
status: active
target: guaranteed
weight: 8
actors: [maintainer, reviewer]
difficulty: intermediate
commands: [bench, doctor]
doctrines: [majordomus.verification-integrity, majordomus.enforcement-wiring, majordomus.policy-completeness]
claims: [benchmark-coverage-derived, hot-path-no-rebuild, rust-hot-path-benchmarks, rust-evidence-gates]
responsibilities: [doctor]
applications: [ci-gated-project]
scenario:
  setup: installed-wired
  given:
    - 'installed and wired, nothing measured yet'
  steps:
    - id: the-targets
      run: ['bench', '--list']
      note: 'every public command of the registry is a target; nothing is listed by hand'
      expect:
        exit: 0
        stdout_contains: ['^doctor +read-only', '^start +state-mutating']
    - id: measure-one
      run: ['bench', 'version', '--samples', '1', '--warmup', '0', '--mode', 'cold', '--no-save']
      note: 'one cold run, not saved'
      expect:
        exit: 0
        stdout_contains: ['^version +cold +ok +1 ']
    - id: within-budget
      run: ['doctor']
      note: 'doctor reports its own wall time against the policy budget'
      expect:
        exit: 0
        stdout_contains: ['budget', 'doctor: 0 failure']
  then:
    - 'a latency figure comes from a recorded run, never from prose'
---

# Situation

A refactor made doctor slower and nobody measured; a hot path in the executable rebuilds the index on every request and the only evidence is a feeling that MCP calls got sluggish. Performance claims exist in the README and nowhere else.

# What you run

- `bench`: samples every public command of the shell tool, cold and warm, after warm-up runs, and writes a result document under the local half; --check refuses a regression larger than the fractions the policy declares
- `doctor`: reports its own wall time against the budget in the policy, as a warning that names the key and never as the exit code

# Outcome

Every externally callable operation is a benchmark target derived from the registry, directly and over each transport it is exposed on, with a coverage table that CI checks is complete. Hundreds of MCP or HTTP requests rebuild nothing, and the counters prove it. The hot paths of the executable carry criterion benchmarks that build on every push, and the thresholds and budgets are policy, measured and changed with a run.
