# Milestone 3 — Prove the 0.15.0 intent contract

Goal: demonstrate release semantics are real, deterministic and enforced.

## A. Verdict ownership

Prove canonical intent verdict comes from intent criteria/evidence, generic completion cannot independently decide it, typed states are explicit, and unknown never silently becomes satisfied.

## B. Revision-aware evidence

Prove with minimal fixtures:
- evidence current for judged revision R can satisfy R;
- same evidence does not automatically satisfy materially changed R+1 when freshness policy requires new evidence;
- stale evidence is detectable and explained;
- current replacement evidence restores satisfaction;
- result is deterministic.

## C. Preflight purpose mapping

Cover:
1. issue serves criteria -> allowed and mapping explained;
2. legitimate maintenance/non-intent work -> explicit typed allowance only;
3. expected intent work unbound -> refused;
4. missing criterion reference -> validation failure;
5. unresolved intent/milestone/issue links -> validation failure where applicable;
6. criterion/work coverage gap -> current policy response.

No shell-only exemption hacks.

## D. Canonical ledger/satisfaction

Search for duplicate verdict/satisfaction implementations, frontend completion booleans, GitHub-status shortcuts. Remove legacy duplication only if clearly in scope and safe.

## E. Intent CI class

Prove it is invoked, blocking failures exit non-zero, CI does not swallow the code, and a mutation/broken fixture makes it red.

Deliver matrix:

```text
contract | canonical implementation | test/case | command | result
```
