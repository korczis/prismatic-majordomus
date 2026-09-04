# A projection can own only the region between its markers

## What it means

A projection declared with `mode: region` owns the text between `<!-- majordomus:begin <sha> -->` and `<!-- majordomus:end -->` and nothing else. Everything outside those two lines belongs to the repository, is never read as input, and is never rewritten. `update` splices the rendered body into the region; `doctor` and `watch` fingerprint the region, not the file.

## How it works

`lib/common.sh` carries the primitives: `mj_region_extract` returns the current region body and distinguishes *no begin marker* from *malformed markers*; `mj_region_splice` rebuilds the whole file around a new body; `mj_projection_mode` defaults an unset `mode` to `file`, so every existing policy keeps its behaviour. `lib/update.sh` renders the provider template into the region and records `mode: region` plus the region's hash in `.majordomus/generated/fingerprints.yaml`. An unknown mode is a policy error and exits 10 rather than guessing. `lib/doctor.sh` and `lib/watch.sh` read the mode from the fingerprint and compare the region; the budget, link and count checks measure the region and report the host document's length as information, because the host is not the tool's to shrink.

## How to see it

```bash
majordomus update                 # writes the region into CLAUDE.md
echo "a paragraph of my own" >> CLAUDE.md
majordomus doctor                 # ok — the edit is outside the region
sed -i '' 's/one active task/two active tasks/' CLAUDE.md
majordomus doctor
# FAIL projection  CLAUDE.md — region hash differs from fingerprint (hand-edited?)
```

## What it does not cover

The tool does not create the markers for you in a file it did not write. If the begin marker is missing, `update` appends the region at the end of the file; where in the document the region belongs is an editorial decision. Markers that are present but malformed — a begin without an end, or an end before its begin — are refused rather than repaired.

## Why it exists

A repository that already has a hand-authored `CLAUDE.md` had to choose between keeping its own governance root and adopting the tool. `mode: region` removes the choice: the authored text stays authoritative and the generated supervisory section sits inside it, fingerprinted like any other projection.
