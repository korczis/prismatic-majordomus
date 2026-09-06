+++
title = "Every generated instruction file carries its own stamp, and a hand edit is detected"
description = "When update writes a projection it stamps it: the first line of a file-mode target, or the begin marker of a region-mode target, names the hash of the policy it was rendered from and the hash of the content the stamp covers. doctor and watch hash the content on disk and compare it with the stamp; a difference means somebody edited a generated file by hand, and it is reported by name. Nothing is recorded anywhere else, so a fresh clone is checked exactly as the checkout that generated the file was."
weight = 20
[extra]
claim_id = "projection-fingerprint"
status = "guaranteed"
source = "docs/claims/projection-fingerprint.md"
+++
{% raw %}

## What it means

When `update` writes a projection it stamps it: the first line of a file-mode target, or the begin marker of a region-mode target, names the hash of the policy it was rendered from and the hash of the content the stamp covers. `doctor` and `watch` hash the content on disk and compare it with the stamp; a difference means somebody edited a generated file by hand, and it is reported by name. Nothing is recorded anywhere else, so a fresh clone is checked exactly as the checkout that generated the file was.

## How it works

`lib/update.sh` renders each target, computes the content hash, and writes the stamp line (`mj_stamp_line` in `lib/common.sh`) or the marker payload above the content. `lib/doctor.sh` reads the stamp back (`mj_stamp_read`), hashes the owned content (`mj_owned_content`) and fails on a mismatch (`content differs from its own stamp (hand-edited?)`), on a target with no stamp, and on a missing target; `lib/watch.sh` additionally reports policy drift when the policy hash a stamp names is not the policy on disk, meaning the projection is stale.

## How to see it

```bash
echo "my own rule" >> CLAUDE.md
majordomus doctor
# FAIL projection  CLAUDE.md — content differs from its own stamp (hand-edited?)  [reproduce: majordomus update --diff CLAUDE.md]
```

## What it does not cover

The stamp tells you a file changed, not what the change meant. `update --diff <target>` shows the difference so a person can decide whether the edit belongs in the policy body. A stamp is evidence about the content under it; it is not a signature and proves nothing about who generated the file.

## Why it exists

Hand edits to generated files are how two rulebooks come to exist. A stamp inside the file makes the moment of divergence visible in one command instead of months later, and makes it visible on every clone rather than only where a provenance file happened to be committed.
{% endraw %}
