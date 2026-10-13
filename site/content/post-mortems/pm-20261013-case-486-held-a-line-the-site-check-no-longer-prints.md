+++
title = "Case 486 asserted a line of the site check that a landing had reworded"
description = "Case 486 asserted a line of the site check that a landing had reworded"
weight = 5
[extra]
kind = "post-mortem"
slug = "pm-20261013-case-486-held-a-line-the-site-check-no-longer-prints"
identity = "pm-20261013-case-486-held-a-line-the-site-check-no-longer-prints"
status = "resolved"
source = ".ai/repo/post-mortems/pm-20261013-case-486-held-a-line-the-site-check-no-longer-prints.md"
+++
{% raw %}

## Case 486 asserted a line of the site check that a landing had reworded

### What failed

`suite-shard (3)` failed in two `validate` runs with `FAIL 486_rustdoc_composition` after
about 870 seconds, while the site check inside the case reported `site-check: 0
failure(s)`: the check passed and the case's own expectation did not.

### Root cause

The case held `^OK   nav +every navigation link on the homepage resolves$`. Pull request
1045 (`b34259715e`) made the landing page link the published UI, and the site check's line
gained `(N of them the root of a snapshot the publication holds and this build does not
compose)`. The anchored pattern no longer matched. Whether shard 3 reported on the pull
request's own head before it was merged was not established for this record.

### Fix

`ba49c586c2` (pull request 1079) holds the sentence with the parenthetical and leaves the
count to the publication.

### Prevention

The case itself is the guard, and it takes fourteen minutes: a change to `scripts/site-check`
output runs case 486 before it is pushed.
{% endraw %}
