+++
title = "The policy changed and its four copies did not"
description = "A decision is updated at its source and the generated copies keep serving the previous version, with nothing reporting the difference."
weight = 170
[extra]
id = "policy-changed-projection-stale"
status = "stable"
source = ".ai/repo/why/moments/policy-changed-projection-stale.md"
+++
{% raw %}

## The moment

The policy was updated in March. The four files generated from it still carry the February
version, because regenerating them was a step somebody had to remember, and nothing said
they were stale.

## Why it happens

Generation without a staleness check is a one-shot copy. The moment the source changes, the
copies become claims about a past state, and they look exactly like claims about the current
one. Nothing in the file says which generation produced it or from what.

## Why a better model does not fix it

The worker reads the file it is given and applies it faithfully. It has no way to know the
file is a stale projection of something else, because staleness is a property of the pair,
and the worker only ever sees one half.

## What it costs

Workers follow a superseded policy, and the failure surfaces as behaviour nobody asked for,
weeks after the change that caused it. Tracing it back means discovering that the source of
truth was never the source anything read.

## What Majordomus does

Every generated projection is stamped with the hash of the policy it came from and the hash
of its own content. `doctor` and `watch` compare both: a source that moved on, or a file
that was edited by hand, is a named failing check. `update` regenerates deterministically
and refuses to overwrite a hand edit until the diff has been seen. The same discipline
covers every derived artifact the repository commits, and CI refuses a tree in which any of
them differs from what its source produces.

## Before and after

```text
before   policy.yaml  (March)      CLAUDE.md, AGENTS.md, ... (February)

after    $ majordomus doctor
         FAIL projection  CLAUDE.md — generated from policy 3b0d79b4, current is 9f21c0aa
                          [reproduce: majordomus update --diff CLAUDE.md]
```

## What it does not do

It does not decide whether the change should propagate; regeneration is explicit and its
diff is reviewable. It stamps and compares — it does not merge a hand edit back into the
source.
{% endraw %}
