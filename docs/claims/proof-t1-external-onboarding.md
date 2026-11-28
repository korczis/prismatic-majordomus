# A proof test will record an adoption in a repository not built around Majordomus

## What it means

**Planned.** Test `t1-external-onboarding` of the proof plan will record whether a coding agent adopts Majordomus in a repository that was not built around it. It follows the adoption from the published installer to an accepted finish and records what the adoption costs that repository.

## How it works

The test is registered in `.ai/repo/benchmarks/proof-plan.yaml` (schema `proof-plan/v1`) with its question, its dimension, the words a result would be stated in and its instrument. `economics::proof::load` validates the plan fail-closed. The test has no protocol yet. Its own experiment registers one, frozen by its own digest, and until then no record counts toward it.

Its instrument is planned. The adoption workflow (I3359), the candidate floors and the trials protocol (I3360), the first-run stage of install-check (I3361), the measured candidates (I3362), the adoption itself (I3363), the overhead in the adopted repository (I3364, which reuses the overhead bench of I3299) and the friction ledger (I3365) build it. The typed trial record of I3353 holds its records.

## How to see it

```bash
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test economics test_tracked_proof_plan_has_six_tests
majordomus economics check
```

## What it does not cover

This claim registers the test and its instrument. It does not set the protocol, threshold, sample size or corpus, it states no result, and it asserts no advantage. The claim cannot become guaranteed until its test reads passed.

## Why it exists

Adopting a tool in its own repository proves little about anyone else's. This claim names the test that will show what happens in a repository with its own history and conventions, and the test's row reads not run until it does.
