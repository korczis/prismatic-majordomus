+++
title = "The canonical policy is parsed and rejected if it contains an unknown key"
description = ".ai/repo/policy.yaml is read by a parser that knows exactly which keys may appear at every level. A key it does not know — a typo, a field from a newer version, a field someone invented — is an error, not a silent no-op. The same rule applies to every profile file."
weight = 15
[extra]
claim_id = "policy-parse"
status = "guaranteed"
source = "docs/claims/policy-parse.md"
+++
{% raw %}

## What it means

`.ai/repo/policy.yaml` is read by a parser that knows exactly which keys may appear at every level. A key it does not know — a typo, a field from a newer version, a field someone invented — is an error, not a silent no-op. The same rule applies to every profile file.

## How it works

`lib/common.sh` carries a small YAML parser written for the subset the policy uses: nested maps, block lists, lists of maps, inline lists, quoted scalars and comments. It flattens a file into `dotted.path=value` lines. `lib/doctor.sh` then compares every key against an allowlist of regular expressions in `share/allow/policy.txt` (and `share/allow/profile.txt` for profiles). Any key with no match is reported with the file and the key name. The parser refuses what it cannot represent honestly — tabs, anchors, flow maps, multi-line scalars — rather than guessing.

## How to see it

```bash
echo "nonsense: 1" >> .ai/repo/policy.yaml
majordomus doctor
# FAIL policy      .ai/repo/policy.yaml — unknown keys: nonsense  [reproduce: grep -nE 'nonsense' .ai/repo/policy.yaml]
```

## What it does not cover

Values are validated only where a command needs them (a duration, a number, a profile name). A well-formed key with an implausible value passes the parser and fails later in the command that reads it, with a message from that command.

## Why it exists

Two agents re-entered a linted catalogue in the source environment carrying four invented frontmatter keys and a dated model pin; nothing stopped them because unknown keys were ignored. A configuration format that accepts anything cannot be trusted to have said anything.
{% endraw %}
