# Milestones and issues are canonical repository files, and a key nobody reads is an error

## What it means

A milestone is one file under `.majordomus/project/milestones/`, an issue is one file under `.majordomus/project/issues/`, and both are hand-written YAML in the same restricted subset the policy uses. Every key in them is checked against an allowlist in `share/allow/`. A key that no code reads is a failure of `majordomus plan validate` and of `majordomus doctor`, not a comment.

## How it works

`mj_project_load` flattens each file, refuses one whose declared `id` disagrees with its filename, and refuses two files claiming the same id. `mj_project_unknown_keys` compares every flattened key against `share/allow/project.txt`, `share/allow/milestone.txt` or `share/allow/issue.txt`. The rule is the one already applied to `policy.yaml` and to every profile, so there is one answer in the repository to "what happens to a field nobody reads".

## How to see it

```bash
printf 'estimate: 3d\n' >> .majordomus/project/issues/I0001.yaml
majordomus plan validate
# FAIL schema      I0001 — unknown keys: estimate  [reproduce: majordomus plan validate]
```

## What it does not cover

The allowlist proves that a key is read by something. It does not prove that the value is meaningful: a `priority` of `p9` parses, sorts last, and is nobody's error.

## Why it exists

Every field that exists but is never read eventually appears on the website as a fact. The cheapest moment to refuse it is the moment it is written.
