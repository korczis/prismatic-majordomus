+++
title = "The behavioural and negative tests the subject index names for a command are exactly the ones its command page lists, both read from the first coverage and negative header of each case"
description = "A case says which public commands it exercises and which it refutes, in its own header:"
weight = 200
[extra]
claim_id = "evidence-subject-commands-agree-with-the-command-pages"
status = "guaranteed"
source = "docs/claims/evidence-subject-commands-agree-with-the-command-pages.md"
+++
{% raw %}

## What it means

A case says which public commands it exercises and which it refutes, in its own header:
`# majordomus-covers: <commands>` and `# majordomus-negative: <commands>`. Several programs
read those headers, and the subject index is one more. This claim is that the index and the
command pages never disagree about which cases prove a command: the behavioural tests and
the negative tests the index names for `command:<name>` are exactly the tests the page for
that command lists.

## How it works

The headers have several readers, each in its own file:

- the command pages, which `scripts/generate-site-data` writes to
  `site/data/generated/commands.json`, read the **first** covers line and the first negative
  line of every case;
- the command-coverage doctrine, in `lib/commands.sh`, reads the same first lines and
  refuses a word that is not a public command;
- the `command-furnished` gate, in `scripts/ci/command-furnished`, and the use-case impact
  trace, in `lib/usecase.sh`, read **any** header line.

The subject index reads the first line word for word, as the page does, and `none` names
nothing. The readers of any line differ from it in two ways. They read a second header of the
same kind, which the first-line readers ignore. And they match a command between word
boundaries rather than as a whole word, so a header word with a command inside it counts as
covering that command for them and for neither the index nor the page: the word
`capability:commit.plan` holds `plan`. So the case holds every case of this repository to at
most one covers line and at most one negative line, and holds its covers line to naming, word
for word, exactly the public commands it holds between word boundaries. Until the gate slice
moves those readers onto the index, that guard is what keeps the readings equal.

Two more shell derivations reach tests through declarations rather than headers, and the
same case holds each of them to the index:

- the capability pages, from `scripts/lib/executable-site.jq`, give every capability the
  claims implemented in the file its module is composed in, and list those claims' tests;
  the index gives `capability:<id>` exactly those claims as members, and the case compares
  both the claims and the tests;
- the doctrine pages, `site/data/generated/doctrines.json`, name the first test of each
  rule's enforcement block, which `lib/doctrine.sh` reads; the case holds every such test to
  a route or a mechanism of `rule:<id>`.

## How to see it

```bash
bash test/run.sh 507_evidence_subjects_are_derived
jq '.commands[] | {name, tests}' site/data/generated/commands.json
jq '.subjects["command:plan"].routes' site/data/registry/evidence-subjects.json
```

## What it does not cover

The command-coverage doctrine and the gates are not yet moved onto the index; they still
read the headers themselves, and the guard above is what keeps them in step. The gate slice
does that move.

The claims the capability pages attach to a whole surface — the executable, its command
line, MCP, HTTP and the benchmarks — have no subject, because no subject kind is a surface.
The index neither derives them nor compares them.

## Why it exists

A command page and an evidence page that named different tests for one command would each
be a faithful projection of a different reading of the same headers, and a reader could not
tell which was the answer. Several readers of one declaration is how that happens; holding
them equal, and naming each one, is how it stops happening until there is one reader.
{% endraw %}
