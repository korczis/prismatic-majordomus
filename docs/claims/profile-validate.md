# Every profile is parsed, its unknown keys rejected, and the default profile proven to exist

## What it means

Each file under `.majordomus/profiles/` must parse, must carry only known keys, must have a `name` equal to its filename, and the profile the policy names as default must exist as a file. A policy that points at a profile nobody wrote is a failure, not a fallback to something else.

## How it works

`lib/doctor.sh` iterates the profile directory, flattens each file with the same parser the policy uses, checks keys against `share/allow/profile.txt`, compares `name` with the filename, and finally looks up `profiles.default` from the policy on disk. `majordomus start` performs the same lookup for the profile a task names and refuses with exit code 12 when the file is absent.

## How to see it

```bash
sed -i.bak 's/^name: routine/name: other/' .majordomus/profiles/routine.yaml
majordomus doctor
# FAIL profiles    routine — name field 'other' does not match filename
```

## What it does not cover

Doctor does not judge whether a profile's values are sensible for your team; `effort: max` on `routine` parses fine. It validates shape and existence.

## Why it exists

Four mutually inconsistent, unenforced status vocabularies and profiles that existed only in prose were found in the source environment. A profile is only a decision if a command can read it and fail when it is missing.
