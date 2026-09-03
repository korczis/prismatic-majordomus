# Every generated instruction file is fingerprinted, and a hand edit is detected

## What it means

When `update` writes a projection it records the file's hash in `.majordomus/generated/fingerprints.yaml`, together with the hash of the policy that produced it. `doctor` and `watch` compare the file on disk with its fingerprint; a difference means somebody edited a generated file by hand, and it is reported by name.

## How it works

`lib/update.sh` writes `fingerprints.yaml` with `policy_sha256`, a timestamp, and one `sha256` and line count per target. `lib/doctor.sh` fails on a mismatch (`hash differs from fingerprint (hand-edited?)`) and on a target with no fingerprint; `lib/watch.sh` additionally reports policy drift when the policy on disk no longer matches `policy_sha256`, meaning the projections are stale.

## How to see it

```bash
echo "my own rule" >> CLAUDE.md
majordomus doctor
# FAIL projection  CLAUDE.md — hash differs from fingerprint (hand-edited?)  [reproduce: majordomus update --diff CLAUDE.md]
```

## What it does not cover

The fingerprint tells you a file changed, not what the change meant. `update --diff <target>` shows the difference so a person can decide whether the edit belongs in the policy body.

## Why it exists

Hand edits to generated files are how two rulebooks come to exist. Fingerprinting makes the moment of divergence visible in one command instead of months later.
