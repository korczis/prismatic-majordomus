# A deployment that breaks its closed contract is refused by name, with no registration anywhere

## What it means

A deployment of the executable is one YAML object under the layer's deployments section,
read against the `deployment/v1` contract. `majordomus doctor` refuses an object that could
not work or must not exist, and names the object and the reason when it does:

- a key the contract does not have, including a field invented to hold a token;
- a schema version other than the one this tool reads;
- a field every deployment states, missing;
- a listen interface that is neither `loopback` nor `all`;
- a port outside the unprivileged range, since the deployed process does not run as root;
- more machines kept running than the deployment has;
- a health route written as a URL rather than a path;
- a value that reads as a credential, in a field that does exist;
- two objects claiming one identity.

Nothing is registered for any of it. The section, its contract and its source class are
seeded by `init`, so a repository that deploys something adds one file, and the index, the
object listing and `doctor` follow the kind as data.

## How it works

`mj_validate_deployments` in `lib/deployment.sh` is the validator the vendored rule
`majordomus.deployment-contract` declares, dispatched from `doctor` and `watch`. It
flattens each object, compares its keys with `share/allow/deployment.txt` (an allow-list
generated from the contract's schema, never written by hand), and then checks the schema
version, the required fields, the interface, the port range, the machine counts, the health
routes, the values that match known credential prefixes and the identities already seen.
Each violation is a `FAIL` finding under the `deployment` category with the command that
shows the offending lines, and the command exits 10.

`test/cases/84_deployment_contract.sh` proves it by mutation. It initialises a repository,
confirms an empty section is not a finding, adds one valid deployment and confirms `doctor`
accepts and counts it, then applies each violation above to that object in turn and
requires `doctor` to refuse it with the reason named. It also checks that the allow-list is
the projection of the schema, and that the restored object is accepted again, so each
refusal is known to come from the mutation rather than from the shape of the fixture.

## How to see it

```bash
majordomus doctor                                  # FAIL deployment <object> — <reason>
bash test/run.sh 84_deployment_contract            # the refusals, one mutation at a time
cat share/allow/deployment.txt                     # the keys the contract allows
```

## What it does not cover

This is the half of the contract that is decided without a Rust toolchain. Whether a
deployment works against this repository in particular, whether its health routes are
routes a capability registers and whether its package and build inputs exist in the
workspace, is decided by `apps/majordomus-cli/src/deploy/mod.rs` and proven by that
module's own tests, not by this case.

The credential check matches the prefixes of known token formats. A secret in a format it
does not know, written into a field the contract allows, is not recognised as one.

It does not prove that the generated container definition and provider configuration match
the object; `majordomus generate --check` holds that, and its own case proves it. Nothing
here contacts a hosting provider or proves that a deployment, once accepted, will start.

## Why it exists

A deployment that cannot work should cost a failing test rather than a failed rollout, and
a credential must never reach a file every clone of the repository carries. Both failures
are cheap to find while the object is being written and expensive after it has shipped, so
the contract is closed and checked locally, and every refusal says which object and why,
so the fix is one edit rather than an investigation.
