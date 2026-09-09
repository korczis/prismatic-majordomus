# A release publishes an artifact for every supported target or it is not published

## What it means

`share/distribution.yaml` marks a target `supported`, `experimental` or `unavailable`. A
release publishes an artifact for every target in the first two states. There is no state
in which a release exists, is offered by `latest.json`, and has nothing for a platform the
documentation lists.

## How it works

Three independent refusals, at three moments:

- the build matrix is `fail-fast`, so one target failing to build stops the run;
- publication is a separate job that begins only when every build has succeeded;
- `scripts/release-record`, which writes the record from the artifacts that were actually
  uploaded, refuses to write one that is missing a supported target, and the typed check
  behind it (`Release::findings`) is also what `majordomus distribution validate` runs over
  every committed record.

Completeness is judged **against the set the release was published against**, which the
record snapshots in `required_targets` rather than recomputing. Adding a platform to the
model is a promise about the next release: `v0.3.1` cannot grow a `riscv64` artifact however
the model changes afterwards, and a check that said otherwise would turn every historical
record invalid the day a target was added — which is what it did, until the first real
release made it visible. The enforcement point is publication, where `release-record` sees
the model of its own day and refuses a partial release with exit 10. A record written before
the field existed carries no set and is judged on internal consistency alone.

The public metadata is generated from that record, so a record that could not be written is
metadata that does not exist, and `latest.json` keeps pointing at the release before it.

## How to see it

```bash
majordomus distribution validate            # every record against the model
bash test/run.sh 87_release_pipeline        # including a recorder given an empty directory
```

## What it does not cover

A target the model marks `unavailable` is documented with its reason and is not built; the
installer refuses it by name rather than pretending. That is the supported way to not ship
a platform — silently omitting one is what this claim forbids.

## Why it exists

The half-published release is the classic distribution failure: the tag looks green, the
release page has five archives instead of six, and the sixth platform's users get a 404
from an installer that was told the platform exists. Making completeness a property of the
*record* rather than of anyone's attention means the failure happens in CI, before anyone
has downloaded anything.
