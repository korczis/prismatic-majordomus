+++
title = "Every repository-relative reference in the always-loaded file is proven to resolve"
description = "The always-loaded instruction file may point workers at other files (docs/DESIGN.md, .ai/repo/rules/README.md). doctor extracts every repository-relative Markdown link from it and checks the target exists. A worker following a broken pointer wastes a session on a file that is not there."
weight = 33
[extra]
claim_id = "pointer-integrity"
status = "guaranteed"
source = "docs/claims/pointer-integrity.md"
+++
{% raw %}

## What it means

The always-loaded instruction file may point workers at other files (`docs/DESIGN.md`, `.ai/repo/rules/README.md`). `doctor` extracts every repository-relative Markdown link from it and checks the target exists. A worker following a broken pointer wastes a session on a file that is not there.

## How it works

`lib/doctor.sh` scans the always-loaded projection for `](path)` links that are not absolute URLs and tests each against the filesystem relative to the file's directory. Each unresolved reference is a failing finding naming the path.

## How to see it

```bash
printf '\nSee [design](docs/MISSING.md).\n' >> AGENTS.md
majordomus doctor
# FAIL projection  AGENTS.md — content differs from its own stamp (hand-edited?)
# FAIL links       AGENTS.md — reference docs/MISSING.md does not resolve  [reproduce: ls docs/MISSING.md]
```

## What it does not cover

Only the always-loaded file is checked, and only file paths; anchors within files and external URLs are not verified.

## Why it exists

The source environment's instruction hubs accumulated over a hundred dangling references and headlined five onboarding scripts that did not exist as the recommended workflow. The linter that finally caught them had a single sharp line: an unresolvable reference is an error.
{% endraw %}
