+++
title = "Prose describes the tool as it stands, without a version label"
description = "Prose describes the tool as it stands, without a version label"
weight = 130
[extra]
kind = "rule"
slug = "project-prose-states-no-version-1"
identity = "project.prose-states-no-version@1"
status = "active"
source = ".ai/repo/rules/project/prose-states-no-version.v1.md"
+++
{% raw %}

## Rationale

The public limitations page opened with "What v0.1 does not do" while the site beside it
offered v0.13.0. Nobody wrote anything false: the sentence was true the day it was typed and
then the version moved on underneath it. The same label pinned the roadmap's
description, the schema and command references, four claim pages and the economics
document's summary, and the list it headed had gone stale with it — the tool had grown
session hooks, token measurement and cross-machine claims while the page still said it had
none.

The version is authored once and derived everywhere else
(`project.release-is-a-projection`), and that rule's diagnosis looked only where the tool's
own files live. Prose was the remaining place a version could be stated by hand, and a label
there is the worst kind of statement: it reads as a scope ("in v0.1 this is not enforced")
that nobody revisits, because nothing tells the writer the scope has ended.

## Required behaviour

- Hand-written prose — the README and the bootstrap files, `docs/`, the site's authored
  pages, data and templates, the layer's own documents and the provider templates — names
  no version label: `v`, a major and a minor, and no patch (`v0.1`, `v1.2`).
- A released version is named as the record it is, three numbers (`v0.3.1`), and only when
  the sentence is about that release. Such a mention is history and stays true.
- A document that is *about* a past version — a design-phase specification, the report of
  how it was derived, a dated review — is declared in `.ai/repo/version-label-history.txt`,
  one `<path> <reason>` per line. The reason is required.
- Generated files and the layer's dated records (`.ai/repo/adrs/`, `.ai/repo/sessions/`) are
  not hand-written prose: a label in a projection is found in its source.

## Failure behaviour

`release::version::diagnose` reports `version-label-in-prose` (an error) for every label in
hand-written prose outside the history, naming the file and line, and
`version-label-history-invalid` (an error) for a history entry with no reason, one naming a
path that is not tracked hand-written prose, and one whose document no longer carries a
label — the list is held from both sides, so it cannot outlive what it exempts.
`bin/majordomus-cli release version` prints them and exits 10, and the
`version-authored-once` gate runs it on every plan.

What it does not catch, stated rather than implied: a three-part version used as a scope
("as implemented in v0.3.1") reads exactly like a mention of that release, and a present-tense
claim that went stale without any version beside it. The first is review's to catch; the
second is why each limitation names its evidence.

## Verification

```sh
bin/majordomus-cli release version          # the version-authored-once gate
bash test/run.sh 900_prose_states_no_version
```

`test/cases/900_prose_states_no_version.sh` proves the refusal against a fixture repository —
a label refused, a release mention and a declared history allowed, and each invalid history
entry refused — and that this repository's own prose is clean.
{% endraw %}
