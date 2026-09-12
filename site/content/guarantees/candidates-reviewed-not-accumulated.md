+++
title = "More candidates than the policy cap, or a candidate older than the policy age, is an advisory doctor finding that names them; both numbers are declared once in the policy"
description = "The review queue is measured. doctor prints WARN knowledge and exits 0 when more records under candidates/ carry status: candidate than knowledge.candidates_max_files, when a candidate has waited longer than knowledge.candidate_max_age_minutes, or when a candidate is present on disk and not tracked by version control. Each finding names the paths. Both numbers are declared once in .ai/repo/policy.yaml and read by every surface, and a policy that declares neither is reported at the rule's own class, naming the missing key."
weight = 191
[extra]
claim_id = "candidates-reviewed-not-accumulated"
status = "guaranteed"
source = "docs/claims/candidates-reviewed-not-accumulated.md"
+++
{% raw %}

## What it means

The review queue is measured. `doctor` prints `WARN knowledge` and exits 0 when more records under `candidates/` carry `status: candidate` than `knowledge.candidates_max_files`, when a candidate has waited longer than `knowledge.candidate_max_age_minutes`, or when a candidate is present on disk and not tracked by version control. Each finding names the paths. Both numbers are declared once in `.ai/repo/policy.yaml` and read by every surface, and a policy that declares neither is reported at the rule's own class, naming the missing key.

## How it works

`mj_validate_knowledge_accumulation` in `lib/knowledge.sh` reads both keys with `mj_pol_req`, counts the files whose status is `candidate` (a superseded record stays in the directory and counts toward neither), and measures each candidate's review age from the oldest `knowledge.derived` line whose `paths` names the file, falling back to the commit that added the file and only then to the record's `date`; the finding says which source it used. Untracked files are read from `git status --porcelain` over the directory, and the remedy `git add .ai/repo/knowledge/candidates && majordomus derive` follows `fix:`. The rule `majordomus.candidates-reviewed` is advisory and dispatched from `doctor` only, because under `watch` an advisory finding is drift and exits 11, and a full review queue must not turn `watch` red in a hook.

## How to see it

```bash
majordomus doctor | grep -E 'knowledge'                 # OK knowledge ... or WARN knowledge candidates ... over cap
majordomus knowledge candidates                         # the queue the cap is measured over
git status --porcelain .ai/repo/knowledge/candidates    # the untracked ones a WARN names
git add .ai/repo/knowledge/candidates && majordomus derive
majordomus watch; echo "exit $?"                        # unaffected by this rule
```

## What it does not cover

It empties nothing. Whether a candidate deserves promotion is a person's judgement, and a machine that emptied the queue would be making it. It does not measure `curated/`, which is authored content, and it does not count superseded records, which are the rejections a person already made.

## Why it exists

The deriver writes a candidate at every episode boundary that produced evidence, and nothing removes one except a person. Left alone, the directory grows, and `project.accumulation-is-measured` says that anything which grows on its own is measured and reported before it is a problem. The age is the queue entry's and not the evidence's because a record's `date` is the evidence day, kept deterministic, and a candidate derived today from last month's decision has been waiting one day. `.ai/repo/adrs/0058-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md` records both.
{% endraw %}
