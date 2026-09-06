+++
title = "A fluent contribution that could not have known the rules"
description = "Assistants let anybody produce plausible contributions at volume; the conventions that make one correct are still only in the maintainers."
weight = 350
[extra]
id = "contribution-that-could-not-have-known"
status = "stable"
source = ".ai/repo/why/moments/contribution-that-could-not-have-known.md"
+++
{% raw %}

## The moment

The contribution is well written. The tests pass. It uses the pattern the project abandoned
two years ago, for reasons that are excellent and recorded nowhere the contributor could
reach. The maintainer writes the explanation for the eleventh time.

## Why it happens

A project's real conventions are the residue of its arguments, and arguments are not
documents. They survive in the maintainers and in old review threads. Assistants changed the
economics on one side only: producing a plausible contribution is now nearly free, and
transmitting the conventions is exactly as expensive as it was.

## Why a better model does not fix it

The contributor's assistant did well with what it had — the code, the README, the tests.
None of them contains the reason the abandoned pattern was abandoned. A stronger model makes
a more convincing case for the wrong pattern.

## What it costs

The maintainer's scarcest resource, spent on repetition. And a queue that grows faster than
it drains, which eventually converts an open project into a closed one.

## What Majordomus does

The rules are objects, not folklore: the effective set is the vendored baseline plus the
project's own, resolved as a dependency graph, each saying whether the tool enforces it or
nobody does. The context documents attach to the directories and paths they govern and
compose for the path being changed, so a contributor's assistant can be told to resolve them
before writing. Decisions carry their reason and the alternative that was rejected, which is
the sentence review keeps having to supply.

## Before and after

```text
before   review comment #11: "we don't use that pattern here, because ..."

after    $ majordomus context resolve lib/auth
         $ majordomus rules list
         project.no-claim-without-test   blocking   enforced by: review
         majordomus.scope-integrity      blocking   enforced by: check, finish, watch
```

## What it does not do

It does not gate contributions, and it cannot make anyone read anything. It converts the
conventions from something a maintainer transmits into something a contributor can load.
{% endraw %}
