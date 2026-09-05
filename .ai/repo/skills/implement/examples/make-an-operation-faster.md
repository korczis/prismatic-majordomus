# Make an operation faster

```text
Apply the implement skill to make `majordomus doctor` finish under its budget.
Record the baseline (MJ_TIMING=1 majordomus doctor, majordomus bench), find the phase
that dominates, change it, compare against the baseline, and add a regression guard only
where the contract justifies one.
```

Expected shape: baseline → bottleneck identified → change → comparison → regression
guard where justified.
