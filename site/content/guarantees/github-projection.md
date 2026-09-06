+++
title = "GitHub milestones and issues are generated from the canonical model, and a hand-edited generated region is reported rather than overwritten"
description = "scripts/github-sync renders each canonical record through majordomus plan body and projects it onto a GitHub milestone or issue: title, body region, milestone assignment, labels, and whether the issue is open. Comments, assignees and any text a human writes outside the generated region belong to GitHub and are never touched. A human edit *inside* the generated region is reported as drift and left alone."
weight = 95
[extra]
claim_id = "github-projection"
status = "guaranteed"
source = "docs/claims/github-projection.md"
+++
{% raw %}

## What it means

`scripts/github-sync` renders each canonical record through `majordomus plan body` and projects it onto a GitHub milestone or issue: title, body region, milestone assignment, labels, and whether the issue is open. Comments, assignees and any text a human writes outside the generated region belong to GitHub and are never touched. A human edit *inside* the generated region is reported as drift and left alone.

## How it works

The body is spliced between `<!-- majordomus:begin <hash> -->` and `<!-- majordomus:end -->`, the same region mechanism that lets Majordomus share a `CLAUDE.md` with a hand-written one. The hash in the begin marker is the hash of the canonical record the region was generated from, which is what separates the two cases: a different hash means the plan moved and the region is refreshed; the same hash with different content means a person edited it, and `--apply` refuses without `--force`.

The adapter lives in `scripts/` rather than in `lib/`, because `bin/`, `lib/`, `share/` and `test/` contain no network client and `test/cases/08_no_forbidden_constructs.sh` proves it. The model it projects comes from the same `lib/project.sh` the CLI uses.

## How to see it

```bash
scripts/github-sync --plan        # offline: what would be created or changed, and the body hashes
scripts/github-sync --check       # compare against the live repository; exit 11 on drift
scripts/github-sync --apply       # create what is missing, update what canonically changed
```

## What it does not cover

Nothing is read back. Closing an issue on GitHub does not complete it canonically; the next `--check` reports the disagreement and the canonical record wins. There is no mapping file either — a record is matched by the id that prefixes its title, so renaming that prefix on GitHub orphans the issue.

## Why it exists

GitHub is where the conversation happens and a poor place for the plan to live: it has no dependency graph, no validation, and no way to refuse a status that contradicts one. Making it a projection keeps the collaboration and moves the truth into the repository.
{% endraw %}
