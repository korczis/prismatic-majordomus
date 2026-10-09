# The body GitHub shows for an issue lists what the record declares the issue serves, generated with the rest of the body, and states nothing of where a criterion stands

## What it means

An issue whose record says which intent criteria it serves says so on GitHub too: its generated body carries a `Serves` section with each entry as the record states it. An issue that serves nothing has no such section. The section is part of the generated region, so it is held like the rest of it: change the mapping in the record and the issue reads `behind` until the projection is applied; edit it by hand on GitHub and it reads `edited`.

## How it works

`mj_plan_body_issue` in `lib/plan.sh` prints the section from the record's `serves` list, between what is out of scope and what the issue depends on. `majordomus plan body <issue>` and `scripts/github-sync --render <issue>` call the same function, so the command line and the projection cannot differ. The hash in the region's begin marker is the hash of that body, which is how a changed mapping is told from a hand edit.

## How to see it

```bash
scripts/github-sync --plan        # offline: every record with the hash of the body it would post
```

<!-- majordomus:unrun the id is whichever issue declares serves in the repository at hand -->
```bash
majordomus plan body <issue>
```

## What it does not cover

Where a criterion stands is not in the body. It moves with the evidence, and a body that moved whenever a run went stale would read `behind` on every branch with no record edited; the section names the command that shows it. A milestone's state on GitHub follows the plan and not an intent's verdict (ADR 0116), and nothing read from GitHub reaches a criterion or the plan.

## Why it exists

An issue on GitHub said what it was and nothing of why it existed. The criterion it serves was in the record and invisible where the conversation about the issue happens.
