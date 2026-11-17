# Every observation candidate is routed by the structure of what it is about

## What it means

A candidate derived from an observation says which owner should look first — `platform`, `enforcement`, `generator`, `documentation` or `project` — read from its subject's structure and never from its words. In a repository that adopted the tool, the project's own `share/` is the project's, and a project's defect is never routed into the tool's governance.

## How it works

In this order: a `command:`, `capability:` or `rule:majordomus.*` subject, a path under the vendored rules directory, or a path under the tool's own `share/` only when that directory lies inside the repository, routes `platform`; another `rule:` or a `gate:` routes `enforcement`; a path the scope declares generated (`out.generated` paths or names) or `.gitattributes` marks `merge=derived` routes `generator`; a path under `docs/` routes `documentation`; anything else routes `project`. Two observations about one subject in different words carry the same route. The derivation writes no event it reads, so a derivation never starts another.

## How to see it

```bash
majordomus knowledge observe --kind friction --subject command:finish "finish discards the verifier's output"
majordomus knowledge observe --kind drift --subject docs/CLI.md "the start syntax lacks --issue"
majordomus session end
majordomus knowledge candidates             # about command:finish  route platform ...; about docs/CLI.md  route documentation ...
```

## What it does not cover

The route decides nothing: it opens no issue, writes no rule and sends nothing to another repository. Turning a routed candidate into work is an act and a later milestone.

## Why it exists

A candidate with no owner is read by nobody, and ADR 0091 rejected classifying by wording. `.ai/repo/adrs/0118-a-session-leaves-what-it-observed-and-one-candidate-per-defect-is-routed.md` derives the owner from structure, so the same subject always routes the same way, on any machine.
