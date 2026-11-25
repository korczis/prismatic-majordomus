# Why a cache misses after every deploy

Intent: "Find out why reads are slow after every deploy. Use two independent lenses, have each
lens refute the other, and give me only what survives."

The worker writes the question document first. It holds two questions (what empties the cache,
and how much of the latency the misses explain), the read-only rule and the tag grammar. Two
lens workers read independently. The cache lens finds:

- the cache lives in process memory (OBSERVED, `src/cache.rs:12`);
- a deploy restarts every process (OBSERVED, the deploy log);
- eviction may drop hot keys (INFERRED: a run with eviction disabled would confirm it).

The load lens derives that half of the reads miss and that a miss costs 40 ms.

In pass 2 each lens cross-checks the other:

- The "half of the reads" claim is REFUTED. The metrics count a retried read twice, so the
  true rate is 20%.
- The eviction claim is UNVERIFIABLE under the read-only rule, so it goes to the gaps ledger.

The synthesis builds two root causes from the CONFIRMED findings and maps two remedies to
owners. One remedy is `unowned`, which marks it as a hole for someone to take. The refuted
claim goes in the register.

`majordomus reasoning analysis --path test/fixtures/analysis/conforming` reads `conforms`.
Remove a cross-check document and the same bundle reads `incomplete`, naming the check that
did not run. Cite the refuted finding as a root cause and the bundle reads `fails`.
