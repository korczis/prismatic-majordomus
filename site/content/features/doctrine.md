+++
title = "Rules a machine decides, wired, tested and CI-blocking"
description = "Every rule is a portable Markdown object with front matter, and it is enforced in one of three declared modes: a validator the dispatcher runs, a gate or case that proves it, or a stated reason why no program can express it. Doctor walks the dispatch chain from the source, refusing a validator nobody declares and a declaration nothing runs; the proof graph walks the other relation, joining each rule to the tree and to the runs recorded against it, so that a rule naming a case which was deleted is reported as reading enforced rather than being it."
weight = 70
[extra]
id = "doctrine"
status = "stable"
source = ".ai/repo/features/doctrine.md"
+++
{% raw %}

## What it does

The effective rule set is the vendored baseline the tool ships plus the rules this
repository wrote, resolved as a dependency graph: a missing dependency, a cycle or two rules
claiming one identity is an error, and a set that does not resolve is not applied at all. A
rule's class is `blocking` or `advisory` and there is no third; the class is read at
dispatch time and is what routes a finding, so changing one word in a rule file changes
whether `majordomus check` exits zero — and a test asserts exactly that.

`majordomus doctor` reads the source rather than a rule's description of itself: the
validator function must exist, the commands the rule names must dispatch it, a blocking
rule must be able to exit non-zero, a test must prove it, and CI must run that test without
swallowing its exit code. A validator no rule declares is enforcement running under no
rule, and fails.

## What it does not do

A rule without a validator is normative for whoever reads it and enforced by nobody, and it
says so; the tool does not pretend otherwise. There is no severity ladder, no baseline and
no override: a project rule may add a constraint and may never weaken a vendored one.
{% endraw %}
