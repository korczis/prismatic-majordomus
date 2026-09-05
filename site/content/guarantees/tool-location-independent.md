+++
title = "The tool runs read-only from any location, and two locations of one version agree about one repository"
description = "The distribution — bin/, lib/, share/ — is not part of the repository's data. It may be a checkout under ~/tools, a package on PATH, or a copy inside the repository, and every form behaves the same because nothing writes into it. Two copies of one version, pointed at one repository, read the same files under .ai/ and produce the same findings."
weight = 74
[extra]
claim_id = "tool-location-independent"
status = "guaranteed"
source = "docs/claims/tool-location-independent.md"
+++
{% raw %}

## What it means

The distribution — `bin/`, `lib/`, `share/` — is not part of the repository's data. It may be a checkout under `~/tools`, a package on `PATH`, or a copy inside the repository, and every form behaves the same because nothing writes into it. Two copies of one version, pointed at one repository, read the same files under `.ai/` and produce the same findings.

## How it works

`lib/common.sh` derives the distribution root from the entry point that was run, never from the environment, so a majordomus started under another majordomus (a git hook, `finish --verify-command`) reads its own `share/` and not the outer one; the repository root comes from git. Every path the commands write is resolved under the repository's `.ai/` or one of the projection targets the policy names, and `test/cases/08_no_forbidden_constructs.sh` refuses a redirect into any other path variable.

## How to see it

```bash
cp -R /path/to/majordomus /tmp/dist
before=$(cd /tmp/dist && find . -type f | sort | xargs shasum -a 256 | shasum -a 256)
/tmp/dist/bin/majordomus init && /tmp/dist/bin/majordomus doctor
( cd /elsewhere && PATH=/tmp/dist/bin:$PATH majordomus --repo /path/to/repo start "task" --scope docs )
after=$(cd /tmp/dist && find . -type f | sort | xargs shasum -a 256 | shasum -a 256)
[ "$before" = "$after" ]              # the distribution is byte for byte what it was
```

## What it does not cover

Two different versions are allowed to disagree, and `rules vendor status` is how a repository learns that the executable in front of it ships a different rule package than the one it vendored. Nothing here pins a repository to a tool version.

## Why it exists

A tool that writes into its own installation cannot be shared, cannot be installed read-only, and behaves differently depending on which copy ran last. `test/cases/65_tool_root_independence.sh` runs one version from the checkout, from an unrelated absolute path and through `PATH`, compares their `doctor` findings line by line, and hashes the distribution before and after.
{% endraw %}
