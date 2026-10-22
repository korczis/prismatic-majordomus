+++
title = "A deployment that serves the expected commit while stating a version its commit does not declare is refused, never reported as served"
description = "When the site serves the expected commit, the version it tells its reader is the version that"
weight = 42
[extra]
claim_id = "the-served-version-is-the-declared-one"
status = "guaranteed"
source = "docs/claims/the-served-version-is-the-declared-one.md"
+++
{% raw %}

## What it means

When the site serves the expected commit, the version it tells its reader is the version that
commit declares. A deployment that serves the right commit with another version on its pages
is refused as `mismatched`, a measured no with exit `10`, never reported as served.

## How it works

The site publishes its build identity (`/build.json`): the commit it was built from and the
version its derived data carried. `majordomus served observe` fetches it, judges containment
of the expected commit, and then holds the stated `source_version` to the version the served
commit declares in its authored manifest, read through git. `scripts/ci/pages-check`, the
`pages-live` gate `finish` runs at completion, asks `served` for that verdict rather than
restating the rule.

## How to see it

```bash
majordomus served observe --commit origin/master --dry-run
# pages  served  the deployment serves e45ce7c3e32a at version 0.12.0
```

## What it does not cover

A build that states no version keeps its commit's verdict and says the version was not judged,
and so does a commit whose declaration cannot be read. In a CI job without a built executable
the gate reports the version as not judged rather than passing it.

## Why it exists

The right commit telling its reader the wrong version is what a deploy of stale derived data
publishes, and nothing that checks only the commit can see it.
{% endraw %}
